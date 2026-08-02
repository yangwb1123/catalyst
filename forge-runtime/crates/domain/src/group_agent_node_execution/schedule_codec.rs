use serde::Serialize;
use sha2::{Digest, Sha256};

use super::{
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_DIGEST_DOMAIN, GroupAgentGraphExecutionAttemptPolicy,
    GroupAgentGraphExecutionFailurePolicy, GroupAgentGraphExecutionMode,
    GroupAgentGraphExecutionProgressionPolicy, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleNode, GroupAgentGraphExecutionScheduleOutcomePolicy,
    GroupAgentGraphExecutionScheduleValidationError, GroupAgentGraphExecutionSelectionPolicy,
    GroupAgentGraphPredecessorDataflow, GroupAgentGraphPredecessorSemantics,
    GroupAgentGraphReceiptHandling, MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES,
};

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct SchedulePayload<'a> {
    v: u16,
    scheduler_protocol_version: u16,
    execution_schedule_protocol_version: u16,
    control_snapshot_sha256: &'a str,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
    graph_run_id: &'a str,
    graph_id: &'a str,
    source_snapshot_sha256: &'a str,
    graph_manifest_sha256: &'a str,
    core_plan_sha256: &'a str,
    node_count: usize,
    wave_count: usize,
    execution_mode: GroupAgentGraphExecutionMode,
    max_in_flight_nodes: usize,
    selection_policy: GroupAgentGraphExecutionSelectionPolicy,
    progression_policy: GroupAgentGraphExecutionProgressionPolicy,
    attempt_policy: GroupAgentGraphExecutionAttemptPolicy,
    failure_policy: GroupAgentGraphExecutionFailurePolicy,
    outcome_policy: &'a GroupAgentGraphExecutionScheduleOutcomePolicy,
    predecessor_semantics: GroupAgentGraphPredecessorSemantics,
    predecessor_dataflow: GroupAgentGraphPredecessorDataflow,
    partial_output_dataflow: bool,
    receipt_handling: GroupAgentGraphReceiptHandling,
    nodes: &'a [GroupAgentGraphExecutionScheduleNode],
    initial_frontier: &'a [String],
    initial_node: &'a str,
    execution_contract_present: bool,
    dispatch_authority_released: bool,
    progress_observed: bool,
    successor_advanced: bool,
}

pub(super) fn decode_exact(
    bytes: &[u8],
) -> Result<GroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleValidationError> {
    if bytes.is_empty() || bytes.len() > MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES {
        return Err(invalid(
            "execution schedule input is outside its byte bound",
        ));
    }
    let schedule: GroupAgentGraphExecutionSchedule = serde_json::from_slice(bytes)
        .map_err(|_| invalid("execution schedule input is invalid JSON"))?;
    schedule.validate()?;
    if schedule.canonical_json()?.as_bytes() != bytes {
        return Err(invalid(
            "execution schedule input is not exact canonical JSON",
        ));
    }
    Ok(schedule)
}

pub(super) fn canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentGraphExecutionScheduleValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("execution schedule cannot be encoded"))
}

pub(super) fn schedule_digest(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<String, GroupAgentGraphExecutionScheduleValidationError> {
    let json = canonical_json(&payload_from(schedule))?;
    Ok(digest_hex(
        GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_DIGEST_DOMAIN,
        json.as_bytes(),
    ))
}

#[cfg(test)]
pub(super) fn schedule_payload_json(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<String, GroupAgentGraphExecutionScheduleValidationError> {
    canonical_json(&payload_from(schedule))
}

fn payload_from(schedule: &GroupAgentGraphExecutionSchedule) -> SchedulePayload<'_> {
    SchedulePayload {
        v: schedule.v,
        scheduler_protocol_version: schedule.scheduler_protocol_version,
        execution_schedule_protocol_version: schedule.execution_schedule_protocol_version,
        control_snapshot_sha256: &schedule.control_snapshot_sha256,
        expected_last_event_seq: schedule.expected_last_event_seq,
        expected_last_event_sha256: &schedule.expected_last_event_sha256,
        graph_run_id: &schedule.graph_run_id,
        graph_id: &schedule.graph_id,
        source_snapshot_sha256: &schedule.source_snapshot_sha256,
        graph_manifest_sha256: &schedule.graph_manifest_sha256,
        core_plan_sha256: &schedule.core_plan_sha256,
        node_count: schedule.node_count,
        wave_count: schedule.wave_count,
        execution_mode: schedule.execution_mode,
        max_in_flight_nodes: schedule.max_in_flight_nodes,
        selection_policy: schedule.selection_policy,
        progression_policy: schedule.progression_policy,
        attempt_policy: schedule.attempt_policy,
        failure_policy: schedule.failure_policy,
        outcome_policy: &schedule.outcome_policy,
        predecessor_semantics: schedule.predecessor_semantics,
        predecessor_dataflow: schedule.predecessor_dataflow,
        partial_output_dataflow: schedule.partial_output_dataflow,
        receipt_handling: schedule.receipt_handling,
        nodes: &schedule.nodes,
        initial_frontier: &schedule.initial_frontier,
        initial_node: &schedule.initial_node,
        execution_contract_present: schedule.execution_contract_present,
        dispatch_authority_released: schedule.dispatch_authority_released,
        progress_observed: schedule.progress_observed,
        successor_advanced: schedule.successor_advanced,
    }
}

fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn invalid(message: &str) -> GroupAgentGraphExecutionScheduleValidationError {
    GroupAgentGraphExecutionScheduleValidationError {
        message: message.into(),
    }
}
