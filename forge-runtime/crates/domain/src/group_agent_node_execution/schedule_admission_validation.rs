use std::collections::BTreeMap;

use super::{
    AdmitGroupAgentGraphExecutionSchedule, GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION,
    GroupAgentGraphExecutionSchedule, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphExecutionScheduleNode, GroupAgentGraphExecutionScheduleRecord,
    GroupAgentGraphExecutionScheduleValidationError,
    MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES,
    validation::{invalid, is_digest},
};
use crate::{
    GroupAgentGraphControlSnapshot, MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_GRAPH_NODES,
    group_agent_project_lane_sha256,
};

const SCHEDULE_ID_PREFIX: &str = "graph-execution-schedule-";

pub(super) fn validate_against_control(
    schedule: &GroupAgentGraphExecutionSchedule,
    snapshot: &GroupAgentGraphControlSnapshot,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    schedule.validate()?;
    snapshot
        .validate()
        .map_err(|_| invalid("execution schedule control snapshot is invalid"))?;
    if snapshot.plan.authored_node_ids.len() < 2 {
        return Err(invalid(
            "execution schedules require a multi-node control snapshot",
        ));
    }
    validate_control_bindings(schedule, snapshot)?;
    let expected_nodes = expected_nodes(snapshot)?;
    if schedule.nodes != expected_nodes
        || schedule.initial_frontier != snapshot.plan.waves[0]
        || schedule.initial_node != snapshot.plan.waves[0][0]
    {
        return Err(invalid(
            "execution schedule topology or initial selection disagrees with control",
        ));
    }
    Ok(())
}

fn validate_control_bindings(
    schedule: &GroupAgentGraphExecutionSchedule,
    snapshot: &GroupAgentGraphControlSnapshot,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    let valid = schedule.scheduler_protocol_version == snapshot.scheduler_protocol_version
        && schedule.control_snapshot_sha256 == snapshot.snapshot_sha256
        && schedule.expected_last_event_seq == snapshot.last_event_seq
        && schedule.expected_last_event_sha256 == snapshot.last_event_sha256
        && schedule.graph_run_id == snapshot.graph_run_id
        && schedule.graph_id == snapshot.graph_id
        && schedule.source_snapshot_sha256 == snapshot.source_snapshot_sha256
        && schedule.graph_manifest_sha256 == snapshot.graph_manifest_sha256
        && schedule.core_plan_sha256 == snapshot.core_plan_sha256
        && schedule.node_count == snapshot.plan.authored_node_ids.len()
        && schedule.wave_count == snapshot.plan.waves.len()
        && schedule.execution_contract_present == snapshot.execution_contract_present
        && schedule.dispatch_authority_released == snapshot.dispatch_authority_released;
    valid
        .then_some(())
        .ok_or_else(|| invalid("execution schedule identity, head, or control binding disagrees"))
}

fn expected_nodes(
    snapshot: &GroupAgentGraphControlSnapshot,
) -> Result<
    Vec<GroupAgentGraphExecutionScheduleNode>,
    GroupAgentGraphExecutionScheduleValidationError,
> {
    let authored_positions = snapshot
        .plan
        .authored_node_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let manifest_nodes = snapshot
        .manifest
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let mut result = Vec::with_capacity(snapshot.plan.authored_node_ids.len());
    for (wave_index, wave) in snapshot.plan.waves.iter().enumerate() {
        for node_id in wave {
            let authored_node_index = authored_positions
                .get(node_id.as_str())
                .copied()
                .ok_or_else(|| invalid("control plan has an unknown scheduled node"))?;
            let manifest_node = manifest_nodes
                .get(node_id.as_str())
                .copied()
                .ok_or_else(|| invalid("control manifest has no scheduled node"))?;
            result.push(GroupAgentGraphExecutionScheduleNode {
                execution_ordinal: result.len(),
                node_id: node_id.clone(),
                authored_node_index,
                topology_wave_index: wave_index,
                project_lane_sha256: group_agent_project_lane_sha256(&manifest_node.project_id),
                attempt: 1,
                direct_predecessor_node_ids: direct_predecessors(snapshot, node_id),
            });
        }
    }
    Ok(result)
}

fn direct_predecessors(snapshot: &GroupAgentGraphControlSnapshot, node_id: &str) -> Vec<String> {
    snapshot
        .plan
        .authored_node_ids
        .iter()
        .filter(|candidate| {
            snapshot
                .plan
                .edges
                .iter()
                .any(|edge| edge.from_node_id == candidate.as_str() && edge.to_node_id == node_id)
        })
        .cloned()
        .collect()
}

pub(super) fn validate_record(
    record: &GroupAgentGraphExecutionScheduleRecord,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    let valid = record.v == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION
        && valid_identifier(&record.schedule_id)
        && record.schedule_id == format!("{SCHEDULE_ID_PREFIX}{}", record.schedule_sha256)
        && valid_identifier(&record.graph_run_id)
        && valid_identifier(&record.graph_id)
        && is_digest(&record.control_snapshot_sha256)
        && is_digest(&record.schedule_sha256)
        && (1..=MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES).contains(&record.schedule_bytes)
        && (2..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&record.node_count)
        && (1..=record.node_count).contains(&record.wave_count)
        && record.expected_last_event_seq == 1
        && is_digest(&record.expected_last_event_sha256)
        && !record.execution_contract_present
        && !record.dispatch_authority_released
        && i64::try_from(record.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid execution schedule record"))
}

pub(super) fn validate_admission(
    request: &AdmitGroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    request
        .control_snapshot
        .validate()
        .map_err(|_| invalid("schedule admission control snapshot is invalid"))?;
    request.schedule.validate()?;
    request
        .schedule
        .validate_against_control(&request.control_snapshot)?;
    let valid = request.v == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION
        && request.graph_run_id == request.control_snapshot.graph_run_id
        && request.graph_run_id == request.schedule.graph_run_id
        && request.control_snapshot.canonical_json().as_deref()
            == Ok(request.control_snapshot_json.as_str())
        && request.schedule.canonical_json().as_deref() == Ok(request.schedule_json.as_str())
        && request.schedule_json.len() <= MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES
        && valid_text(
            &request.idempotency_key,
            MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES,
        )
        && i64::try_from(request.admitted_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid exact execution schedule admission"))
}

pub(super) fn validate_inspection(
    inspection: &GroupAgentGraphExecutionScheduleInspection,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    inspection.record.validate()?;
    let decoded = GroupAgentGraphExecutionSchedule::decode_exact(&inspection.schedule_json)?;
    let schedule = &inspection.schedule;
    schedule.validate()?;
    let record = &inspection.record;
    let valid = inspection.v == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION
        && decoded == *schedule
        && record.schedule_id == schedule.schedule_id
        && record.graph_run_id == schedule.graph_run_id
        && record.graph_id == schedule.graph_id
        && record.control_snapshot_sha256 == schedule.control_snapshot_sha256
        && record.schedule_sha256 == schedule.schedule_sha256
        && record.schedule_bytes == inspection.schedule_json.len()
        && record.node_count == schedule.node_count
        && record.wave_count == schedule.wave_count
        && record.expected_last_event_seq == schedule.expected_last_event_seq
        && record.expected_last_event_sha256 == schedule.expected_last_event_sha256
        && record.execution_contract_present == schedule.execution_contract_present
        && record.dispatch_authority_released == schedule.dispatch_authority_released;
    valid
        .then_some(())
        .ok_or_else(|| invalid("execution schedule inspection bindings disagree"))
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(unsupported_character)
}

fn unsupported_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}
