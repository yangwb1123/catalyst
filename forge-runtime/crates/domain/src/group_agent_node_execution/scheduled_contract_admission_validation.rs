use super::{
    AdmitGroupAgentScheduledNodeContractCandidate, GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeContractRecord, GroupAgentScheduledNodeContractValidationError,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES, group_agent_scheduled_node_user_prompt,
    validation::{digest, invalid},
};
use crate::{
    GroupAgentGraphControlSnapshot, GroupAgentGraphExecutionSchedule,
    MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES, group_agent_node_system_prompt,
};

pub(super) fn validate_against_sources(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    control: &GroupAgentGraphControlSnapshot,
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    candidate.validate()?;
    control
        .validate()
        .map_err(|_| invalid("scheduled contract control is invalid"))?;
    schedule
        .validate_against_control(control)
        .map_err(|_| invalid("scheduled contract schedule is invalid"))?;
    validate_source_bindings(candidate, control, schedule)?;
    validate_initial_node(candidate, control, schedule)
}

fn validate_source_bindings(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    control: &GroupAgentGraphControlSnapshot,
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let valid = candidate.scheduler_protocol_version == control.scheduler_protocol_version
        && candidate.execution_schedule_protocol_version
            == schedule.execution_schedule_protocol_version
        && candidate.graph_run_id == control.graph_run_id
        && candidate.graph_id == control.graph_id
        && candidate.source_snapshot_sha256 == control.source_snapshot_sha256
        && candidate.graph_manifest_sha256 == control.graph_manifest_sha256
        && candidate.core_plan_sha256 == control.core_plan_sha256
        && candidate.control_snapshot_sha256 == control.snapshot_sha256
        && candidate.schedule_id == schedule.schedule_id
        && candidate.schedule_sha256 == schedule.schedule_sha256
        && candidate.expected_last_event_seq == control.last_event_seq
        && candidate.expected_last_event_seq == schedule.expected_last_event_seq
        && candidate.expected_last_event_sha256 == control.last_event_sha256
        && candidate.expected_last_event_sha256 == schedule.expected_last_event_sha256;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled contract source bindings disagree"))
}

fn validate_initial_node(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    control: &GroupAgentGraphControlSnapshot,
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let scheduled = schedule
        .nodes
        .first()
        .ok_or_else(|| invalid("execution schedule has no initial node"))?;
    let source = control
        .manifest
        .nodes
        .iter()
        .find(|node| node.node_id == scheduled.node_id)
        .ok_or_else(|| invalid("scheduled initial node is absent from the manifest"))?;
    let node = &candidate.node;
    let request = &candidate.request;
    let prompts_match = request.system_prompt
        == group_agent_node_system_prompt(&control.manifest.manager.instruction)
        && request.user_prompt
            == group_agent_scheduled_node_user_prompt(
                &source.node_id,
                &source.task,
                &source.acceptance,
            )?;
    let valid = scheduled.execution_ordinal == 0
        && scheduled.direct_predecessor_node_ids.is_empty()
        && schedule.initial_node == scheduled.node_id
        && node.execution_ordinal == scheduled.execution_ordinal
        && node.node_id == scheduled.node_id
        && node.authored_node_index == scheduled.authored_node_index
        && node.topology_wave_index == scheduled.topology_wave_index
        && node.attempt == scheduled.attempt
        && node.project_id == source.project_id
        && node.member_role == source.member_role
        && node.agent_profile == source.agent_profile
        && node.project_lane_sha256 == scheduled.project_lane_sha256
        && prompts_match;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled contract initial-node binding disagrees"))
}

pub(super) fn validate_record(
    record: &GroupAgentScheduledNodeContractRecord,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    let valid = record.v == GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION
        && content_id(
            &record.contract_id,
            "scheduled-node-contract-",
            &record.contract_sha256,
        )
        && super::super::validation::valid_identifier(&record.graph_run_id)
        && content_id(
            &record.schedule_id,
            "graph-execution-schedule-",
            &record.schedule_sha256,
        )
        && super::super::validation::valid_identifier(&record.node_id)
        && record.execution_ordinal == 0
        && record.attempt == 1
        && digest(&record.control_snapshot_sha256)
        && (1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES).contains(&record.contract_bytes)
        && content_id(
            &record.request_id,
            "scheduled-node-request-",
            &record.request_sha256,
        )
        && digest(&record.project_lane_sha256)
        && record.expected_last_event_seq == 1
        && digest(&record.expected_last_event_sha256)
        && record.predecessor_receipt_count == 0
        && !record.lifecycle_contract_admitted
        && !record.provider_request_present
        && !record.execution_authority_released
        && !record.dispatch_authority_released
        && !record.progress_observed
        && !record.successor_advance_authorized
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid scheduled contract record"))
}

pub(super) fn validate_admission(
    request: &AdmitGroupAgentScheduledNodeContractCandidate,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    request
        .candidate
        .validate_against_control_and_schedule(&request.control_snapshot, &request.schedule)?;
    let valid = request.v == GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION
        && request.graph_run_id == request.control_snapshot.graph_run_id
        && request.graph_run_id == request.schedule.graph_run_id
        && request.graph_run_id == request.candidate.graph_run_id
        && request.control_snapshot.canonical_json().as_deref()
            == Ok(request.control_snapshot_json.as_str())
        && request.schedule.canonical_json().as_deref() == Ok(request.schedule_json.as_str())
        && request.candidate.canonical_json().as_deref() == Ok(request.candidate_json.as_str())
        && (1..=MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES)
            .contains(&request.candidate_json.len())
        && super::super::validation::valid_text(
            &request.idempotency_key,
            MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.admitted_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid exact scheduled contract admission"))
}

pub(super) fn validate_inspection(
    inspection: &GroupAgentScheduledNodeContractInspection,
) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
    inspection.record.validate()?;
    let decoded =
        GroupAgentScheduledNodeContractCandidate::decode_exact(&inspection.candidate_json)?;
    let candidate = &inspection.candidate;
    candidate.validate()?;
    let record = &inspection.record;
    let valid = inspection.v == GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION
        && decoded == *candidate
        && record.contract_id == candidate.contract_id
        && record.graph_run_id == candidate.graph_run_id
        && record.schedule_id == candidate.schedule_id
        && record.node_id == candidate.node.node_id
        && record.execution_ordinal == candidate.node.execution_ordinal
        && record.attempt == candidate.node.attempt
        && record.control_snapshot_sha256 == candidate.control_snapshot_sha256
        && record.schedule_sha256 == candidate.schedule_sha256
        && record.contract_sha256 == candidate.contract_sha256
        && record.contract_bytes == inspection.candidate_json.len()
        && record.request_id == candidate.request.request_id
        && record.request_sha256 == candidate.request.request_sha256
        && record.project_lane_sha256 == candidate.node.project_lane_sha256
        && record.expected_last_event_seq == candidate.expected_last_event_seq
        && record.expected_last_event_sha256 == candidate.expected_last_event_sha256
        && record.predecessor_receipt_count
            == candidate.request.predecessor_terminal_receipts.len()
        && flags_match(record, candidate);
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled contract inspection bindings disagree"))
}

fn flags_match(
    record: &GroupAgentScheduledNodeContractRecord,
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> bool {
    record.lifecycle_contract_admitted == candidate.lifecycle_contract_admitted
        && record.provider_request_present == candidate.provider_request_present
        && record.execution_authority_released == candidate.execution_authority_released
        && record.dispatch_authority_released == candidate.dispatch_authority_released
        && record.progress_observed == candidate.progress_observed
        && record.successor_advance_authorized == candidate.successor_advance_authorized
}

fn content_id(value: &str, prefix: &str, sha256: &str) -> bool {
    digest(sha256)
        && super::super::validation::valid_identifier(value)
        && value == format!("{prefix}{sha256}")
}
