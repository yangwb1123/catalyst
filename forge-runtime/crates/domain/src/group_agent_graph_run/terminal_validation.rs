use super::{
    GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION, GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunRecord,
    GroupAgentGraphRunStatus,
};
use crate::{
    GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION, GroupAgentNodeTerminalOutcome,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_NODE_COST_USD_MICROS,
    MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES, group_agent_node_dispatch_authorization_id,
    group_agent_node_dispatch_request_id, group_agent_node_terminal_artifact_id,
    group_agent_node_terminal_receipt_id,
};

use super::journal_validation::{is_lower_hex_digest, valid_identifier, valid_text};

pub(super) fn valid_terminal_record_state(record: &GroupAgentGraphRunRecord) -> bool {
    let common = record.execution_contract_present
        && record.dispatch_request_present
        && record.dispatch_authority_released;
    common
        && matches!(
            (record.v, record.status, record.last_event_seq),
            (
                GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION,
                GroupAgentGraphRunStatus::DispatchUnknown,
                4,
            ) | (
                GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION,
                GroupAgentGraphRunStatus::Completed
                    | GroupAgentGraphRunStatus::Failed
                    | GroupAgentGraphRunStatus::FailedUncertain,
                5,
            )
        )
}

pub(super) fn terminal_record_matches_head(
    record: &GroupAgentGraphRunRecord,
    events: &[GroupAgentGraphRunEvent],
) -> bool {
    if record.v != GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION {
        return true;
    }
    let Some(GroupAgentGraphRunEvent {
        kind: GroupAgentGraphRunEventKind::NodeLifecycleTerminalized { graph_status, .. },
        ..
    }) = events.get(4)
    else {
        return false;
    };
    record.status == *graph_status
}

pub(super) fn validate_lifecycle_event(
    event: &GroupAgentGraphRunEvent,
    kind: &GroupAgentGraphRunEventKind,
) -> bool {
    match kind {
        GroupAgentGraphRunEventKind::NodeDispatchReleased { .. } => {
            validate_dispatch_released_event(event, kind)
        }
        GroupAgentGraphRunEventKind::NodeLifecycleTerminalized { .. } => {
            validate_terminalized_event(event, kind)
        }
        _ => false,
    }
}

pub(super) fn validate_dispatch_released_event(
    event: &GroupAgentGraphRunEvent,
    kind: &GroupAgentGraphRunEventKind,
) -> bool {
    let GroupAgentGraphRunEventKind::NodeDispatchReleased {
        previous_event_sha256,
        dispatch_id,
        authorization_id,
        authorization_sha256,
        dispatch_request_id,
        dispatch_request_sha256,
        logical_request_sha256,
        request_body_sha256,
        request_body_bytes,
        pricing_snapshot_sha256,
        node_id,
        attempt,
        max_cost_usd_micros,
        consent_contract_version,
        lane_ownership_id,
        project_lane_sha256,
        released_at_ms,
    } = kind
    else {
        return false;
    };
    event.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION
        && event.seq == 4
        && valid_identifier(&event.graph_run_id)
        && is_lower_hex_digest(previous_event_sha256)
        && valid_text(dispatch_id, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
        && authorization_id == &group_agent_node_dispatch_authorization_id(authorization_sha256)
        && is_lower_hex_digest(authorization_sha256)
        && dispatch_request_id == &group_agent_node_dispatch_request_id(dispatch_request_sha256)
        && is_lower_hex_digest(dispatch_request_sha256)
        && is_lower_hex_digest(logical_request_sha256)
        && is_lower_hex_digest(request_body_sha256)
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES).contains(request_body_bytes)
        && is_lower_hex_digest(pricing_snapshot_sha256)
        && valid_identifier(node_id)
        && *attempt == 1
        && (1..=MAX_GROUP_AGENT_NODE_COST_USD_MICROS).contains(max_cost_usd_micros)
        && *consent_contract_version == GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION
        && valid_text(lane_ownership_id, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
        && is_lower_hex_digest(project_lane_sha256)
        && i64::try_from(*released_at_ms).is_ok()
}

pub(super) fn validate_terminalized_event(
    event: &GroupAgentGraphRunEvent,
    kind: &GroupAgentGraphRunEventKind,
) -> bool {
    let GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
        previous_event_sha256,
        dispatch_id,
        lane_ownership_id,
        project_lane_sha256,
        artifact_id,
        artifact_sha256,
        terminal_receipt_id,
        terminal_receipt_sha256,
        node_id,
        attempt,
        node_outcome,
        wave_index,
        wave_outcome,
        graph_status,
        retry_authorized,
        lane_released,
        terminalized_at_ms,
    } = kind
    else {
        return false;
    };
    event.v == GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION
        && event.seq == 5
        && valid_identifier(&event.graph_run_id)
        && is_lower_hex_digest(previous_event_sha256)
        && valid_text(dispatch_id, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
        && valid_text(lane_ownership_id, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
        && is_lower_hex_digest(project_lane_sha256)
        && artifact_id == &group_agent_node_terminal_artifact_id(artifact_sha256)
        && is_lower_hex_digest(artifact_sha256)
        && terminal_receipt_id == &group_agent_node_terminal_receipt_id(terminal_receipt_sha256)
        && is_lower_hex_digest(terminal_receipt_sha256)
        && valid_identifier(node_id)
        && *attempt == 1
        && *wave_index == 0
        && terminal_outcomes_agree(*node_outcome, *wave_outcome, *graph_status)
        && !retry_authorized
        && *lane_released
        && i64::try_from(*terminalized_at_ms).is_ok()
}

pub(super) fn previous_event_sha256(event: &GroupAgentGraphRunEvent) -> Option<&str> {
    match &event.kind {
        GroupAgentGraphRunEventKind::GraphRunPrepared { .. } => None,
        GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
            previous_event_sha256,
            ..
        }
        | GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256,
            ..
        }
        | GroupAgentGraphRunEventKind::NodeDispatchReleased {
            previous_event_sha256,
            ..
        }
        | GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
            previous_event_sha256,
            ..
        } => Some(previous_event_sha256),
    }
}

fn terminal_outcomes_agree(
    node: GroupAgentNodeTerminalOutcome,
    wave: GroupAgentNodeTerminalOutcome,
    graph: GroupAgentGraphRunStatus,
) -> bool {
    node == wave
        && matches!(
            (node, graph),
            (
                GroupAgentNodeTerminalOutcome::Completed,
                GroupAgentGraphRunStatus::Completed,
            ) | (
                GroupAgentNodeTerminalOutcome::Failed,
                GroupAgentGraphRunStatus::Failed,
            ) | (
                GroupAgentNodeTerminalOutcome::FailedUncertain,
                GroupAgentGraphRunStatus::FailedUncertain,
            )
        )
}
