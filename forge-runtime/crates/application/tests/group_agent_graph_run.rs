#[path = "group_agent_graph_support/graph_runs.rs"]
mod graph_runs;
#[allow(dead_code, unused_imports)]
mod group_agent_graph_support;

use std::sync::Arc;

use forge_runtime_application::{
    BeginGroupAgentGraphRunDisposition, GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
    GROUP_AGENT_GRAPH_RUN_VERSION, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GroupAgentGraphCorePlan, GroupAgentGraphRunService, GroupAgentGraphRunServiceError,
    GroupAgentGraphRunStatus, PrepareGroupAgentGraphRunInput,
};
use forge_runtime_domain::GroupAgentGraphInspection;

use graph_runs::MemoryGraphRunStore;
use group_agent_graph_support::{
    Harness as GraphHarness, harness as graph_harness, prepare_input as graph_input,
};

struct RunHarness {
    service: GroupAgentGraphRunService,
    graph_source: GraphHarness,
    runs: Arc<MemoryGraphRunStore>,
    graph: GroupAgentGraphInspection,
}

#[test]
fn prepare_admits_only_one_passive_plan_and_event_receipt() {
    let harness = run_harness();
    let input = run_input(&harness.graph);
    let result = harness.service.prepare(&input).expect("prepare Graph Run");

    assert_eq!(
        result.disposition,
        BeginGroupAgentGraphRunDisposition::Created
    );
    assert_eq!(
        result.inspection.run.status,
        GroupAgentGraphRunStatus::AwaitingExecutionContract
    );
    assert!(!result.inspection.plan.execution_contract_present);
    assert!(!result.inspection.plan.dispatch_authority_released);
    assert_eq!(result.inspection.run.last_event_seq, 1);
    assert_eq!(result.inspection.events[0].seq, 1);
    assert_eq!(result.inspection.run.node_count, 4);
    assert_eq!(result.inspection.run.wave_count, 3);
    assert_request_bindings(&harness, &input);
}

#[test]
fn malformed_noncanonical_and_unknown_plans_never_reach_the_store() {
    let base = run_harness();
    let canonical = run_input(&base.graph).plan_json;
    let cases = [
        format!("{canonical}\n"),
        canonical.replacen("\"v\":1", "\"v\":1,\"unknown\":true", 1),
        canonical.replacen("\"plan_sha256\":\"", "\"plan_sha256\":\"0", 1),
    ];

    for plan_json in cases {
        let harness = run_harness();
        let mut input = run_input(&harness.graph);
        input.plan_json = plan_json;
        assert!(matches!(
            harness.service.prepare(&input),
            Err(GroupAgentGraphRunServiceError::InvalidPlan)
        ));
        assert!(harness.runs.last_request().is_none());
    }
}

#[test]
fn valid_but_divergent_graph_bindings_never_reach_the_store() {
    let harness = run_harness();
    let mut manifest_drift = plan_for(&harness.graph);
    manifest_drift.graph_manifest_sha256 = "a".repeat(64);
    rehash(&mut manifest_drift);
    assert_graph_drift_rejected(&harness, &manifest_drift);

    let harness = run_harness();
    let mut authored_order = plan_for(&harness.graph);
    authored_order.authored_node_ids.swap(0, 1);
    authored_order.waves[1].swap(0, 1);
    rehash(&mut authored_order);
    authored_order
        .validate()
        .expect("internally valid divergent plan");
    assert_graph_drift_rejected(&harness, &authored_order);
}

#[test]
fn exact_replay_retains_the_original_run_identity_time_plan_and_event() {
    let harness = run_harness();
    let input = run_input(&harness.graph);
    let created = harness.service.prepare(&input).expect("created Graph Run");
    let mut retry = input;
    retry.graph_run_id = "ignored-run-candidate".into();
    retry.created_at_ms = 999;
    let replayed = harness.service.prepare(&retry).expect("replayed Graph Run");

    assert_eq!(
        replayed.disposition,
        BeginGroupAgentGraphRunDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_eq!(replayed.inspection.run.graph_run_id, "graph-run-1");
    assert_eq!(replayed.inspection.run.created_at_ms, 73);
}

#[test]
fn prepare_fails_closed_on_an_inconsistent_store_result() {
    let harness = run_harness();
    harness.runs.corrupt_next_prepare();

    assert!(matches!(
        harness.service.prepare(&run_input(&harness.graph)),
        Err(GroupAgentGraphRunServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn inspect_revalidates_run_bytes_and_the_exact_source_graph() {
    let harness = run_harness();
    harness
        .service
        .prepare(&run_input(&harness.graph))
        .expect("prepare Graph Run");
    harness
        .service
        .inspect("graph-run-1")
        .expect("inspect Graph Run");

    let mut corrupt_run = harness.runs.inspection("graph-run-1");
    corrupt_run.plan_json.push(' ');
    harness.runs.replace("graph-run-1", corrupt_run);
    assert!(matches!(
        harness.service.inspect("graph-run-1"),
        Err(GroupAgentGraphRunServiceError::InconsistentStoreResult)
    ));

    let harness = prepared_harness();
    let mut corrupt_graph = harness.graph.clone();
    corrupt_graph.manifest.nodes[0]
        .task
        .push_str(" unbound drift");
    harness
        .graph_source
        .graphs
        .replace(&corrupt_graph.graph.graph_id.clone(), corrupt_graph);
    assert!(matches!(
        harness.service.inspect("graph-run-1"),
        Err(GroupAgentGraphRunServiceError::InvalidGraph)
    ));
}

#[test]
fn list_is_bounded_filtered_unique_and_metadata_only() {
    let harness = prepared_harness();
    let records = harness
        .service
        .list(Some(&harness.graph.graph.graph_id), 10)
        .expect("list Graph Runs");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].graph_run_id, "graph-run-1");

    assert!(matches!(
        harness.service.list(None, 0),
        Err(GroupAgentGraphRunServiceError::InvalidInput)
    ));
    let duplicate = vec![records[0].clone(), records[0].clone()];
    harness.runs.set_list(duplicate);
    assert!(matches!(
        harness.service.list(None, 10),
        Err(GroupAgentGraphRunServiceError::InconsistentStoreResult)
    ));
}

fn run_harness() -> RunHarness {
    let graph_source = graph_harness();
    let graph = graph_source
        .service
        .prepare(&graph_input())
        .expect("prepare source Graph")
        .inspection;
    let runs = Arc::new(MemoryGraphRunStore::default());
    let service = GroupAgentGraphRunService::new(graph_source.graphs.clone(), runs.clone());
    RunHarness {
        service,
        graph_source,
        runs,
        graph,
    }
}

fn prepared_harness() -> RunHarness {
    let harness = run_harness();
    harness
        .service
        .prepare(&run_input(&harness.graph))
        .expect("prepare Graph Run");
    harness
}

fn run_input(graph: &GroupAgentGraphInspection) -> PrepareGroupAgentGraphRunInput {
    let plan = plan_for(graph);
    PrepareGroupAgentGraphRunInput {
        graph_run_id: "graph-run-1".into(),
        graph_id: graph.graph.graph_id.clone(),
        plan_json: plan.canonical_json().expect("canonical plan"),
        idempotency_key: "graph-run-key".into(),
        created_at_ms: 73,
    }
}

fn plan_for(graph: &GroupAgentGraphInspection) -> GroupAgentGraphCorePlan {
    let mut plan = GroupAgentGraphCorePlan {
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
    };
    rehash(&mut plan);
    plan
}

fn rehash(plan: &mut GroupAgentGraphCorePlan) {
    plan.plan_sha256 = plan.expected_sha256().expect("plan digest");
}

fn assert_graph_drift_rejected(harness: &RunHarness, plan: &GroupAgentGraphCorePlan) {
    let mut input = run_input(&harness.graph);
    input.plan_json = plan.canonical_json().expect("canonical divergent plan");
    assert!(matches!(
        harness.service.prepare(&input),
        Err(GroupAgentGraphRunServiceError::InvalidGraph)
    ));
    assert!(harness.runs.last_request().is_none());
}

fn assert_request_bindings(harness: &RunHarness, input: &PrepareGroupAgentGraphRunInput) {
    let request = harness.runs.last_request().expect("captured request");
    assert_eq!(request.v, GROUP_AGENT_GRAPH_RUN_VERSION);
    assert_eq!(request.graph_run_id, input.graph_run_id);
    assert_eq!(
        request.source_snapshot_sha256,
        harness.graph.graph.source_snapshot_sha256
    );
    assert_eq!(
        request.graph_manifest_sha256,
        harness.graph.graph.manifest_sha256
    );
    assert_eq!(request.plan_json, input.plan_json);
    assert_eq!(
        request.event_json,
        request.event.canonical_json().expect("canonical event")
    );
    assert_eq!(request.event.graph_run_id, input.graph_run_id);
    assert_eq!(request.created_at_ms, input.created_at_ms);
}
