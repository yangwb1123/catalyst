use std::{fs, path::Path};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphManager,
    GroupAgentGraphManifest, GroupAgentGraphNode, GroupAgentGraphSource, GroupAgentGraphStore,
    GroupContextPolicy, GroupRunSnapshot, GroupRunStore, HubStore, HubStoreError,
    PrepareGroupAgentGraph, PrepareGroupRun,
};
use tempfile::TempDir;

use super::{read, write};
use crate::sqlite_hub::{SqliteHubStore, group_context_build};

pub(in crate::sqlite_hub) struct Fixture {
    _root: TempDir,
    pub(in crate::sqlite_hub) store: SqliteHubStore,
    pub(in crate::sqlite_hub) request: PrepareGroupAgentGraph,
}

#[test]
fn write_reread_failure_rolls_back_the_graph_row() {
    let fixture = fixture();
    let mut connection = fixture.store.connect().expect("validated graph connection");
    connection
        .execute_batch(
            "CREATE TRIGGER mutate_graph_after_insert
             AFTER INSERT ON group_agent_graphs
             BEGIN
               UPDATE group_agent_graphs SET node_count=2 WHERE id=NEW.id;
             END;",
        )
        .expect("install post-insert mutation");

    assert!(matches!(
        write::prepare(&mut connection, &fixture.request),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(row_count(&connection, "group_agent_graphs"), 0);
    assert!(connection.is_autocommit());
}

#[test]
fn inspect_keeps_graph_and_source_in_one_deferred_snapshot() {
    let fixture = fixture();
    let created = fixture
        .store
        .prepare_group_agent_graph(&fixture.request)
        .expect("prepare graph")
        .inspection;
    let mut reader = fixture.store.connect().expect("validated graph reader");
    let writer = fixture.store.connect().expect("validated graph writer");

    let inspection = read::inspect_after_graph(&mut reader, "graph-1", || {
        writer
            .execute(
                "UPDATE group_runs SET snapshot_sha256=zeroblob(32) WHERE id='group-run-1'",
                [],
            )
            .expect("commit source corruption after graph read");
        Ok(())
    })
    .expect("reader retains its original snapshot");

    assert_eq!(inspection, created);
    assert!(matches!(
        fixture.store.inspect_group_agent_graph("graph-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

pub(in crate::sqlite_hub) fn fixture() -> Fixture {
    let root = TempDir::new().expect("graph atomicity root");
    let database = root.path().join("state").join("hub.sqlite3");
    let store = SqliteHubStore::open(database).expect("open Hub");
    let group = store
        .create_group("Atomic graph", "atomic-group-key")
        .expect("create Group");
    let project_id = add_member(&store, root.path(), &group.id);
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
    let request = graph_request(&snapshot, &project_id);
    Fixture {
        _root: root,
        store,
        request,
    }
}

fn add_member(store: &SqliteHubStore, root: &Path, group_id: &str) -> String {
    let path = root.join("worker");
    fs::create_dir(&path).expect("project directory");
    let project = store
        .open_project(&path.canonicalize().expect("canonical project"))
        .expect("open Project");
    store
        .add_project_to_group(group_id, &project.id, "worker", "worker-link-key")
        .expect("link Group member");
    project.id
}

fn graph_request(snapshot: &GroupRunSnapshot, project_id: &str) -> PrepareGroupAgentGraph {
    let manifest = graph_manifest(snapshot, project_id);
    let bytes = group_context_build::canonical_json_bytes(&manifest).expect("canonical manifest");
    PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-1".into(),
        manifest,
        manifest_json: String::from_utf8(bytes.clone()).expect("manifest UTF-8"),
        manifest_sha256: group_context_build::digest_with_domain(
            GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN,
            &bytes,
        ),
        idempotency_key: "graph-key".into(),
        created_at_ms: 20,
    }
}

fn graph_manifest(snapshot: &GroupRunSnapshot, project_id: &str) -> GroupAgentGraphManifest {
    GroupAgentGraphManifest {
        v: GROUP_AGENT_GRAPH_VERSION,
        source: source(snapshot),
        manager: GroupAgentGraphManager {
            agent_profile: "manager".into(),
            instruction: "coordinate the frozen worker".into(),
        },
        nodes: vec![GroupAgentGraphNode {
            node_id: "worker".into(),
            project_id: project_id.into(),
            member_role: "worker".into(),
            agent_profile: "implementer".into(),
            task: "complete the bounded task".into(),
            acceptance: "the task contract passes".into(),
        }],
        edges: Vec::new(),
        waves: vec![vec!["worker".into()]],
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

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
