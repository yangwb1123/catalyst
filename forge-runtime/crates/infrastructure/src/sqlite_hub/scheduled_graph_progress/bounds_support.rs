use std::{fs, path::Path};

use crate::runtime_domain::{
    GROUP_AGENT_GRAPH_VERSION, GroupAgentGraphEdge, GroupAgentGraphInspection,
    GroupAgentGraphManager, GroupAgentGraphManifest, GroupAgentGraphNode, GroupAgentGraphSource,
    GroupAgentGraphStore, GroupContextPolicy, GroupRunSnapshot, GroupRunStore, HubStore,
    PrepareGroupAgentGraph, PrepareGroupRun, compute_group_agent_graph_waves,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

use super::{
    sqlite_group_agent_graph_execution_schedule_support as schedule_support,
    sqlite_group_agent_graph_run_support::{Fixture, encode_graph_manifest},
};
use crate::runtime_domain::{
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore, MAX_GROUP_AGENT_GRAPH_NODES,
};

pub(super) fn prepared_fixture() -> Fixture {
    let fixture = serial_fixture();
    fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-1", "run-bound-key", 30))
        .expect("begin bound Graph Run");
    let schedule = schedule_support::request(&fixture, "schedule-bound-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit bound schedule");
    fixture
}

fn serial_fixture() -> Fixture {
    let root = TempDir::new().expect("bound Graph Run root");
    let database = root.path().join("state").join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("open bound Hub");
    let group = store
        .create_group("Bound Delivery", "group-bound-key")
        .expect("create bound Group");
    let project = add_member(&store, root.path(), &group.id);
    let snapshot = prepare_group_run(&store, &group.id);
    let graph = prepare_graph(&store, &snapshot, &project);
    Fixture {
        _root: root,
        database,
        store,
        graph,
    }
}

fn add_member(store: &SqliteHubStore, root: &Path, group_id: &str) -> String {
    let path = root.join("worker");
    fs::create_dir(&path).expect("bound project directory");
    let project = store
        .open_project(&path.canonicalize().expect("canonical bound project"))
        .expect("open bound Project");
    store
        .add_project_to_group(group_id, &project.id, "worker", "link-bound-worker")
        .expect("link bound Group member");
    project.id
}

fn prepare_group_run(store: &SqliteHubStore, group_id: &str) -> GroupRunSnapshot {
    store
        .prepare_group_run(&PrepareGroupRun {
            v: 1,
            run_id: "group-run-bound-32".into(),
            group_id: group_id.into(),
            policy: GroupContextPolicy::default(),
            idempotency_key: "group-run-bound-key".into(),
            created_at_ms: 10,
        })
        .expect("prepare bound Group Run")
        .snapshot
}

fn prepare_graph(
    store: &SqliteHubStore,
    snapshot: &GroupRunSnapshot,
    project_id: &str,
) -> GroupAgentGraphInspection {
    let manifest = serial_manifest(snapshot, project_id);
    let (bytes, _) = encode_graph_manifest(&manifest);
    let request = PrepareGroupAgentGraph {
        v: GROUP_AGENT_GRAPH_VERSION,
        graph_id: "graph-bound-32".into(),
        manifest_sha256: manifest.expected_sha256().expect("bound manifest digest"),
        manifest_json: String::from_utf8(bytes).expect("bound manifest UTF-8"),
        manifest,
        idempotency_key: "graph-bound-key".into(),
        created_at_ms: 20,
    };
    store
        .prepare_group_agent_graph(&request)
        .expect("prepare bound Graph")
        .inspection
}

fn serial_manifest(snapshot: &GroupRunSnapshot, project_id: &str) -> GroupAgentGraphManifest {
    let nodes = (0..MAX_GROUP_AGENT_GRAPH_NODES)
        .map(|index| graph_node(index, project_id))
        .collect::<Vec<_>>();
    let edges = (1..MAX_GROUP_AGENT_GRAPH_NODES)
        .map(|index| GroupAgentGraphEdge {
            from_node_id: node_id(index - 1),
            to_node_id: node_id(index),
        })
        .collect::<Vec<_>>();
    let waves = compute_group_agent_graph_waves(&nodes, &edges).expect("bound serial DAG waves");
    GroupAgentGraphManifest {
        v: GROUP_AGENT_GRAPH_VERSION,
        source: graph_source(snapshot),
        manager: GroupAgentGraphManager {
            agent_profile: "integration-manager".into(),
            instruction: "coordinate the bounded serial graph".into(),
        },
        nodes,
        edges,
        waves,
    }
}

fn graph_node(index: usize, project_id: &str) -> GroupAgentGraphNode {
    let id = node_id(index);
    GroupAgentGraphNode {
        node_id: id.clone(),
        project_id: project_id.into(),
        member_role: "worker".into(),
        agent_profile: "implementer".into(),
        task: format!("complete {id}"),
        acceptance: format!("{id} contract passes"),
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

pub(super) fn node_id(index: usize) -> String {
    format!("node-{index:02}")
}
