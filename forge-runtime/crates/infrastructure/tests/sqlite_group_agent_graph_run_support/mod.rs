use std::{
    fs,
    path::{Path, PathBuf},
};

use forge_runtime_domain::{
    BeginGroupAgentGraphRun, GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
    GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_RUN_VERSION,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GROUP_AGENT_GRAPH_VERSION,
    GroupAgentGraphCorePlan, GroupAgentGraphEdge, GroupAgentGraphInspection,
    GroupAgentGraphManager, GroupAgentGraphManifest, GroupAgentGraphNode, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentGraphSource, GroupAgentGraphStore, GroupContextPolicy,
    GroupRunSnapshot, GroupRunStore, HubStore, PrepareGroupAgentGraph, PrepareGroupRun,
    compute_group_agent_graph_waves,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use tempfile::TempDir;

mod encoding;
use encoding::{canonical_json_bytes, decode_hex, digest_hex};

pub struct Fixture {
    pub _root: TempDir,
    pub database: PathBuf,
    pub store: SqliteHubStore,
    pub graph: GroupAgentGraphInspection,
}

impl Fixture {
    pub fn new() -> Self {
        let root = TempDir::new().expect("Graph Run root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let group = store.create_group("Delivery", "group-key").expect("Group");
        let frontend = add_member(&store, root.path(), &group.id, "frontend");
        let backend = add_member(&store, root.path(), &group.id, "backend");
        let snapshot = prepare_group_run(&store, &group.id);
        let graph = prepare_graph(&store, &snapshot, &frontend, &backend);
        Self {
            _root: root,
            database,
            store,
            graph,
        }
    }

    /// Diamond (frontend, backend -> sso): same-wave siblings, zero receipts.
    #[allow(dead_code)]
    pub fn diamond() -> Self {
        let root = TempDir::new().expect("diamond Graph Run root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let group = store.create_group("Delivery", "group-key").expect("Group");
        let frontend = add_member(&store, root.path(), &group.id, "frontend");
        let backend = add_member(&store, root.path(), &group.id, "backend");
        let sso = add_member(&store, root.path(), &group.id, "sso");
        let snapshot = prepare_group_run(&store, &group.id);
        let graph = prepare_graph_diamond(&store, &snapshot, &frontend, &backend, &sso);
        Self {
            _root: root,
            database,
            store,
            graph,
        }
    }

    /// Serial chain (frontend -> backend -> sso): two successive successors.
    #[allow(dead_code)]
    pub fn serial_three() -> Self {
        let root = TempDir::new().expect("serial-three Graph Run root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let group = store.create_group("Delivery", "group-key").expect("Group");
        let frontend = add_member(&store, root.path(), &group.id, "frontend");
        let backend = add_member(&store, root.path(), &group.id, "backend");
        let sso = add_member(&store, root.path(), &group.id, "sso");
        let snapshot = prepare_group_run(&store, &group.id);
        let graph = prepare_graph_three(&store, &snapshot, &frontend, &backend, &sso);
        Self {
            _root: root,
            database,
            store,
            graph,
        }
    }

    #[allow(dead_code)]
    pub fn single_node() -> Self {
        let root = TempDir::new().expect("single-node Graph Run root");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Hub");
        let group = store.create_group("Delivery", "group-key").expect("Group");
        let project = add_member(&store, root.path(), &group.id, "frontend");
        let snapshot = prepare_group_run(&store, &group.id);
        let graph = prepare_single_graph(&store, &snapshot, &project);
        Self {
            _root: root,
            database,
            store,
            graph,
        }
    }

    pub fn request(&self, run_id: &str, key: &str, created_at_ms: u64) -> BeginGroupAgentGraphRun {
        request(&self.graph, run_id, key, created_at_ms)
    }

    #[allow(dead_code)]
    pub fn prepare_single_node_sibling(&self) -> GroupAgentGraphInspection {
        let node = self
            .graph
            .manifest
            .nodes
            .first()
            .expect("source Graph node")
            .clone();
        let node_id = node.node_id.clone();
        let manifest = GroupAgentGraphManifest {
            v: GROUP_AGENT_GRAPH_VERSION,
            source: self.graph.manifest.source.clone(),
            manager: self.graph.manifest.manager.clone(),
            nodes: vec![node],
            edges: Vec::new(),
            waves: vec![vec![node_id]],
        };
        let bytes = canonical_json_bytes(&manifest);
        self.store
            .prepare_group_agent_graph(&PrepareGroupAgentGraph {
                v: GROUP_AGENT_GRAPH_VERSION,
                graph_id: "graph-single-sibling".into(),
                manifest,
                manifest_json: String::from_utf8(bytes.clone()).expect("manifest UTF-8"),
                manifest_sha256: digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes),
                idempotency_key: "graph-single-sibling-key".into(),
                created_at_ms: 20,
            })
            .expect("prepare single-node sibling Graph")
            .inspection
    }

    pub fn connection(&self) -> Connection {
        Connection::open(&self.database).expect("raw Graph Run SQLite")
    }

    pub fn row_count(&self, table: &str) -> i64 {
        self.connection()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("row count")
    }
}

pub fn recanonicalize(request: &mut BeginGroupAgentGraphRun) {
    request.plan.plan_sha256 = request.plan.expected_sha256().expect("Core Plan digest");
    request.plan_json = request.plan.canonical_json().expect("canonical Core Plan");
    request.event = prepared_event(request);
    request.event_json = request.event.canonical_json().expect("canonical event");
}

pub fn encode_graph_manifest(manifest: &GroupAgentGraphManifest) -> (Vec<u8>, Vec<u8>) {
    let bytes = canonical_json_bytes(manifest);
    let hex = digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes);
    (bytes, decode_hex(&hex))
}

fn add_member(store: &SqliteHubStore, root: &Path, group_id: &str, role: &str) -> String {
    let path = root.join(role);
    fs::create_dir(&path).expect("project directory");
    let project = store
        .open_project(&path.canonicalize().expect("canonical project"))
        .expect("Project");
    store
        .add_project_to_group(group_id, &project.id, role, &format!("link-{role}"))
        .expect("link Group member");
    project.id
}

fn prepare_group_run(store: &SqliteHubStore, group_id: &str) -> GroupRunSnapshot {
    store
        .prepare_group_run(&PrepareGroupRun {
            v: 1,
            run_id: "group-run-1".into(),
            group_id: group_id.into(),
            policy: GroupContextPolicy::default(),
            idempotency_key: "group-run-key".into(),
            created_at_ms: 10,
        })
        .expect("prepare Group Run")
        .snapshot
}

fn prepare_graph_diamond(
    store: &SqliteHubStore,
    snapshot: &GroupRunSnapshot,
    frontend: &str,
    backend: &str,
    sso: &str,
) -> GroupAgentGraphInspection {
    let request = graph_request_diamond(snapshot, frontend, backend, sso);
    store
        .prepare_group_agent_graph(&request)
        .expect("prepare diamond Group Agent Graph")
        .inspection
}

fn graph_request_diamond(
    snapshot: &GroupRunSnapshot,
    frontend: &str,
    backend: &str,
    sso: &str,
) -> PrepareGroupAgentGraph {
    let nodes = vec![
        graph_node("frontend", frontend),
        graph_node("backend", backend),
        graph_node("sso", sso),
    ];
    let mut edges = vec![
        GroupAgentGraphEdge {
            from_node_id: "frontend".into(),
            to_node_id: "sso".into(),
        },
        GroupAgentGraphEdge {
            from_node_id: "backend".into(),
            to_node_id: "sso".into(),
        },
    ];
    edges.sort_by(|a, b| {
        a.from_node_id
            .cmp(&b.from_node_id)
            .then(a.to_node_id.cmp(&b.to_node_id))
    });
    let waves = compute_group_agent_graph_waves(&nodes, &edges).expect("diamond DAG waves");
    let manifest = graph_manifest(snapshot, nodes, edges, waves);
    let bytes = canonical_json_bytes(&manifest);
    PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-diamond".into(),
        manifest,
        manifest_json: String::from_utf8(bytes.clone()).expect("manifest UTF-8"),
        manifest_sha256: digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes),
        idempotency_key: "graph-diamond-key".into(),
        created_at_ms: 10,
    }
}

fn prepare_graph_three(
    store: &SqliteHubStore,
    snapshot: &GroupRunSnapshot,
    frontend: &str,
    backend: &str,
    sso: &str,
) -> GroupAgentGraphInspection {
    let request = graph_request_three(snapshot, frontend, backend, sso);
    store
        .prepare_group_agent_graph(&request)
        .expect("prepare three-node Group Agent Graph")
        .inspection
}

fn graph_request_three(
    snapshot: &GroupRunSnapshot,
    frontend: &str,
    backend: &str,
    sso: &str,
) -> PrepareGroupAgentGraph {
    let nodes = vec![
        graph_node("frontend", frontend),
        graph_node("backend", backend),
        graph_node("sso", sso),
    ];
    let mut edges = vec![
        GroupAgentGraphEdge {
            from_node_id: "frontend".into(),
            to_node_id: "backend".into(),
        },
        GroupAgentGraphEdge {
            from_node_id: "backend".into(),
            to_node_id: "sso".into(),
        },
    ];
    edges.sort_by(|a, b| {
        a.from_node_id
            .cmp(&b.from_node_id)
            .then(a.to_node_id.cmp(&b.to_node_id))
    });
    let waves = compute_group_agent_graph_waves(&nodes, &edges).expect("serial-three DAG waves");
    let manifest = graph_manifest(snapshot, nodes, edges, waves);
    let bytes = canonical_json_bytes(&manifest);
    PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-serial-three".into(),
        manifest,
        manifest_json: String::from_utf8(bytes.clone()).expect("manifest UTF-8"),
        manifest_sha256: digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes),
        idempotency_key: "graph-serial-three-key".into(),
        created_at_ms: 10,
    }
}

fn prepare_graph(
    store: &SqliteHubStore,
    snapshot: &GroupRunSnapshot,
    frontend: &str,
    backend: &str,
) -> GroupAgentGraphInspection {
    let request = graph_request(snapshot, frontend, backend);
    store
        .prepare_group_agent_graph(&request)
        .expect("prepare Group Agent Graph")
        .inspection
}

#[allow(dead_code)]
fn prepare_single_graph(
    store: &SqliteHubStore,
    snapshot: &GroupRunSnapshot,
    project: &str,
) -> GroupAgentGraphInspection {
    let nodes = vec![graph_node("frontend", project)];
    let edges = Vec::new();
    let waves = compute_group_agent_graph_waves(&nodes, &edges).expect("single-node DAG waves");
    let manifest = graph_manifest(snapshot, nodes, edges, waves);
    let bytes = canonical_json_bytes(&manifest);
    let request = PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-single".into(),
        manifest,
        manifest_json: String::from_utf8(bytes.clone()).expect("manifest UTF-8"),
        manifest_sha256: digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes),
        idempotency_key: "graph-single-key".into(),
        created_at_ms: 20,
    };
    store
        .prepare_group_agent_graph(&request)
        .expect("prepare single-node Graph")
        .inspection
}

fn graph_request(
    snapshot: &GroupRunSnapshot,
    frontend: &str,
    backend: &str,
) -> PrepareGroupAgentGraph {
    let nodes = vec![
        graph_node("frontend", frontend),
        graph_node("backend", backend),
    ];
    let edges = vec![GroupAgentGraphEdge {
        from_node_id: "frontend".into(),
        to_node_id: "backend".into(),
    }];
    let waves = compute_group_agent_graph_waves(&nodes, &edges).expect("DAG waves");
    let manifest = graph_manifest(snapshot, nodes, edges, waves);
    let bytes = canonical_json_bytes(&manifest);
    PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-1".into(),
        manifest,
        manifest_json: String::from_utf8(bytes.clone()).expect("manifest UTF-8"),
        manifest_sha256: digest_hex(GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN, &bytes),
        idempotency_key: "graph-key".into(),
        created_at_ms: 20,
    }
}

fn graph_node(node_id: &str, project_id: &str) -> GroupAgentGraphNode {
    GroupAgentGraphNode {
        node_id: node_id.into(),
        project_id: project_id.into(),
        member_role: node_id.into(),
        agent_profile: "implementer".into(),
        task: format!("complete {node_id} integration"),
        acceptance: format!("{node_id} contract passes"),
    }
}

fn graph_manifest(
    snapshot: &GroupRunSnapshot,
    nodes: Vec<GroupAgentGraphNode>,
    edges: Vec<GroupAgentGraphEdge>,
    waves: Vec<Vec<String>>,
) -> GroupAgentGraphManifest {
    GroupAgentGraphManifest {
        v: GROUP_AGENT_GRAPH_VERSION,
        source: graph_source(snapshot),
        manager: GroupAgentGraphManager {
            agent_profile: "integration-manager".into(),
            instruction: "coordinate the frozen member projects".into(),
        },
        nodes,
        edges,
        waves,
    }
}

fn graph_source(snapshot: &GroupRunSnapshot) -> GroupAgentGraphSource {
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

fn request(
    graph: &GroupAgentGraphInspection,
    run_id: &str,
    key: &str,
    created_at_ms: u64,
) -> BeginGroupAgentGraphRun {
    let mut plan = core_plan(graph);
    plan.plan_sha256 = plan.expected_sha256().expect("Core Plan digest");
    let event = event(run_id, graph, &plan, created_at_ms);
    BeginGroupAgentGraphRun {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: run_id.into(),
        graph_id: graph.graph.graph_id.clone(),
        source_snapshot_sha256: graph.graph.source_snapshot_sha256.clone(),
        graph_manifest_sha256: graph.graph.manifest_sha256.clone(),
        plan_json: plan.canonical_json().expect("canonical Core Plan"),
        plan,
        event_json: event.canonical_json().expect("canonical event"),
        event,
        idempotency_key: key.into(),
        created_at_ms,
    }
}

fn core_plan(graph: &GroupAgentGraphInspection) -> GroupAgentGraphCorePlan {
    GroupAgentGraphCorePlan {
        v: GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        graph_version: graph.graph.v,
        graph_id: graph.graph.graph_id.clone(),
        graph_manifest_sha256: graph.graph.manifest_sha256.clone(),
        authored_node_ids: graph
            .manifest
            .nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect(),
        edges: graph.manifest.edges.clone(),
        waves: graph.manifest.waves.clone(),
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan_sha256: "0".repeat(64),
    }
}

fn event(
    run_id: &str,
    graph: &GroupAgentGraphInspection,
    plan: &GroupAgentGraphCorePlan,
    created_at_ms: u64,
) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: run_id.into(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: graph.graph.graph_id.clone(),
            graph_manifest_sha256: graph.graph.manifest_sha256.clone(),
            plan_sha256: plan.plan_sha256.clone(),
            scheduler_protocol_version: plan.scheduler_protocol_version,
            prepared_at_ms: created_at_ms,
        },
    }
}

fn prepared_event(request: &BeginGroupAgentGraphRun) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: request.graph_run_id.clone(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: request.graph_id.clone(),
            graph_manifest_sha256: request.graph_manifest_sha256.clone(),
            plan_sha256: request.plan.plan_sha256.clone(),
            scheduler_protocol_version: request.plan.scheduler_protocol_version,
            prepared_at_ms: request.created_at_ms,
        },
    }
}
