use std::collections::BTreeSet;

use super::{
    MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES, MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES,
    SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION, SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION,
    SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION, ScheduledGraphProgressNode,
    ScheduledGraphProgressSnapshot, ScheduledGraphProgressValidationError,
    ScheduledGraphReconcileDecision, ScheduledGraphReconcileDisposition,
};
use crate::{
    GroupAgentGraphExecutionAttemptPolicy, GroupAgentGraphExecutionFailurePolicy,
    GroupAgentGraphExecutionMode, GroupAgentGraphExecutionProgressionPolicy,
    GroupAgentScheduledNodeLifecycleStatus, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES,
    MAX_GROUP_AGENT_GRAPH_NODES, group_agent_scheduled_node_provider_request_id,
};

const SCHEDULE_ID_PREFIX: &str = "graph-execution-schedule-";
const CANDIDATE_ID_PREFIX: &str = "scheduled-node-contract-";

pub(super) fn validate_snapshot(
    snapshot: &ScheduledGraphProgressSnapshot,
) -> Result<(), ScheduledGraphProgressValidationError> {
    validate_snapshot_header(snapshot)?;
    validate_snapshot_nodes(snapshot)?;
    if snapshot.snapshot_sha256 != snapshot.expected_sha256()? {
        return Err(invalid("progress snapshot digest disagrees"));
    }
    validate_size(
        snapshot.canonical_json()?.len(),
        MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES,
        "progress snapshot",
    )
}

fn validate_snapshot_header(
    snapshot: &ScheduledGraphProgressSnapshot,
) -> Result<(), ScheduledGraphProgressValidationError> {
    let fixed = snapshot.v == SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION
        && snapshot.progress_protocol_version == SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION
        && valid_identifier(&snapshot.graph_run_id)
        && valid_identifier(&snapshot.graph_id)
        && snapshot.schedule_id == format!("{SCHEDULE_ID_PREFIX}{}", snapshot.schedule_sha256)
        && digest(&snapshot.schedule_sha256)
        && (2..=MAX_GROUP_AGENT_GRAPH_NODES).contains(&snapshot.node_count)
        && snapshot.execution_mode == GroupAgentGraphExecutionMode::Serial
        && snapshot.max_in_flight_nodes == 1
        && snapshot.progression_policy
            == GroupAgentGraphExecutionProgressionPolicy::CompletedContiguousPrefix
        && snapshot.attempt_policy == GroupAgentGraphExecutionAttemptPolicy::ExactlyOne
        && snapshot.failure_policy == GroupAgentGraphExecutionFailurePolicy::FailFastNoRetry
        && digest(&snapshot.snapshot_sha256);
    fixed
        .then_some(())
        .ok_or_else(|| invalid("progress snapshot header or fixed policy is invalid"))
}

fn validate_snapshot_nodes(
    snapshot: &ScheduledGraphProgressSnapshot,
) -> Result<(), ScheduledGraphProgressValidationError> {
    if snapshot.nodes.len() != snapshot.node_count {
        return Err(invalid("progress snapshot node count disagrees"));
    }
    let mut node_ids = BTreeSet::new();
    let mut candidate_ids = BTreeSet::new();
    let mut provider_ids = BTreeSet::new();
    let mut terminal_receipts = BTreeSet::new();
    for (ordinal, node) in snapshot.nodes.iter().enumerate() {
        validate_node(node, ordinal)?;
        if !node_ids.insert(node.node_id.as_str())
            || !insert_optional(&mut candidate_ids, node.candidate_id.as_deref())
            || !insert_optional(&mut provider_ids, node.provider_request_id.as_deref())
            || !insert_optional(
                &mut terminal_receipts,
                node.terminal_receipt_sha256.as_deref(),
            )
        {
            return Err(invalid("progress snapshot contains duplicate identities"));
        }
    }
    Ok(())
}

fn validate_node(
    node: &ScheduledGraphProgressNode,
    ordinal: usize,
) -> Result<(), ScheduledGraphProgressValidationError> {
    if node.execution_ordinal != ordinal || !valid_identifier(&node.node_id) || node.attempt != 1 {
        return Err(invalid("progress snapshot node identity is invalid"));
    }
    validate_candidate(node)?;
    validate_provider(node)?;
    validate_lifecycle(node)
}

fn validate_candidate(
    node: &ScheduledGraphProgressNode,
) -> Result<(), ScheduledGraphProgressValidationError> {
    match (&node.candidate_id, &node.candidate_sha256) {
        (None, None) => Ok(()),
        (Some(identifier), Some(sha256))
            if digest(sha256) && identifier == &format!("{CANDIDATE_ID_PREFIX}{sha256}") =>
        {
            Ok(())
        }
        _ => Err(invalid("progress node candidate identity disagrees")),
    }
}

fn validate_provider(
    node: &ScheduledGraphProgressNode,
) -> Result<(), ScheduledGraphProgressValidationError> {
    match (&node.provider_request_id, &node.prepared_request_sha256) {
        (None, None) => Ok(()),
        (Some(identifier), Some(sha256))
            if node.candidate_id.is_some()
                && digest(sha256)
                && identifier == &group_agent_scheduled_node_provider_request_id(sha256) =>
        {
            Ok(())
        }
        _ => Err(invalid("progress node provider request identity disagrees")),
    }
}

fn validate_lifecycle(
    node: &ScheduledGraphProgressNode,
) -> Result<(), ScheduledGraphProgressValidationError> {
    let paired_terminal = node.terminal_outcome.is_some()
        && node.terminal_receipt_sha256.as_deref().is_some_and(digest);
    let terminal_absent = node.terminal_outcome.is_none() && node.terminal_receipt_sha256.is_none();
    let valid = match node.lifecycle_status {
        Some(GroupAgentScheduledNodeLifecycleStatus::Terminalized) => paired_terminal,
        None
        | Some(
            GroupAgentScheduledNodeLifecycleStatus::Claimed
            | GroupAgentScheduledNodeLifecycleStatus::Quarantined
            | GroupAgentScheduledNodeLifecycleStatus::Adjudicated,
        ) => terminal_absent,
    };
    (node.lifecycle_status.is_none() || node.provider_request_id.is_some())
        .then_some(())
        .ok_or_else(|| invalid("progress node lifecycle has no provider request"))?;
    valid
        .then_some(())
        .ok_or_else(|| invalid("progress node lifecycle evidence shape disagrees"))
}

pub(super) fn validate_decision(
    decision: &ScheduledGraphReconcileDecision,
) -> Result<(), ScheduledGraphProgressValidationError> {
    let header = decision.v == SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION
        && decision.progress_protocol_version == SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION
        && valid_identifier(&decision.graph_run_id)
        && decision.schedule_id == format!("{SCHEDULE_ID_PREFIX}{}", decision.schedule_sha256)
        && digest(&decision.schedule_sha256)
        && digest(&decision.snapshot_sha256)
        && digest(&decision.decision_sha256);
    if !header || !decision_fields_match_disposition(decision) {
        return Err(invalid("reconcile decision shape or identity is invalid"));
    }
    if decision.decision_sha256 != decision.expected_sha256()? {
        return Err(invalid("reconcile decision digest disagrees"));
    }
    validate_size(
        decision.canonical_json()?.len(),
        MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES,
        "reconcile decision",
    )
}

fn decision_fields_match_disposition(decision: &ScheduledGraphReconcileDecision) -> bool {
    match decision.disposition {
        ScheduledGraphReconcileDisposition::Ready => {
            decision.next_execution_ordinal.is_some()
                && decision
                    .next_node_id
                    .as_deref()
                    .is_some_and(valid_identifier)
        }
        _ => decision.next_execution_ordinal.is_none() && decision.next_node_id.is_none(),
    }
}

pub(super) fn validate_decision_against_snapshot(
    decision: &ScheduledGraphReconcileDecision,
    snapshot: &ScheduledGraphProgressSnapshot,
) -> Result<(), ScheduledGraphProgressValidationError> {
    snapshot.validate()?;
    decision.validate()?;
    let bound = decision.graph_run_id == snapshot.graph_run_id
        && decision.schedule_id == snapshot.schedule_id
        && decision.schedule_sha256 == snapshot.schedule_sha256
        && decision.snapshot_sha256 == snapshot.snapshot_sha256;
    if !bound {
        return Err(invalid("reconcile decision source binding disagrees"));
    }
    validate_next_node_binding(decision, snapshot)
}

fn validate_next_node_binding(
    decision: &ScheduledGraphReconcileDecision,
    snapshot: &ScheduledGraphProgressSnapshot,
) -> Result<(), ScheduledGraphProgressValidationError> {
    let Some(ordinal) = decision.next_execution_ordinal else {
        return Ok(());
    };
    let node = snapshot.nodes.get(ordinal);
    let bound = node.is_some_and(|node| {
        decision.next_node_id.as_deref() == Some(node.node_id.as_str())
            && node.execution_ordinal == ordinal
    });
    bound
        .then_some(())
        .ok_or_else(|| invalid("reconcile decision next node binding disagrees"))
}

fn insert_optional<'a>(set: &mut BTreeSet<&'a str>, value: Option<&'a str>) -> bool {
    value.is_none_or(|value| set.insert(value))
}

fn validate_size(
    bytes: usize,
    maximum: usize,
    subject: &str,
) -> Result<(), ScheduledGraphProgressValidationError> {
    (bytes > 0 && bytes <= maximum)
        .then_some(())
        .ok_or_else(|| invalid(&format!("{subject} exceeds its byte bound")))
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

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn invalid(message: &str) -> ScheduledGraphProgressValidationError {
    ScheduledGraphProgressValidationError {
        message: message.into(),
    }
}
