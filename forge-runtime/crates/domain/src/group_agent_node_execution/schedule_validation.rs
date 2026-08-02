use std::collections::{BTreeMap, BTreeSet};

use super::{
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION,
    GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION, GroupAgentGraphCompletedOutcomePolicy,
    GroupAgentGraphDispatchUnknownOutcomePolicy, GroupAgentGraphExecutionAttemptPolicy,
    GroupAgentGraphExecutionFailurePolicy, GroupAgentGraphExecutionMode,
    GroupAgentGraphExecutionProgressionPolicy, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleNode, GroupAgentGraphExecutionScheduleValidationError,
    GroupAgentGraphExecutionSelectionPolicy, GroupAgentGraphLengthOutcomePolicy,
    GroupAgentGraphPredecessorDataflow, GroupAgentGraphPredecessorSemantics,
    GroupAgentGraphReceiptHandling, GroupAgentGraphUncertaintyOutcomePolicy,
    MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES,
};
use crate::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODES,
};

const SCHEDULE_ID_PREFIX: &str = "graph-execution-schedule-";

pub(super) fn validate_schedule(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    validate_header(schedule)?;
    validate_policy(schedule)?;
    validate_nodes(schedule)?;
    validate_initial_selection(schedule)?;
    let expected = schedule.expected_sha256()?;
    if schedule.schedule_sha256 != expected
        || schedule.schedule_id != format!("{SCHEDULE_ID_PREFIX}{expected}")
    {
        return Err(invalid("execution schedule digest or identity disagrees"));
    }
    let bytes = schedule.canonical_json()?.len();
    if !(1..=MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES).contains(&bytes) {
        return Err(invalid("execution schedule exceeds its byte bound"));
    }
    Ok(())
}

fn validate_header(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    let valid = schedule.v == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION
        && schedule.scheduler_protocol_version == GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION
        && schedule.execution_schedule_protocol_version
            == GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION
        && is_digest(&schedule.control_snapshot_sha256)
        && schedule.expected_last_event_seq == 1
        && is_digest(&schedule.expected_last_event_sha256)
        && valid_identifier(&schedule.graph_run_id)
        && valid_identifier(&schedule.graph_id)
        && is_digest(&schedule.source_snapshot_sha256)
        && is_digest(&schedule.graph_manifest_sha256)
        && is_digest(&schedule.core_plan_sha256)
        && (2..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&schedule.node_count)
        && (1..=schedule.node_count).contains(&schedule.wave_count)
        && !schedule.execution_contract_present
        && !schedule.dispatch_authority_released
        && !schedule.progress_observed
        && !schedule.successor_advanced
        && is_digest(&schedule.schedule_sha256);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid passive execution schedule header"))
}

fn validate_policy(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    let outcome = &schedule.outcome_policy;
    let valid = schedule.execution_mode == GroupAgentGraphExecutionMode::Serial
        && schedule.max_in_flight_nodes == 1
        && schedule.selection_policy
            == GroupAgentGraphExecutionSelectionPolicy::TopologyWaveThenAuthoredOrder
        && schedule.progression_policy
            == GroupAgentGraphExecutionProgressionPolicy::CompletedContiguousPrefix
        && schedule.attempt_policy == GroupAgentGraphExecutionAttemptPolicy::ExactlyOne
        && schedule.failure_policy == GroupAgentGraphExecutionFailurePolicy::FailFastNoRetry
        && outcome.completed == GroupAgentGraphCompletedOutcomePolicy::AdvanceOrComplete
        && outcome.length == GroupAgentGraphLengthOutcomePolicy::FailGraph
        && outcome.uncertainty == GroupAgentGraphUncertaintyOutcomePolicy::FailGraphUncertain
        && outcome.dispatch_unknown
            == GroupAgentGraphDispatchUnknownOutcomePolicy::QuarantineNoAdvance
        && schedule.predecessor_semantics == GroupAgentGraphPredecessorSemantics::OrderingOnly
        && schedule.predecessor_dataflow == GroupAgentGraphPredecessorDataflow::None
        && !schedule.partial_output_dataflow
        && schedule.receipt_handling == GroupAgentGraphReceiptHandling::FutureVerifiedIdentitySlots;
    valid
        .then_some(())
        .ok_or_else(|| invalid("execution schedule policy is not the fixed passive policy"))
}

fn validate_nodes(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    if schedule.nodes.len() != schedule.node_count {
        return Err(invalid("execution schedule node count disagrees"));
    }
    let mut ids = BTreeMap::new();
    let mut authored = BTreeSet::new();
    let mut waves = BTreeSet::new();
    for (ordinal, node) in schedule.nodes.iter().enumerate() {
        validate_node_shape(schedule, node, ordinal)?;
        if ordinal > 0 {
            let previous = &schedule.nodes[ordinal - 1];
            if node.topology_wave_index < previous.topology_wave_index
                || (node.topology_wave_index == previous.topology_wave_index
                    && node.authored_node_index <= previous.authored_node_index)
            {
                return Err(invalid(
                    "execution schedule nodes are not in wave-then-authored order",
                ));
            }
        }
        if ids.insert(node.node_id.as_str(), ordinal).is_some()
            || !authored.insert(node.authored_node_index)
        {
            return Err(invalid(
                "execution schedule node identities or authored indexes are duplicated",
            ));
        }
        waves.insert(node.topology_wave_index);
    }
    if authored != (0..schedule.node_count).collect() {
        return Err(invalid(
            "execution schedule authored indexes are not complete",
        ));
    }
    if waves.len() != schedule.wave_count {
        return Err(invalid(
            "execution schedule topology waves are not complete",
        ));
    }
    validate_predecessors(schedule, &ids)
}

fn validate_node_shape(
    schedule: &GroupAgentGraphExecutionSchedule,
    node: &GroupAgentGraphExecutionScheduleNode,
    ordinal: usize,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    let valid = node.execution_ordinal == ordinal
        && valid_identifier(&node.node_id)
        && node.authored_node_index < schedule.node_count
        && node.topology_wave_index < schedule.wave_count
        && is_digest(&node.project_lane_sha256)
        && node.attempt == 1;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid execution schedule node"))
}

fn validate_predecessors(
    schedule: &GroupAgentGraphExecutionSchedule,
    ids: &BTreeMap<&str, usize>,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    for node in &schedule.nodes {
        let mut seen = BTreeSet::new();
        let mut prior_authored_index = None;
        for predecessor in &node.direct_predecessor_node_ids {
            let Some(ordinal) = ids.get(predecessor.as_str()).copied() else {
                return Err(invalid("execution schedule predecessor is unknown"));
            };
            let predecessor_node = &schedule.nodes[ordinal];
            let valid = seen.insert(predecessor.as_str())
                && predecessor != &node.node_id
                && ordinal < node.execution_ordinal
                && predecessor_node.topology_wave_index < node.topology_wave_index
                && prior_authored_index
                    .is_none_or(|prior| prior < predecessor_node.authored_node_index);
            if !valid {
                return Err(invalid(
                    "execution schedule predecessors are invalid or out of authored order",
                ));
            }
            prior_authored_index = Some(predecessor_node.authored_node_index);
        }
    }
    Ok(())
}

fn validate_initial_selection(
    schedule: &GroupAgentGraphExecutionSchedule,
) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
    let expected = schedule
        .nodes
        .iter()
        .filter(|node| node.topology_wave_index == 0)
        .map(|node| node.node_id.as_str())
        .collect::<Vec<_>>();
    let actual = schedule
        .initial_frontier
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let valid = !expected.is_empty()
        && actual == expected
        && schedule.initial_node == schedule.initial_frontier[0]
        && schedule.nodes[0].node_id == schedule.initial_node;
    valid
        .then_some(())
        .ok_or_else(|| invalid("execution schedule initial frontier or selection disagrees"))
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES
        && !value.chars().any(unsupported_character)
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

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn invalid(message: &str) -> GroupAgentGraphExecutionScheduleValidationError {
    GroupAgentGraphExecutionScheduleValidationError {
        message: message.into(),
    }
}
