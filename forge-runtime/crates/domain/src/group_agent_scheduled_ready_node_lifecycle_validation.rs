use serde::Serialize;

use super::{
    ClaimGroupAgentScheduledReadyNodeDispatch, GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
    GroupAgentScheduledReadyNodeLifecycleInspection,
};
use crate::{
    GroupAgentScheduledNodeDispatchClaim, GroupAgentScheduledNodeLifecycleStatus,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalControl,
    MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES, group_agent_node_provider_request_sha256,
};

type Error = crate::GroupAgentScheduledNodeLifecycleValidationError;

pub(super) fn validate_claim_request(
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> Result<(), Error> {
    if request.v != GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION {
        return Err(invalid("invalid scheduled ready-node lifecycle version"));
    }
    validate_claim_sources(request)?;
    validate_claim_bindings(request)
}

fn validate_claim_sources(
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> Result<(), Error> {
    request
        .release_control
        .validate()
        .map_err(|error| invalid(&error.message))?;
    request
        .authorization
        .validate_against_release_control(&request.release_control)
        .map_err(|error| invalid(&error.message))?;
    request
        .pricing
        .verify_scheduled_ready_authorization(&request.authorization)
        .map_err(|_| invalid("scheduled ready-node pricing binding is invalid"))?;
    request
        .provider_request
        .validate()
        .map_err(|error| invalid(&error.message))?;
    exact_json(&request.release_control_json, &request.release_control)?;
    exact_json(&request.authorization_json, &request.authorization)?;
    exact_json(&request.pricing_json, &request.pricing)?;
    validate_provider_body(request)
}

fn validate_provider_body(
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> Result<(), Error> {
    let body = &request.provider_request_body;
    let valid = !body.is_empty()
        && body.len() <= MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES
        && group_agent_node_provider_request_sha256(body)
            == request.provider_request.provider_request_sha256
        && request.release_control.provider_request_json.as_bytes() == body;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled ready-node provider request bytes disagree"))
}

fn validate_claim_bindings(
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> Result<(), Error> {
    request.claim.validate()?;
    request.active_lane.validate_against_claim(&request.claim)?;
    request.claim_event.validate()?;
    exact_json(&request.claim_json, &request.claim)?;
    exact_json(&request.active_lane_json, &request.active_lane)?;
    exact_json(&request.claim_event_json, &request.claim_event)?;
    let auth = &request.authorization;
    let claim = &request.claim;
    let valid = common_claim_bindings(claim, request)
        && claim.authorization_id == auth.authorization_id
        && claim.authorization_sha256 == auth.authorization_sha256
        && claim.pricing_snapshot_sha256 == auth.pricing_snapshot_sha256
        && claim.max_cost_usd_micros == auth.budgets.max_cost_usd_micros
        && claim.node_id == auth.node_id
        && claim.attempt == auth.attempt
        && claim.project_lane_sha256 == auth.project_lane_sha256
        && claim.expected_last_event_seq == auth.expected_last_event_seq
        && claim.expected_last_event_sha256 == auth.expected_last_event_sha256
        && request.claim_event.released_at_ms == claim.released_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled ready-node claim source bindings disagree"))
}

fn common_claim_bindings(
    claim: &GroupAgentScheduledNodeDispatchClaim,
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
) -> bool {
    let record = &request.provider_request;
    claim.graph_run_id == request.release_control.graph_run.graph_run_id
        && claim.provider_request_id == record.provider_request_id
        && claim.provider_request_sha256 == record.prepared_request_sha256
        && claim.provider_request_sha256 == request.authorization.scheduled_provider_request_sha256
        && claim.request_body_sha256 == record.provider_request_sha256
        && claim.request_body_sha256 == request.authorization.request_body_sha256
        && claim.request_body_bytes == request.authorization.request_body_bytes
        && claim.node_id == record.node_id
        && claim.project_lane_sha256 == record.project_lane_sha256
        && claim.expected_last_event_seq == record.expected_last_event_seq
        && claim.expected_last_event_sha256 == record.expected_last_event_sha256
}

#[allow(clippy::too_many_lines)]
pub(super) fn validate_inspection(
    value: &GroupAgentScheduledReadyNodeLifecycleInspection,
) -> Result<(), Error> {
    if value.v != GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION {
        return Err(invalid(
            "invalid scheduled ready-node lifecycle inspection version",
        ));
    }
    validate_inspection_sources(value)?;
    validate_terminal_evidence(value)?;
    validate_projections(value)?;
    validate_status(value)
}

fn validate_inspection_sources(
    value: &GroupAgentScheduledReadyNodeLifecycleInspection,
) -> Result<(), Error> {
    value
        .release_control
        .validate()
        .map_err(|error| invalid(&error.message))?;
    value
        .authorization
        .validate_against_release_control(&value.release_control)
        .map_err(|error| invalid(&error.message))?;
    value
        .pricing
        .verify_scheduled_ready_authorization(&value.authorization)
        .map_err(|_| invalid("scheduled ready-node lifecycle pricing disagrees"))?;
    value
        .provider_request
        .validate()
        .map_err(|e| invalid(&e.message))?;
    value.claim.validate()?;
    let valid = inspection_run_matches_release(value)
        && inspection_claim_matches_sources(value)
        && group_agent_node_provider_request_sha256(&value.provider_request_body)
            == value.claim.request_body_sha256
        && value.provider_request_body.len() == value.claim.request_body_bytes
        && value.release_control.provider_request == value.provider_request
        && value.release_control.provider_request_json.as_bytes() == value.provider_request_body;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled ready-node lifecycle request binding disagrees"))?;
    exact_json(&value.claim_json, &value.claim)
}

fn inspection_run_matches_release(value: &GroupAgentScheduledReadyNodeLifecycleInspection) -> bool {
    value.graph_run.run == value.release_control.graph_run
        && value.graph_run.events == value.release_control.journal_events
        && value.graph_run.plan == value.release_control.control_snapshot.plan
}

fn inspection_claim_matches_sources(
    value: &GroupAgentScheduledReadyNodeLifecycleInspection,
) -> bool {
    let claim = &value.claim;
    let auth = &value.authorization;
    let request = &value.provider_request;
    claim.graph_run_id == value.release_control.graph_run.graph_run_id
        && claim.provider_request_id == request.provider_request_id
        && claim.provider_request_sha256 == request.prepared_request_sha256
        && claim.provider_request_sha256 == auth.scheduled_provider_request_sha256
        && claim.request_body_sha256 == request.provider_request_sha256
        && claim.request_body_sha256 == auth.request_body_sha256
        && claim.request_body_bytes == auth.request_body_bytes
        && claim.authorization_id == auth.authorization_id
        && claim.authorization_sha256 == auth.authorization_sha256
        && claim.pricing_snapshot_sha256 == auth.pricing_snapshot_sha256
        && claim.max_cost_usd_micros == auth.budgets.max_cost_usd_micros
        && claim.node_id == request.node_id
        && claim.node_id == auth.node_id
        && claim.attempt == auth.attempt
        && claim.project_lane_sha256 == request.project_lane_sha256
        && claim.project_lane_sha256 == auth.project_lane_sha256
        && claim.expected_last_event_seq == request.expected_last_event_seq
        && claim.expected_last_event_seq == auth.expected_last_event_seq
        && claim.expected_last_event_sha256 == request.expected_last_event_sha256
        && claim.expected_last_event_sha256 == auth.expected_last_event_sha256
}

fn validate_terminal_evidence(
    value: &GroupAgentScheduledReadyNodeLifecycleInspection,
) -> Result<(), Error> {
    if let Some(lane) = &value.active_lane {
        lane.validate_against_claim(&value.claim)?;
    }
    if let Some(artifact) = &value.artifact {
        artifact.validate()?;
        if !artifact_matches_claim(artifact, &value.claim) {
            return Err(invalid(
                "scheduled ready-node artifact disagrees with claim",
            ));
        }
    }
    if let Some(control) = &value.terminal_control {
        control.validate()?;
        if !control_matches_sources(control, value)
            || value.artifact.as_ref() != Some(&control.artifact)
        {
            return Err(invalid("scheduled ready-node terminal control disagrees"));
        }
    }
    if let Some(receipt) = &value.terminal_receipt {
        let control = value
            .terminal_control
            .as_ref()
            .ok_or_else(|| invalid("scheduled ready-node receipt has no control"))?;
        receipt.validate_against_control(control)?;
    }
    Ok(())
}

fn validate_projections(
    value: &GroupAgentScheduledReadyNodeLifecycleInspection,
) -> Result<(), Error> {
    exact_optional(value.active_lane_json.as_ref(), value.active_lane.as_ref())?;
    exact_optional(value.artifact_json.as_ref(), value.artifact.as_ref())?;
    exact_optional(
        value.terminal_control_json.as_ref(),
        value.terminal_control.as_ref(),
    )?;
    exact_optional(
        value.terminal_receipt_json.as_ref(),
        value.terminal_receipt.as_ref(),
    )
}

fn validate_status(value: &GroupAgentScheduledReadyNodeLifecycleInspection) -> Result<(), Error> {
    let valid = match value.status {
        GroupAgentScheduledNodeLifecycleStatus::Claimed => {
            value.active_lane.is_some()
                && value.artifact.is_none()
                && value.terminal_control.is_none()
                && value.terminal_receipt.is_none()
        }
        GroupAgentScheduledNodeLifecycleStatus::Terminalized => {
            value.active_lane.is_none()
                && value.artifact.is_some()
                && value.terminal_control.is_some()
                && value.terminal_receipt.is_some()
        }
        GroupAgentScheduledNodeLifecycleStatus::Quarantined => {
            value.active_lane.is_none()
                && value.artifact.is_some()
                && value.terminal_control.is_none()
                && value.terminal_receipt.is_none()
        }
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated => {
            value.active_lane.is_none()
                && value.artifact.is_none()
                && value.terminal_control.is_none()
                && value.terminal_receipt.is_none()
        }
    };
    if !valid {
        return Err(invalid(
            "scheduled ready-node lifecycle evidence/status disagree",
        ));
    }
    validate_adjudication_time(value)
}

fn validate_adjudication_time(
    value: &GroupAgentScheduledReadyNodeLifecycleInspection,
) -> Result<(), Error> {
    let valid = match value.status {
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated => value
            .adjudicated_at_ms
            .is_some_and(|time| time >= value.claim.released_at_ms),
        GroupAgentScheduledNodeLifecycleStatus::Claimed
        | GroupAgentScheduledNodeLifecycleStatus::Terminalized
        | GroupAgentScheduledNodeLifecycleStatus::Quarantined => value.adjudicated_at_ms.is_none(),
    };
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled ready-node lifecycle adjudication time disagrees"))
}

fn control_matches_sources(
    control: &GroupAgentScheduledNodeTerminalControl,
    value: &GroupAgentScheduledReadyNodeLifecycleInspection,
) -> bool {
    let claim = &value.claim;
    control.release_control_snapshot_sha256 == value.release_control.snapshot_sha256
        && control.graph_run_id == claim.graph_run_id
        && control.graph_id == value.graph_run.run.graph_id
        && control.node_id == claim.node_id
        && control.attempt == claim.attempt
        && control.dispatch_id == claim.dispatch_id
        && control.provider_request_id == claim.provider_request_id
        && control.authorization_sha256 == claim.authorization_sha256
        && control.provider_request_sha256 == claim.provider_request_sha256
        && control.request_body_sha256 == claim.request_body_sha256
        && control.expected_last_event_seq == claim.expected_last_event_seq
        && control.expected_last_event_sha256 == claim.expected_last_event_sha256
        && control.claim_event_sha256 == claim.claim_event_sha256
        && control.project_lane_sha256 == claim.project_lane_sha256
}

fn artifact_matches_claim(
    artifact: &GroupAgentScheduledNodeTerminalArtifact,
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> bool {
    artifact.graph_run_id == claim.graph_run_id
        && artifact.node_id == claim.node_id
        && artifact.attempt == claim.attempt
        && artifact.dispatch_id == claim.dispatch_id
        && artifact.provider_request_id == claim.provider_request_id
        && artifact.claim_event_sha256 == claim.claim_event_sha256
        && artifact.authorization_sha256 == claim.authorization_sha256
        && artifact.provider_request_sha256 == claim.provider_request_sha256
        && artifact.request_body_sha256 == claim.request_body_sha256
        && artifact.pricing_snapshot_sha256 == claim.pricing_snapshot_sha256
        && artifact.lane_ownership_id == claim.lane_ownership_id
        && artifact.project_lane_sha256 == claim.project_lane_sha256
        && artifact.created_at_ms >= claim.released_at_ms
}

fn exact_optional<T: Serialize>(json: Option<&String>, value: Option<&T>) -> Result<(), Error> {
    match (json, value) {
        (Some(json), Some(value)) => exact_json(json, value),
        (None, None) => Ok(()),
        _ => Err(invalid(
            "scheduled ready-node lifecycle JSON presence disagrees",
        )),
    }
}

fn exact_json<T: Serialize>(json: &str, value: &T) -> Result<(), Error> {
    let canonical = serde_json::to_string(value)
        .map_err(|_| invalid("scheduled ready-node lifecycle JSON cannot be encoded"))?;
    (canonical == json)
        .then_some(())
        .ok_or_else(|| invalid("scheduled ready-node lifecycle JSON is not canonical"))
}

fn invalid(message: &str) -> Error {
    Error {
        message: message.into(),
    }
}
