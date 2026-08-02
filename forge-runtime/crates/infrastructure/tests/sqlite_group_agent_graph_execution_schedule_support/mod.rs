use std::collections::BTreeMap;

use forge_runtime_domain::{
    AdmitGroupAgentGraphExecutionSchedule, GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION,
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION,
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION, GROUP_AGENT_GRAPH_RUN_VERSION,
    GroupAgentGraphCompletedOutcomePolicy, GroupAgentGraphControlSnapshot,
    GroupAgentGraphDispatchUnknownOutcomePolicy, GroupAgentGraphExecutionAttemptPolicy,
    GroupAgentGraphExecutionFailurePolicy, GroupAgentGraphExecutionMode,
    GroupAgentGraphExecutionProgressionPolicy, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleNode, GroupAgentGraphExecutionScheduleOutcomePolicy,
    GroupAgentGraphExecutionSelectionPolicy, GroupAgentGraphInspection,
    GroupAgentGraphLengthOutcomePolicy, GroupAgentGraphPredecessorDataflow,
    GroupAgentGraphPredecessorSemantics, GroupAgentGraphReceiptHandling,
    GroupAgentGraphRunInspection, GroupAgentGraphRunStore, GroupAgentGraphUncertaintyOutcomePolicy,
    group_agent_project_lane_sha256,
};

use super::sqlite_group_agent_graph_run_support::Fixture;

pub fn prepared_fixture() -> Fixture {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-1", "run-key", 30))
        .expect("seed schedule Graph Run");
    fixture
}

pub fn request(
    fixture: &Fixture,
    key: &str,
    admitted_at_ms: u64,
) -> AdmitGroupAgentGraphExecutionSchedule {
    request_for_run(fixture, "graph-run-1", key, admitted_at_ms)
}

pub fn request_for_run(
    fixture: &Fixture,
    graph_run_id: &str,
    key: &str,
    admitted_at_ms: u64,
) -> AdmitGroupAgentGraphExecutionSchedule {
    let run = fixture
        .store
        .inspect_group_agent_graph_run(graph_run_id)
        .expect("inspect schedule source Run");
    let control = control_snapshot(&run, &fixture.graph);
    let schedule = execution_schedule(&control);
    AdmitGroupAgentGraphExecutionSchedule {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        graph_run_id: graph_run_id.into(),
        control_snapshot_json: control.canonical_json().expect("canonical control"),
        control_snapshot: control,
        schedule_json: schedule.canonical_json().expect("canonical schedule"),
        schedule,
        idempotency_key: key.into(),
        admitted_at_ms,
    }
}

pub fn recanonicalize(request: &mut AdmitGroupAgentGraphExecutionSchedule) {
    request.schedule.control_snapshot_sha256 = request.control_snapshot.snapshot_sha256.clone();
    request.schedule.expected_last_event_seq = request.control_snapshot.last_event_seq;
    request.schedule.expected_last_event_sha256 =
        request.control_snapshot.last_event_sha256.clone();
    request.schedule.schedule_sha256 = request.schedule.expected_sha256().expect("schedule digest");
    request.schedule.schedule_id = format!(
        "graph-execution-schedule-{}",
        request.schedule.schedule_sha256
    );
    request.schedule_json = request
        .schedule
        .canonical_json()
        .expect("canonical schedule");
}

fn control_snapshot(
    run: &GroupAgentGraphRunInspection,
    graph: &GroupAgentGraphInspection,
) -> GroupAgentGraphControlSnapshot {
    let mut snapshot = GroupAgentGraphControlSnapshot {
        v: GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION,
        scheduler_protocol_version: run.run.scheduler_protocol_version,
        graph_run_version: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: run.run.graph_run_id.clone(),
        graph_id: run.run.graph_id.clone(),
        source_snapshot_sha256: run.run.source_snapshot_sha256.clone(),
        graph_manifest_sha256: run.run.graph_manifest_sha256.clone(),
        core_plan_sha256: run.run.plan_sha256.clone(),
        last_event_seq: 1,
        last_event_sha256: run.events[0].expected_sha256().expect("event head"),
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan: run.plan.clone(),
        manifest: graph.manifest.clone(),
        snapshot_sha256: String::new(),
    };
    snapshot.snapshot_sha256 = snapshot.expected_sha256().expect("control digest");
    snapshot
}

fn execution_schedule(
    control: &GroupAgentGraphControlSnapshot,
) -> GroupAgentGraphExecutionSchedule {
    let mut schedule = schedule_without_identity(control);
    schedule.schedule_sha256 = schedule.expected_sha256().expect("schedule digest");
    schedule.schedule_id = format!("graph-execution-schedule-{}", schedule.schedule_sha256);
    schedule
}

fn schedule_without_identity(
    control: &GroupAgentGraphControlSnapshot,
) -> GroupAgentGraphExecutionSchedule {
    GroupAgentGraphExecutionSchedule {
        v: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        execution_schedule_protocol_version: GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION,
        control_snapshot_sha256: control.snapshot_sha256.clone(),
        expected_last_event_seq: control.last_event_seq,
        expected_last_event_sha256: control.last_event_sha256.clone(),
        graph_run_id: control.graph_run_id.clone(),
        graph_id: control.graph_id.clone(),
        source_snapshot_sha256: control.source_snapshot_sha256.clone(),
        graph_manifest_sha256: control.graph_manifest_sha256.clone(),
        core_plan_sha256: control.core_plan_sha256.clone(),
        node_count: control.plan.authored_node_ids.len(),
        wave_count: control.plan.waves.len(),
        execution_mode: GroupAgentGraphExecutionMode::Serial,
        max_in_flight_nodes: 1,
        selection_policy: GroupAgentGraphExecutionSelectionPolicy::TopologyWaveThenAuthoredOrder,
        progression_policy: GroupAgentGraphExecutionProgressionPolicy::CompletedContiguousPrefix,
        attempt_policy: GroupAgentGraphExecutionAttemptPolicy::ExactlyOne,
        failure_policy: GroupAgentGraphExecutionFailurePolicy::FailFastNoRetry,
        outcome_policy: outcome_policy(),
        predecessor_semantics: GroupAgentGraphPredecessorSemantics::OrderingOnly,
        predecessor_dataflow: GroupAgentGraphPredecessorDataflow::None,
        partial_output_dataflow: false,
        receipt_handling: GroupAgentGraphReceiptHandling::FutureVerifiedIdentitySlots,
        nodes: schedule_nodes(control),
        initial_frontier: control.plan.waves[0].clone(),
        initial_node: control.plan.waves[0][0].clone(),
        execution_contract_present: false,
        dispatch_authority_released: false,
        progress_observed: false,
        successor_advanced: false,
        schedule_id: String::new(),
        schedule_sha256: String::new(),
    }
}

fn schedule_nodes(
    control: &GroupAgentGraphControlSnapshot,
) -> Vec<GroupAgentGraphExecutionScheduleNode> {
    let authored = control
        .plan
        .authored_node_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    control
        .plan
        .waves
        .iter()
        .enumerate()
        .flat_map(|(wave, nodes)| nodes.iter().map(move |node| (wave, node)))
        .enumerate()
        .map(|(ordinal, (wave, node))| schedule_node(control, &authored, ordinal, wave, node))
        .collect()
}

fn schedule_node(
    control: &GroupAgentGraphControlSnapshot,
    authored: &BTreeMap<&str, usize>,
    ordinal: usize,
    wave: usize,
    node_id: &str,
) -> GroupAgentGraphExecutionScheduleNode {
    let manifest = control
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == node_id)
        .expect("manifest node");
    let predecessors =
        control
            .plan
            .authored_node_ids
            .iter()
            .filter(|candidate| {
                control.plan.edges.iter().any(|edge| {
                    edge.from_node_id == candidate.as_str() && edge.to_node_id == node_id
                })
            })
            .cloned()
            .collect();
    GroupAgentGraphExecutionScheduleNode {
        execution_ordinal: ordinal,
        node_id: node_id.into(),
        authored_node_index: authored[node_id],
        topology_wave_index: wave,
        project_lane_sha256: group_agent_project_lane_sha256(&manifest.project_id),
        attempt: 1,
        direct_predecessor_node_ids: predecessors,
    }
}

fn outcome_policy() -> GroupAgentGraphExecutionScheduleOutcomePolicy {
    GroupAgentGraphExecutionScheduleOutcomePolicy {
        completed: GroupAgentGraphCompletedOutcomePolicy::AdvanceOrComplete,
        length: GroupAgentGraphLengthOutcomePolicy::FailGraph,
        uncertainty: GroupAgentGraphUncertaintyOutcomePolicy::FailGraphUncertain,
        dispatch_unknown: GroupAgentGraphDispatchUnknownOutcomePolicy::QuarantineNoAdvance,
    }
}
