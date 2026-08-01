mod group_agent_graph_support;

use std::{fs, sync::Arc};

use forge_runtime_application::{
    GroupAgentGraphService, GroupAgentGraphServiceError, PrepareGroupAgentGraphInput,
};
use forge_runtime_domain::{
    GroupAgentGraphEdge, GroupContextPolicy, GroupRunStore, HubEntity, HubStore, HubStoreError,
    MAX_GROUP_AGENT_GRAPH_LIST_LIMIT, PrepareGroupAgentGraphDisposition, PrepareGroupRun,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

use group_agent_graph_support::{
    GROUP_RUN_ID, corrupt_snapshot, harness, nodes, prepare_input, rebind_snapshot,
};

#[test]
fn prepare_canonicalizes_edges_and_builds_authored_order_waves() {
    let harness = harness();
    let result = harness
        .service
        .prepare(&prepare_input())
        .expect("prepare graph");
    assert_eq!(
        result.disposition,
        PrepareGroupAgentGraphDisposition::Created
    );
    let manifest = &result.inspection.manifest;
    assert_eq!(
        manifest
            .edges
            .iter()
            .map(|edge| format!("{}>{}", edge.from_node_id, edge.to_node_id))
            .collect::<Vec<_>>(),
        [
            "backend-integration>release-check",
            "frontend-integration>release-check",
            "sso-contract>backend-integration",
            "sso-contract>frontend-integration",
        ]
    );
    assert_eq!(
        manifest.waves,
        vec![
            vec!["sso-contract".to_string()],
            vec![
                "frontend-integration".to_string(),
                "backend-integration".to_string(),
            ],
            vec!["release-check".to_string()],
        ]
    );
    assert!(manifest.manager.instruction.contains('\n'));
    assert!(manifest.nodes[0].task.contains('\n'));
    assert_same_project_nodes_allowed(manifest);
    assert_eq!(
        harness
            .graphs
            .last_request()
            .expect("captured request")
            .manifest_json,
        result.inspection.manifest_json
    );
}

fn assert_same_project_nodes_allowed(manifest: &forge_runtime_domain::GroupAgentGraphManifest) {
    let count = manifest
        .nodes
        .iter()
        .filter(|node| node.project_id == "project-frontend")
        .count();
    assert_eq!(
        count, 2,
        "one frozen project member may own multiple authored nodes"
    );
}

#[test]
fn prepare_rejects_bad_graphs_before_persistence() {
    let cases = [
        duplicate_node_input(),
        duplicate_edge_input(),
        self_edge_input(),
        unknown_edge_input(),
        cycle_input(),
    ];
    for input in cases {
        let harness = harness();
        assert!(matches!(
            harness.service.prepare(&input),
            Err(GroupAgentGraphServiceError::InvalidInput)
        ));
        assert!(harness.graphs.last_request().is_none());
    }
}

#[test]
fn prepare_requires_exact_frozen_project_role_membership() {
    let mut absent = prepare_input();
    absent.nodes[0].project_id = "project-absent".into();
    let mut wrong_role = prepare_input();
    wrong_role.nodes[0].member_role = "identity".into();
    for input in [absent, wrong_role] {
        let harness = harness();
        assert!(matches!(
            harness.service.prepare(&input),
            Err(GroupAgentGraphServiceError::InvalidInput)
        ));
        assert!(harness.graphs.last_request().is_none());
    }
}

#[test]
fn exact_replay_retains_original_identity_and_time() {
    let harness = harness();
    let created = harness
        .service
        .prepare(&prepare_input())
        .expect("created graph");
    let mut retry = prepare_input();
    retry.graph_id = "ignored-candidate".into();
    retry.created_at_ms = 500;
    retry.edges.reverse();
    let replay = harness.service.prepare(&retry).expect("replayed graph");
    assert_eq!(
        replay.disposition,
        PrepareGroupAgentGraphDisposition::Replayed
    );
    assert_eq!(replay.inspection, created.inspection);

    let mut conflict = retry;
    conflict.manager.instruction.push_str(" changed");
    assert!(matches!(
        harness.service.prepare(&conflict),
        Err(GroupAgentGraphServiceError::Store(
            HubStoreError::Conflict {
                entity: HubEntity::GroupAgentGraph,
                ..
            }
        ))
    ));

    let mut reordered_nodes = prepare_input();
    reordered_nodes.nodes.swap(0, 1);
    assert!(matches!(
        harness.service.prepare(&reordered_nodes),
        Err(GroupAgentGraphServiceError::Store(
            HubStoreError::Conflict {
                entity: HubEntity::GroupAgentGraph,
                ..
            }
        ))
    ));
}

#[test]
fn application_to_sqlite_replays_edge_order_but_conflicts_node_order() {
    let (_root, service, input) = sqlite_graph_harness();
    let created = service.prepare(&input).expect("create graph");
    let mut edge_retry = input.clone();
    edge_retry.edges.reverse();
    edge_retry.graph_id = "ignored-candidate".into();
    edge_retry.created_at_ms = 999;
    assert_eq!(
        service
            .prepare(&edge_retry)
            .expect("edge replay")
            .inspection,
        created.inspection
    );
    let mut node_conflict = input;
    node_conflict.nodes.swap(0, 1);
    assert!(matches!(
        service.prepare(&node_conflict),
        Err(GroupAgentGraphServiceError::Store(
            HubStoreError::Conflict { .. }
        ))
    ));
}

fn sqlite_graph_harness() -> (TempDir, GroupAgentGraphService, PrepareGroupAgentGraphInput) {
    let root = TempDir::new().expect("integration root");
    let store = Arc::new(
        SqliteHubStore::open(root.path().join("state").join("hub.sqlite3")).expect("open Hub"),
    );
    let group = store
        .create_group("Graph integration", "group-key")
        .expect("create Group");
    let frontend = link_member(&store, root.path(), &group.id, "frontend");
    let backend = link_member(&store, root.path(), &group.id, "backend");
    let identity = link_member(&store, root.path(), &group.id, "identity");
    store
        .prepare_group_run(&PrepareGroupRun {
            v: 1,
            run_id: GROUP_RUN_ID.into(),
            group_id: group.id,
            policy: GroupContextPolicy::default(),
            idempotency_key: "group-run-key".into(),
            created_at_ms: 5,
        })
        .expect("prepare Group Run");
    let mut input = prepare_input();
    for node in &mut input.nodes {
        node.project_id = match node.member_role.as_str() {
            "frontend" => frontend.clone(),
            "backend" => backend.clone(),
            "identity" => identity.clone(),
            role => panic!("unexpected role {role}"),
        };
    }
    let service = GroupAgentGraphService::new(store.clone(), store);
    (root, service, input)
}

fn link_member(
    store: &SqliteHubStore,
    root: &std::path::Path,
    group_id: &str,
    role: &str,
) -> String {
    let path = root.join(role);
    fs::create_dir(&path).expect("member directory");
    let project = store
        .open_project(&path.canonicalize().expect("canonical member"))
        .expect("open Project");
    store
        .add_project_to_group(group_id, &project.id, role, &format!("link-{role}"))
        .expect("link member");
    project.id
}

#[test]
fn prepare_rejects_a_corrupt_store_result() {
    let harness = harness();
    harness.graphs.corrupt_next_prepare();
    assert!(matches!(
        harness.service.prepare(&prepare_input()),
        Err(GroupAgentGraphServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn inspect_revalidates_canonical_bytes_and_exact_source() {
    let byte_case = harness();
    byte_case
        .service
        .prepare(&prepare_input())
        .expect("prepare graph");
    let mut bad_bytes = byte_case.graphs.inspection("graph-1");
    bad_bytes.manifest_json.push(' ');
    byte_case.graphs.replace("graph-1", bad_bytes);
    assert!(matches!(
        byte_case.service.inspect("graph-1"),
        Err(GroupAgentGraphServiceError::InconsistentStoreResult)
    ));

    let rebound = harness();
    rebound
        .service
        .prepare(&prepare_input())
        .expect("prepare graph");
    let changed = rebind_snapshot(rebound.runs.snapshot());
    rebound.runs.respond_with(changed);
    assert!(matches!(
        rebound.service.inspect("graph-1"),
        Err(GroupAgentGraphServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn inspect_rejects_a_corrupt_frozen_source() {
    let harness = harness();
    harness
        .service
        .prepare(&prepare_input())
        .expect("prepare graph");
    let corrupt = corrupt_snapshot(harness.runs.snapshot());
    harness.runs.respond_with(corrupt);
    assert!(matches!(
        harness.service.inspect("graph-1"),
        Err(GroupAgentGraphServiceError::InvalidSource)
    ));
}

#[test]
fn list_is_bounded_filtered_and_rejects_inconsistent_metadata() {
    let harness = harness();
    let prepared = harness
        .service
        .prepare(&prepare_input())
        .expect("prepare graph");
    assert_eq!(
        harness.service.list(Some(GROUP_RUN_ID), 1).expect("list"),
        vec![prepared.inspection.graph.clone()]
    );
    for limit in [0, MAX_GROUP_AGENT_GRAPH_LIST_LIMIT + 1] {
        assert!(matches!(
            harness.service.list(None, limit),
            Err(GroupAgentGraphServiceError::InvalidInput)
        ));
    }
    let record = prepared.inspection.graph;
    harness.graphs.set_list(vec![record.clone(), record]);
    assert!(matches!(
        harness.service.list(None, 2),
        Err(GroupAgentGraphServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn list_rejects_records_outside_the_requested_source() {
    let harness = harness();
    let mut record = harness
        .service
        .prepare(&prepare_input())
        .expect("prepare graph")
        .inspection
        .graph;
    record.group_run_id = "other-run".into();
    harness.graphs.set_list(vec![record]);
    assert!(matches!(
        harness.service.list(Some(GROUP_RUN_ID), 1),
        Err(GroupAgentGraphServiceError::InconsistentStoreResult)
    ));
}

fn duplicate_node_input() -> forge_runtime_application::PrepareGroupAgentGraphInput {
    let mut input = prepare_input();
    input.nodes.push(input.nodes[0].clone());
    input
}

fn duplicate_edge_input() -> forge_runtime_application::PrepareGroupAgentGraphInput {
    let mut input = prepare_input();
    input.edges.push(input.edges[0].clone());
    input
}

fn self_edge_input() -> forge_runtime_application::PrepareGroupAgentGraphInput {
    let mut input = prepare_input();
    input.edges = vec![edge("sso-contract", "sso-contract")];
    input
}

fn unknown_edge_input() -> forge_runtime_application::PrepareGroupAgentGraphInput {
    let mut input = prepare_input();
    input.edges = vec![edge("missing", "sso-contract")];
    input
}

fn cycle_input() -> forge_runtime_application::PrepareGroupAgentGraphInput {
    let mut input = prepare_input();
    input.nodes = nodes().into_iter().take(2).collect();
    input.edges = vec![
        edge("frontend-integration", "backend-integration"),
        edge("backend-integration", "frontend-integration"),
    ];
    input
}

fn edge(from: &str, to: &str) -> GroupAgentGraphEdge {
    GroupAgentGraphEdge {
        from_node_id: from.into(),
        to_node_id: to.into(),
    }
}
