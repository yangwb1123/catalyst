use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
};

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphEdge,
    GroupAgentGraphInspection, GroupAgentGraphManager, GroupAgentGraphManifest,
    GroupAgentGraphNode, GroupAgentGraphSource, GroupAgentGraphStore, GroupContextPolicy,
    GroupRunSnapshot, GroupRunStore, HubEntity, HubStore, HubStoreError, PrepareGroupAgentGraph,
    PrepareGroupAgentGraphDisposition, PrepareGroupRun, compute_group_agent_graph_waves,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    database: PathBuf,
    store: SqliteHubStore,
    snapshot: GroupRunSnapshot,
    frontend_id: String,
    backend_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("graph root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let group = store.create_group("Delivery", "group-key").expect("Group");
        let frontend_id = add_member(&store, root.path(), &group.id, "frontend", "frontend");
        let backend_id = add_member(&store, root.path(), &group.id, "backend", "backend");
        let snapshot = store
            .prepare_group_run(&PrepareGroupRun {
                v: 1,
                run_id: "group-run-1".into(),
                group_id: group.id,
                policy: GroupContextPolicy::default(),
                idempotency_key: "group-run-key".into(),
                created_at_ms: 10,
            })
            .expect("prepare Group Run")
            .snapshot;
        Self {
            _root: root,
            database,
            store,
            snapshot,
            frontend_id,
            backend_id,
        }
    }

    fn request(&self, graph_id: &str, key: &str, time: u64) -> PrepareGroupAgentGraph {
        let nodes = vec![
            node("frontend", &self.frontend_id, "frontend"),
            node("backend", &self.backend_id, "backend"),
        ];
        let edges = vec![GroupAgentGraphEdge {
            from_node_id: "frontend".into(),
            to_node_id: "backend".into(),
        }];
        request(&self.snapshot, nodes, edges, graph_id, key, time)
    }
}

#[test]
fn prepare_replays_exact_bytes_and_reads_full_and_metadata_views() {
    let fixture = Fixture::new();
    let request = fixture.request("graph-1", "graph-key", 20);
    let created = fixture
        .store
        .prepare_group_agent_graph(&request)
        .expect("prepare graph");
    assert_eq!(
        created.disposition,
        PrepareGroupAgentGraphDisposition::Created
    );
    assert_eq!(created.inspection.manifest_json, request.manifest_json);

    let mut replay = request.clone();
    replay.graph_id = "ignored-candidate".into();
    replay.created_at_ms = 999;
    let replayed = fixture
        .store
        .prepare_group_agent_graph(&replay)
        .expect("exact replay");
    assert_eq!(
        replayed.disposition,
        PrepareGroupAgentGraphDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_reads(&fixture, &created.inspection);
    assert_row_shape(&fixture, &created.inspection);
}

#[test]
fn conflicting_input_id_source_and_membership_leave_no_partial_rows() {
    let fixture = Fixture::new();
    let request = fixture.request("graph-1", "graph-key", 20);
    fixture
        .store
        .prepare_group_agent_graph(&request)
        .expect("seed graph");

    let mut changed = request.clone();
    changed.manifest.manager.instruction = "different instruction".into();
    recanonicalize(&mut changed);
    assert_graph_conflict(&fixture.store.prepare_group_agent_graph(&changed));

    let reused_id = fixture.request("graph-1", "other-key", 21);
    assert_graph_conflict(&fixture.store.prepare_group_agent_graph(&reused_id));

    let mut bad_member = fixture.request("graph-2", "bad-member-key", 22);
    bad_member.manifest.nodes[0].member_role = "identity".into();
    recanonicalize(&mut bad_member);
    assert_graph_conflict(&fixture.store.prepare_group_agent_graph(&bad_member));

    let mut bad_source = fixture.request("graph-3", "bad-source-key", 23);
    bad_source.manifest.source.snapshot_sha256 = "0".repeat(64);
    recanonicalize(&mut bad_source);
    assert_graph_conflict(&fixture.store.prepare_group_agent_graph(&bad_source));
    assert_eq!(row_count(&fixture.database, "group_agent_graphs"), 1);
}

#[test]
fn concurrent_same_key_has_one_identity_and_divergence_conflicts() {
    const WORKERS: usize = 6;
    let fixture = Fixture::new();
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            let request = fixture.request(
                &format!("candidate-{index}"),
                "shared-key",
                30 + u64::try_from(index).expect("worker index fits u64"),
            );
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .prepare_group_agent_graph(&request)
                    .expect("concurrent prepare")
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("graph worker"))
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| result.disposition == PrepareGroupAgentGraphDisposition::Created)
            .count(),
        1
    );
    assert!(
        results
            .iter()
            .all(|result| result.inspection == results[0].inspection)
    );

    let mut divergent = fixture.request("other", "shared-key", 99);
    divergent.manifest.manager.instruction = "divergent".into();
    recanonicalize(&mut divergent);
    assert_graph_conflict(&fixture.store.prepare_group_agent_graph(&divergent));
    assert_eq!(row_count(&fixture.database, "group_agent_graphs"), 1);
}

#[test]
fn concurrent_divergent_same_key_has_one_winner_and_one_conflict() {
    let fixture = Fixture::new();
    let barrier = Arc::new(Barrier::new(2));
    let mut first = fixture.request("graph-a", "divergent-key", 40);
    let mut second = fixture.request("graph-b", "divergent-key", 41);
    first.manifest.manager.instruction = "first plan".into();
    second.manifest.manager.instruction = "second plan".into();
    recanonicalize(&mut first);
    recanonicalize(&mut second);
    let workers = [first, second]
        .into_iter()
        .map(|request| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                store.prepare_group_agent_graph(&request)
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("divergent graph worker"))
        .collect::<Vec<_>>();

    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    Err(HubStoreError::Conflict {
                        entity: HubEntity::GroupAgentGraph,
                        ..
                    })
                )
            })
            .count(),
        1
    );
    assert_eq!(row_count(&fixture.database, "group_agent_graphs"), 1);
}

#[test]
fn stored_corruption_remains_corruption_and_metadata_fails_closed() {
    for sql in [
        "UPDATE group_agent_graphs SET manifest_blob=zeroblob(length(manifest_blob))",
        "UPDATE group_agent_graphs SET manifest_sha256=zeroblob(32)",
        "UPDATE group_agent_graphs SET source_snapshot_sha256=zeroblob(32)",
        "UPDATE group_runs SET snapshot_sha256=zeroblob(32) WHERE id='group-run-1'",
    ] {
        let fixture = prepared_fixture();
        Connection::open(&fixture.database)
            .expect("raw SQLite")
            .execute_batch(sql)
            .expect("inject graph corruption");
        assert!(matches!(
            fixture.store.inspect_group_agent_graph("graph-1"),
            Err(HubStoreError::Corrupt { .. })
        ));
        assert!(matches!(
            fixture
                .store
                .prepare_group_agent_graph(&fixture.request("ignored", "graph-key", 99)),
            Err(HubStoreError::Corrupt { .. })
        ));
        let metadata = fixture
            .store
            .list_group_agent_graphs(Some("group-run-1"), 10)
            .expect("metadata list does not load source or manifest content");
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].graph_id, "graph-1");
    }

    let fixture = prepared_fixture();
    let connection = Connection::open(&fixture.database).expect("raw metadata SQLite");
    connection
        .execute_batch(
            "PRAGMA ignore_check_constraints = ON;
             UPDATE group_agent_graphs SET node_count=0;",
        )
        .expect("inject invalid metadata");
    assert!(matches!(
        fixture.store.list_group_agent_graphs(None, 10),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn missing_filter_and_limit_errors_fail_closed_without_rows() {
    let fixture = Fixture::new();
    assert!(matches!(
        fixture.store.list_group_agent_graphs(Some("missing"), 10),
        Err(HubStoreError::NotFound {
            entity: HubEntity::GroupRun,
            ..
        })
    ));
    for limit in [0, 101] {
        assert_graph_conflict(&fixture.store.list_group_agent_graphs(None, limit));
    }
    let oversized_filter = "x".repeat(129);
    assert_graph_conflict(
        &fixture
            .store
            .list_group_agent_graphs(Some(&oversized_filter), 10),
    );

    assert_eq!(row_count(&fixture.database, "group_agent_graphs"), 0);
}

fn prepared_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .store
        .prepare_group_agent_graph(&fixture.request("graph-1", "graph-key", 20))
        .expect("prepare graph");
    fixture
}

fn add_member(
    store: &SqliteHubStore,
    root: &Path,
    group_id: &str,
    name: &str,
    role: &str,
) -> String {
    let path = root.join(name);
    fs::create_dir(&path).expect("project directory");
    let project = store
        .open_project(&path.canonicalize().expect("canonical project"))
        .expect("Project");
    store
        .add_project_to_group(group_id, &project.id, role, &format!("link-{name}"))
        .expect("link Group member");
    project.id
}

fn node(node_id: &str, project_id: &str, member_role: &str) -> GroupAgentGraphNode {
    GroupAgentGraphNode {
        node_id: node_id.into(),
        project_id: project_id.into(),
        member_role: member_role.into(),
        agent_profile: "implementer".into(),
        task: format!("complete {node_id} integration"),
        acceptance: format!("{node_id} contract passes"),
    }
}

fn request(
    snapshot: &GroupRunSnapshot,
    nodes: Vec<GroupAgentGraphNode>,
    edges: Vec<GroupAgentGraphEdge>,
    graph_id: &str,
    key: &str,
    created_at_ms: u64,
) -> PrepareGroupAgentGraph {
    let waves = compute_group_agent_graph_waves(&nodes, &edges).expect("DAG waves");
    let manifest = GroupAgentGraphManifest {
        v: GROUP_AGENT_GRAPH_VERSION,
        source: source(snapshot),
        manager: GroupAgentGraphManager {
            agent_profile: "integration-manager".into(),
            instruction: "coordinate the frozen member projects".into(),
        },
        nodes,
        edges,
        waves,
    };
    let bytes = canonical_json_bytes(&manifest);
    PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: graph_id.into(),
        manifest,
        manifest_json: String::from_utf8(bytes.clone()).expect("manifest UTF-8"),
        manifest_sha256: digest_hex(&bytes),
        idempotency_key: key.into(),
        created_at_ms,
    }
}

fn source(snapshot: &GroupRunSnapshot) -> GroupAgentGraphSource {
    GroupAgentGraphSource {
        group_run_version: snapshot.run.v,
        group_run_id: snapshot.run.run_id.clone(),
        group_id: snapshot.run.group_id.clone(),
        context_version: snapshot.run.context_version,
        context_slice_sha256: snapshot.run.context_slice_sha256.clone(),
        snapshot_sha256: snapshot.run.snapshot_sha256.clone(),
        snapshot_bytes: snapshot.run.snapshot_bytes,
    }
}

fn recanonicalize(request: &mut PrepareGroupAgentGraph) {
    let bytes = canonical_json_bytes(&request.manifest);
    request.manifest_json = String::from_utf8(bytes.clone()).expect("manifest UTF-8");
    request.manifest_sha256 = digest_hex(&bytes);
}

fn canonical_json_bytes(value: &impl Serialize) -> Vec<u8> {
    let value = serde_json::to_value(value).expect("JSON value");
    serde_json::to_vec(&sort_json(value)).expect("canonical JSON")
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => Value::Object(
            items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        other => other,
    }
}

fn digest_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn assert_reads(fixture: &Fixture, expected: &GroupAgentGraphInspection) {
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_graph(&expected.graph.graph_id)
            .expect("inspect graph"),
        *expected
    );
    let listed = fixture
        .store
        .list_group_agent_graphs(Some("group-run-1"), 10)
        .expect("list graph");
    assert_eq!(listed.as_slice(), std::slice::from_ref(&expected.graph));
}

fn assert_row_shape(fixture: &Fixture, expected: &GroupAgentGraphInspection) {
    let connection = Connection::open(&fixture.database).expect("raw row SQLite");
    let row: (String, String, i64, String, i64, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT id,group_run_id,graph_version,status,manifest_bytes,
                    node_count,edge_count,wave_count,created_at_ms
             FROM group_agent_graphs",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            },
        )
        .expect("graph row");
    assert_eq!(
        row,
        (
            expected.graph.graph_id.clone(),
            expected.graph.group_run_id.clone(),
            1,
            "prepared".into(),
            i64::try_from(expected.graph.manifest_bytes).expect("manifest bytes fit i64"),
            2,
            1,
            2,
            i64::try_from(expected.graph.created_at_ms).expect("creation time fits i64"),
        )
    );
    assert_eq!(row_count(&fixture.database, "runs"), 0);
    assert_eq!(row_count(&fixture.database, "prompts"), 0);
}

fn assert_graph_conflict<T>(result: &Result<T, HubStoreError>) {
    assert!(matches!(
        result,
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentGraph,
            ..
        })
    ));
}

fn row_count(database: &Path, table: &str) -> i64 {
    Connection::open(database)
        .expect("raw SQLite")
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
