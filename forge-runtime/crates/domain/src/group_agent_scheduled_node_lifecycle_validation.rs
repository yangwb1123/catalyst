use super::{
    ClaimGroupAgentScheduledNodeDispatch, GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION, GroupAgentScheduledNodeActiveLane,
    GroupAgentScheduledNodeDispatchClaim, GroupAgentScheduledNodeDispatchClaimEvent,
    GroupAgentScheduledNodeLifecycleInspection, GroupAgentScheduledNodeLifecycleStatus,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalControl,
    canonical_json, claim_event_digest,
};
use crate::{
    GroupAgentScheduledNodeProviderRequestRecord, MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES,
    group_agent_node_provider_request_sha256,
};

#[path = "group_agent_scheduled_node_lifecycle_terminal_validation.rs"]
mod terminal_validation;
pub(super) use terminal_validation::{
    decode_exact_control, decode_exact_receipt, validate_artifact, validate_control,
    validate_receipt, validate_receipt_against_control,
};

pub(super) fn validate_claim(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    let valid = claim.v == GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION
        && identifier(&claim.graph_run_id)
        && content_id(
            &claim.dispatch_id,
            "scheduled-node-dispatch-",
            &claim.authorization_sha256,
        )
        && identifier(&claim.provider_request_id)
        && identifier(&claim.authorization_id)
        && digest(&claim.authorization_sha256)
        && digest(&claim.provider_request_sha256)
        && digest(&claim.request_body_sha256)
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES).contains(&claim.request_body_bytes)
        && digest(&claim.pricing_snapshot_sha256)
        && identifier(&claim.node_id)
        && claim.attempt == 1
        && identifier(&claim.lane_ownership_id)
        && digest(&claim.project_lane_sha256)
        && claim.expected_last_event_seq == 1
        && digest(&claim.expected_last_event_sha256)
        && digest(&claim.claim_event_sha256);
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid scheduled dispatch claim"))
}

pub(super) fn validate_active_lane(
    lane: &GroupAgentScheduledNodeActiveLane,
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    let valid = lane.v == GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION
        && lane.project_lane_sha256 == claim.project_lane_sha256
        && lane.lane_ownership_id == claim.lane_ownership_id
        && lane.graph_run_id == claim.graph_run_id
        && lane.provider_request_id == claim.provider_request_id
        && lane.node_id == claim.node_id
        && lane.attempt == claim.attempt
        && lane.dispatch_id == claim.dispatch_id
        && lane.claim_event_sha256 == claim.claim_event_sha256
        && lane.claimed_at_ms == claim.released_at_ms;
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled active lane disagrees with claim"))
}

#[allow(clippy::too_many_lines)]
pub(super) fn validate_claim_request(
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    validate_request_sources(request)?;
    validate_claim_bindings(request)
}

/// Validates every frozen source artifact embedded in the claim request and
/// checks that each JSON projection is the exact canonical encoding.
fn validate_request_sources(
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
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
        .validate()
        .map_err(|error| invalid(&error.message))?;
    request
        .provider_request
        .validate()
        .map_err(|error| invalid(&error.message))?;
    exact_json(&request.release_control_json, &request.release_control)?;
    exact_json(&request.authorization_json, &request.authorization)?;
    exact_json(&request.pricing_json, &request.pricing)?;
    if request.provider_request_body.len() > MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES
        || request.provider_request_body.is_empty()
        || group_agent_node_provider_request_sha256(&request.provider_request_body)
            != request.provider_request.provider_request_sha256
        || request.release_control.provider_request_json.as_bytes() != request.provider_request_body
    {
        return Err(invalid("scheduled provider request bytes disagree"));
    }
    Ok(())
}

/// Validates the claim, lane, and event triad plus every cross-source identity
/// binding that makes the claim unambiguously owned by one exact request.
fn validate_claim_bindings(
    request: &ClaimGroupAgentScheduledNodeDispatch,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    request.claim.validate()?;
    request.active_lane.validate_against_claim(&request.claim)?;
    request.claim_event.validate()?;
    exact_json(&request.claim_json, &request.claim)?;
    exact_json(&request.active_lane_json, &request.active_lane)?;
    exact_json(&request.claim_event_json, &request.claim_event)?;
    let bindings = request.claim.graph_run_id == request.release_control.graph_run.graph_run_id
        && request.claim.provider_request_id == request.provider_request.provider_request_id
        && request.claim.provider_request_sha256
            == request.provider_request.prepared_request_sha256
        && request.claim.provider_request_sha256
            == request.authorization.scheduled_provider_request_sha256
        && request.claim.request_body_sha256 == request.provider_request.provider_request_sha256
        && request.claim.request_body_sha256 == request.authorization.request_body_sha256
        && request.claim.request_body_bytes == request.authorization.request_body_bytes
        && request.claim.authorization_id == request.authorization.authorization_id
        && request.claim.authorization_sha256 == request.authorization.authorization_sha256
        && request.claim.pricing_snapshot_sha256 == request.authorization.pricing_snapshot_sha256
        && request.claim.max_cost_usd_micros == request.authorization.budgets.max_cost_usd_micros
        && request.claim.node_id == request.provider_request.node_id
        && request.claim.node_id == request.authorization.node_id
        && request.claim.attempt == request.authorization.attempt
        && request.claim.project_lane_sha256 == request.provider_request.project_lane_sha256
        && request.claim.project_lane_sha256 == request.authorization.project_lane_sha256
        && request.claim.expected_last_event_seq
            == request.provider_request.expected_last_event_seq
        && request.claim.expected_last_event_seq == request.authorization.expected_last_event_seq
        && request.claim.expected_last_event_sha256
            == request.provider_request.expected_last_event_sha256
        && request.claim.expected_last_event_sha256
            == request.authorization.expected_last_event_sha256
        && request.claim_event.released_at_ms == request.claim.released_at_ms;
    bindings
        .then_some(())
        .ok_or_else(|| invalid("scheduled claim source bindings disagree"))
}

pub(super) fn validate_dispatch_authority(
    request: &GroupAgentScheduledNodeProviderRequestRecord,
    claim: &GroupAgentScheduledNodeDispatchClaim,
    body: &[u8],
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    claim.validate()?;
    let valid = request.provider_request_id == claim.provider_request_id
        && request.prepared_request_sha256 == claim.provider_request_sha256
        && request.provider_request_sha256 == claim.request_body_sha256
        && claim.request_body_sha256 == group_agent_node_provider_request_sha256(body)
        && request.provider_request_bytes == body.len();
    valid
        .then_some(())
        .ok_or_else(|| invalid("scheduled dispatch authority is invalid"))
}

pub(super) fn validate_claim_event(
    event: &GroupAgentScheduledNodeDispatchClaimEvent,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    let expected = claim_event_digest(event)?;
    let valid = event.v == GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION
        && identifier(&event.graph_run_id)
        && identifier(&event.provider_request_id)
        && identifier(&event.dispatch_id)
        && identifier(&event.authorization_id)
        && digest(&event.authorization_sha256)
        && digest(&event.provider_request_sha256)
        && digest(&event.project_lane_sha256)
        && identifier(&event.node_id)
        && event.attempt == 1
        && event.expected_last_event_seq == 1
        && digest(&event.expected_last_event_sha256)
        && identifier(&event.lane_ownership_id)
        && event.event_sha256 == expected;
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid scheduled claim event"))
}

#[allow(clippy::too_many_lines)]
pub(super) fn validate_inspection(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    validate_inspection_sources(inspection)?;
    validate_inspection_evidence(inspection)?;
    validate_inspection_projections(inspection)?;
    validate_inspection_status(inspection)
}

/// Validates every frozen source artifact embedded in the inspection and the
/// exact request-body binding against the claim.
fn validate_inspection_sources(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    inspection
        .release_control
        .validate()
        .map_err(|error| invalid(&error.message))?;
    inspection
        .authorization
        .validate_against_release_control(&inspection.release_control)
        .map_err(|e| invalid(&e.message))?;
    inspection
        .pricing
        .validate()
        .map_err(|e| invalid(&e.message))?;
    inspection
        .provider_request
        .validate()
        .map_err(|e| invalid(&e.message))?;
    inspection.claim.validate()?;
    if group_agent_node_provider_request_sha256(&inspection.provider_request_body)
        != inspection.claim.request_body_sha256
        || inspection.provider_request_body.len() != inspection.claim.request_body_bytes
        || inspection.claim.provider_request_sha256
            != inspection.provider_request.prepared_request_sha256
        || inspection.claim.provider_request_id != inspection.provider_request.provider_request_id
    {
        return Err(invalid("scheduled lifecycle request binding disagrees"));
    }
    exact_json(&inspection.claim_json, &inspection.claim)
}

/// Validates the optional terminal evidence (lane, artifact, control, receipt)
/// and every cross-evidence identity binding.
fn validate_inspection_evidence(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    if let Some(lane) = &inspection.active_lane {
        lane.validate_against_claim(&inspection.claim)?;
    }
    if let Some(artifact) = &inspection.artifact {
        artifact.validate()?;
        if !artifact_matches_claim(artifact, &inspection.claim) {
            return Err(invalid("scheduled terminal artifact disagrees with claim"));
        }
    }
    if let Some(control) = &inspection.terminal_control {
        control.validate()?;
        if !control_matches_sources(control, inspection)
            || inspection.artifact.as_ref() != Some(&control.artifact)
        {
            return Err(invalid("scheduled terminal control disagrees with sources"));
        }
    }
    if let Some(receipt) = &inspection.terminal_receipt {
        receipt.validate()?;
        let control = inspection
            .terminal_control
            .as_ref()
            .ok_or_else(|| invalid("scheduled receipt has no control"))?;
        receipt.validate_against_control(control)?;
    }
    Ok(())
}

/// Validates that every persisted JSON projection is the exact canonical
/// encoding of its structured value and that projections exist exactly when
/// their structured value exists.
fn validate_inspection_projections(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    if let (Some(json), Some(lane)) = (&inspection.active_lane_json, &inspection.active_lane) {
        exact_json(json, lane)?;
    } else if inspection.active_lane.is_some() || inspection.active_lane_json.is_some() {
        return Err(invalid("scheduled lifecycle active lane JSON disagrees"));
    }
    if let (Some(json), Some(artifact)) = (&inspection.artifact_json, &inspection.artifact) {
        exact_json(json, artifact)?;
    } else if inspection.artifact.is_some() || inspection.artifact_json.is_some() {
        return Err(invalid("scheduled lifecycle artifact JSON disagrees"));
    }
    if let (Some(json), Some(control)) = (
        &inspection.terminal_control_json,
        &inspection.terminal_control,
    ) {
        exact_json(json, control)?;
    } else if inspection.terminal_control.is_some() || inspection.terminal_control_json.is_some() {
        return Err(invalid("scheduled lifecycle control JSON disagrees"));
    }
    if let (Some(json), Some(receipt)) = (
        &inspection.terminal_receipt_json,
        &inspection.terminal_receipt,
    ) {
        exact_json(json, receipt)?;
    } else if inspection.terminal_receipt.is_some() || inspection.terminal_receipt_json.is_some() {
        return Err(invalid("scheduled lifecycle receipt JSON disagrees"));
    }
    Ok(())
}

/// Validates that the lifecycle status and the presence/absence of terminal
/// evidence are mutually consistent.
fn validate_inspection_status(
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    let terminal = matches!(
        inspection.status,
        GroupAgentScheduledNodeLifecycleStatus::Terminalized
            | GroupAgentScheduledNodeLifecycleStatus::Quarantined
            | GroupAgentScheduledNodeLifecycleStatus::Adjudicated
    );
    if terminal != inspection.active_lane.is_none() {
        return Err(invalid("scheduled lifecycle lane/status disagree"));
    }
    match inspection.status {
        GroupAgentScheduledNodeLifecycleStatus::Claimed
            if inspection.artifact.is_none()
                && inspection.terminal_control.is_none()
                && inspection.terminal_receipt.is_none()
                && inspection.active_lane.is_some() => {}
        GroupAgentScheduledNodeLifecycleStatus::Terminalized
            if inspection.artifact.is_some()
                && inspection.terminal_control.is_some()
                && inspection.terminal_receipt.is_some() => {}
        GroupAgentScheduledNodeLifecycleStatus::Quarantined
            if inspection.artifact.is_some()
                && inspection.terminal_control.is_none()
                && inspection.terminal_receipt.is_none() => {}
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated
            if inspection.artifact.is_none()
                && inspection.terminal_control.is_none()
                && inspection.terminal_receipt.is_none()
                && inspection.active_lane.is_none() => {}
        _ => return Err(invalid("scheduled lifecycle evidence/status disagree")),
    }
    Ok(())
}

fn exact_json<T: Serialize>(
    json: &str,
    value: &T,
) -> Result<(), super::GroupAgentScheduledNodeLifecycleValidationError> {
    let canonical = canonical_json(value)?;
    (canonical == json)
        .then_some(())
        .ok_or_else(|| invalid("scheduled lifecycle JSON is not canonical"))
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

fn control_matches_sources(
    control: &GroupAgentScheduledNodeTerminalControl,
    inspection: &GroupAgentScheduledNodeLifecycleInspection,
) -> bool {
    control.release_control_snapshot_sha256 == inspection.release_control.snapshot_sha256
        && control.graph_run_id == inspection.claim.graph_run_id
        && control.graph_id == inspection.graph_run.run.graph_id
        && control.node_id == inspection.claim.node_id
        && control.attempt == inspection.claim.attempt
        && control.dispatch_id == inspection.claim.dispatch_id
        && control.provider_request_id == inspection.claim.provider_request_id
        && control.authorization_sha256 == inspection.claim.authorization_sha256
        && control.provider_request_sha256 == inspection.claim.provider_request_sha256
        && control.request_body_sha256 == inspection.claim.request_body_sha256
        && control.expected_last_event_seq == inspection.claim.expected_last_event_seq
        && control.expected_last_event_sha256 == inspection.claim.expected_last_event_sha256
        && control.claim_event_sha256 == inspection.claim.claim_event_sha256
        && control.project_lane_sha256 == inspection.claim.project_lane_sha256
}

fn content_id(value: &str, prefix: &str, digest_value: &str) -> bool {
    value == format!("{prefix}{digest_value}") && digest(&value[prefix.len()..])
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().all(|b| b.is_ascii_graphic() && b != b' ')
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn invalid(message: &str) -> super::GroupAgentScheduledNodeLifecycleValidationError {
    super::GroupAgentScheduledNodeLifecycleValidationError {
        message: message.into(),
    }
}

use serde::Serialize;
