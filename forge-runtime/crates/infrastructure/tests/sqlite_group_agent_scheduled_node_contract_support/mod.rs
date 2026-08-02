use forge_runtime_domain::{
    AdmitGroupAgentScheduledNodeContractCandidate, GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION, GroupAgentGraphExecutionScheduleStore,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeRequest,
    group_agent_node_system_prompt, group_agent_prompt_sha256,
    group_agent_scheduled_node_user_prompt,
};

use super::{
    sqlite_group_agent_graph_execution_schedule_support as schedule_support,
    sqlite_group_agent_graph_run_support::Fixture,
};

pub fn prepared_fixture() -> (Fixture, AdmitGroupAgentScheduledNodeContractCandidate) {
    let fixture = schedule_support::prepared_fixture();
    let schedule = schedule_support::request(&fixture, "schedule-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit candidate source schedule");
    let request = admission(schedule, "scheduled-contract-key", 50);
    (fixture, request)
}

pub fn admission(
    schedule: forge_runtime_domain::AdmitGroupAgentGraphExecutionSchedule,
    key: &str,
    admitted_at_ms: u64,
) -> AdmitGroupAgentScheduledNodeContractCandidate {
    let scheduled = &schedule.schedule.nodes[0];
    let source = schedule
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == scheduled.node_id)
        .expect("scheduled source node");
    let mut candidate = candidate_template();
    bind_sources(&mut candidate, &schedule);
    bind_node(&mut candidate, scheduled, source);
    candidate.request = scheduled_request(&schedule, source);
    candidate.contract_sha256 = candidate.expected_sha256().expect("candidate digest");
    candidate.contract_id = format!("scheduled-node-contract-{}", candidate.contract_sha256);
    let candidate_json = candidate.canonical_json().expect("canonical candidate");
    let request = AdmitGroupAgentScheduledNodeContractCandidate {
        v: GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
        graph_run_id: candidate.graph_run_id.clone(),
        control_snapshot_json: schedule.control_snapshot_json,
        control_snapshot: schedule.control_snapshot,
        schedule_json: schedule.schedule_json,
        schedule: schedule.schedule,
        candidate,
        candidate_json,
        idempotency_key: key.into(),
        admitted_at_ms,
    };
    request
        .validate()
        .expect("valid scheduled candidate admission");
    request
}

fn candidate_template() -> GroupAgentScheduledNodeContractCandidate {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .expect("scheduled candidate fixture");
    let json = fixture["expected"]["canonical_contract_json"]
        .as_str()
        .expect("canonical scheduled candidate fixture");
    GroupAgentScheduledNodeContractCandidate::decode_exact(json)
        .expect("decode scheduled candidate template")
}

fn bind_sources(
    candidate: &mut GroupAgentScheduledNodeContractCandidate,
    schedule: &forge_runtime_domain::AdmitGroupAgentGraphExecutionSchedule,
) {
    let control = &schedule.control_snapshot;
    candidate.scheduler_protocol_version = control.scheduler_protocol_version;
    candidate.node_execution_protocol_version =
        GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION;
    candidate.execution_schedule_protocol_version =
        schedule.schedule.execution_schedule_protocol_version;
    candidate.graph_run_id.clone_from(&control.graph_run_id);
    candidate.graph_id.clone_from(&control.graph_id);
    candidate
        .source_snapshot_sha256
        .clone_from(&control.source_snapshot_sha256);
    candidate
        .graph_manifest_sha256
        .clone_from(&control.graph_manifest_sha256);
    candidate
        .core_plan_sha256
        .clone_from(&control.core_plan_sha256);
    candidate
        .control_snapshot_sha256
        .clone_from(&control.snapshot_sha256);
    candidate
        .schedule_id
        .clone_from(&schedule.schedule.schedule_id);
    candidate
        .schedule_sha256
        .clone_from(&schedule.schedule.schedule_sha256);
    candidate.expected_last_event_seq = control.last_event_seq;
    candidate
        .expected_last_event_sha256
        .clone_from(&control.last_event_sha256);
}

fn bind_node(
    candidate: &mut GroupAgentScheduledNodeContractCandidate,
    scheduled: &forge_runtime_domain::GroupAgentGraphExecutionScheduleNode,
    source: &forge_runtime_domain::GroupAgentGraphNode,
) {
    candidate.node.execution_ordinal = scheduled.execution_ordinal;
    candidate.node.node_id.clone_from(&source.node_id);
    candidate.node.authored_node_index = scheduled.authored_node_index;
    candidate.node.topology_wave_index = scheduled.topology_wave_index;
    candidate.node.attempt = scheduled.attempt;
    candidate.node.project_id.clone_from(&source.project_id);
    candidate.node.member_role.clone_from(&source.member_role);
    candidate
        .node
        .agent_profile
        .clone_from(&source.agent_profile);
    candidate
        .node
        .project_lane_sha256
        .clone_from(&scheduled.project_lane_sha256);
}

fn scheduled_request(
    schedule: &forge_runtime_domain::AdmitGroupAgentGraphExecutionSchedule,
    source: &forge_runtime_domain::GroupAgentGraphNode,
) -> GroupAgentScheduledNodeRequest {
    let node = &schedule.schedule.nodes[0];
    let system_prompt =
        group_agent_node_system_prompt(&schedule.control_snapshot.manifest.manager.instruction);
    let user_prompt =
        group_agent_scheduled_node_user_prompt(&source.node_id, &source.task, &source.acceptance)
            .expect("canonical scheduled-node user Prompt");
    let mut request = GroupAgentScheduledNodeRequest {
        v: GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION,
        graph_run_id: schedule.schedule.graph_run_id.clone(),
        schedule_id: schedule.schedule.schedule_id.clone(),
        schedule_sha256: schedule.schedule.schedule_sha256.clone(),
        execution_ordinal: node.execution_ordinal,
        node_id: node.node_id.clone(),
        attempt: node.attempt,
        system_prompt_bytes: system_prompt.len(),
        system_prompt_sha256: group_agent_prompt_sha256(&system_prompt),
        system_prompt,
        user_prompt_bytes: user_prompt.len(),
        user_prompt_sha256: group_agent_prompt_sha256(&user_prompt),
        user_prompt,
        required_predecessor_node_ids: Vec::new(),
        predecessor_terminal_receipts: Vec::new(),
        predecessor_content_included: false,
        tools: Vec::new(),
        request_id: String::new(),
        request_sha256: String::new(),
    };
    request.request_sha256 = request.expected_sha256().expect("scheduled request digest");
    request.request_id = format!("scheduled-node-request-{}", request.request_sha256);
    request
}
