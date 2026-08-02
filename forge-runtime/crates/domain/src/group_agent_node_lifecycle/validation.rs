use serde::{Serialize, de::DeserializeOwned};

use super::{
    ClaimGroupAgentNodeDispatch, GROUP_AGENT_NODE_ACTIVE_LANE_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION, GROUP_AGENT_NODE_LIFECYCLE_VERSION,
    GroupAgentNodeActiveLane, GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleValidationError,
    GroupAgentNodeTerminalArtifact, GroupAgentNodeTerminalControl, GroupAgentNodeTerminalReceipt,
    MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES, MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES,
    MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES, artifact_validation, inspection_validation,
    terminal_validation,
};
use crate::{
    GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentNodeDispatchRequestRecord,
    MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, MAX_GROUP_AGENT_NODE_COST_USD_MICROS,
    MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES, group_agent_node_dispatch_authorization_id,
    group_agent_node_dispatch_request_id, group_agent_node_provider_request_sha256,
};

const MAX_CLAIM_JSON_BYTES: usize = 64 * 1024;
const MAX_ACTIVE_LANE_JSON_BYTES: usize = 16 * 1024;

pub(super) fn decode_exact_claim(
    bytes: &[u8],
) -> Result<GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleValidationError> {
    let value = decode_json(bytes, MAX_CLAIM_JSON_BYTES, "dispatch claim")?;
    validate_claim(&value)?;
    validate_exact_bytes(&value, bytes)?;
    Ok(value)
}

pub(super) fn decode_exact_active_lane(
    bytes: &[u8],
) -> Result<GroupAgentNodeActiveLane, GroupAgentNodeLifecycleValidationError> {
    let value = decode_json(bytes, MAX_ACTIVE_LANE_JSON_BYTES, "active lane")?;
    validate_active_lane_shape(&value)?;
    validate_exact_bytes(&value, bytes)?;
    Ok(value)
}

pub(super) fn decode_exact_artifact(
    bytes: &[u8],
) -> Result<GroupAgentNodeTerminalArtifact, GroupAgentNodeLifecycleValidationError> {
    let value = decode_json(
        bytes,
        MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES,
        "terminal artifact",
    )?;
    validate_artifact(&value)?;
    validate_exact_bytes(&value, bytes)?;
    Ok(value)
}

pub(super) fn decode_exact_control(
    bytes: &[u8],
) -> Result<GroupAgentNodeTerminalControl, GroupAgentNodeLifecycleValidationError> {
    let value = decode_json(
        bytes,
        MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES,
        "terminal control",
    )?;
    validate_terminal_control(&value)?;
    validate_exact_bytes(&value, bytes)?;
    Ok(value)
}

pub(super) fn decode_exact_receipt(
    bytes: &[u8],
) -> Result<GroupAgentNodeTerminalReceipt, GroupAgentNodeLifecycleValidationError> {
    let value = decode_json(
        bytes,
        MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES,
        "terminal receipt",
    )?;
    validate_terminal_receipt(&value)?;
    validate_exact_bytes(&value, bytes)?;
    Ok(value)
}

pub(super) fn validate_claim(
    claim: &GroupAgentNodeDispatchClaim,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let valid = claim.v == GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION
        && valid_identifier(&claim.graph_run_id)
        && valid_identifier(&claim.dispatch_id)
        && claim.authorization_id
            == group_agent_node_dispatch_authorization_id(&claim.authorization_sha256)
        && is_digest(&claim.authorization_sha256)
        && claim.dispatch_request_id
            == group_agent_node_dispatch_request_id(&claim.dispatch_request_sha256)
        && is_digest(&claim.dispatch_request_sha256)
        && is_digest(&claim.logical_request_sha256)
        && is_digest(&claim.request_body_sha256)
        && (1..=MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES).contains(&claim.request_body_bytes)
        && is_digest(&claim.pricing_snapshot_sha256)
        && valid_identifier(&claim.node_id)
        && claim.attempt == 1
        && (1..=MAX_GROUP_AGENT_NODE_COST_USD_MICROS).contains(&claim.max_cost_usd_micros)
        && claim.consent_contract_version == GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION
        && valid_identifier(&claim.lane_ownership_id)
        && is_digest(&claim.project_lane_sha256)
        && claim.expected_last_event_seq == 3
        && is_digest(&claim.expected_last_event_sha256)
        && is_digest(&claim.claim_event_sha256)
        && i64::try_from(claim.released_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Node dispatch claim"))
}

pub(super) fn validate_active_lane(
    lane: &GroupAgentNodeActiveLane,
    claim: &GroupAgentNodeDispatchClaim,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    validate_claim(claim)?;
    validate_active_lane_shape(lane)?;
    let exact = lane.project_lane_sha256 == claim.project_lane_sha256
        && lane.lane_ownership_id == claim.lane_ownership_id
        && lane.graph_run_id == claim.graph_run_id
        && lane.node_id == claim.node_id
        && lane.attempt == claim.attempt
        && lane.dispatch_id == claim.dispatch_id
        && lane.claim_event_sha256 == claim.claim_event_sha256
        && lane.claimed_at_ms == claim.released_at_ms;
    exact
        .then_some(())
        .ok_or_else(|| invalid("active Project lane disagrees with its dispatch claim"))
}

fn validate_active_lane_shape(
    lane: &GroupAgentNodeActiveLane,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let valid = lane.v == GROUP_AGENT_NODE_ACTIVE_LANE_VERSION
        && is_digest(&lane.project_lane_sha256)
        && valid_identifier(&lane.lane_ownership_id)
        && valid_identifier(&lane.graph_run_id)
        && valid_identifier(&lane.node_id)
        && lane.attempt == 1
        && valid_identifier(&lane.dispatch_id)
        && is_digest(&lane.claim_event_sha256)
        && i64::try_from(lane.claimed_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid active Group Agent Node Project lane"))
}

pub(super) fn validate_claim_request(
    request: &ClaimGroupAgentNodeDispatch,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    if request.v != GROUP_AGENT_NODE_LIFECYCLE_VERSION {
        return Err(invalid("unsupported Group Agent Node lifecycle version"));
    }
    validate_exact_json(&request.release_control, &request.release_control_json)?;
    validate_exact_json(&request.authorization, &request.authorization_json)?;
    validate_exact_json(&request.pricing, &request.pricing_json)?;
    validate_exact_json(&request.claim, &request.claim_json)?;
    validate_exact_json(&request.active_lane, &request.active_lane_json)?;
    validate_exact_json(&request.event, &request.event_json)?;
    validate_claim_against_sources(
        &request.claim,
        &request.event,
        &request.release_control,
        &request.authorization,
        &request.pricing,
    )?;
    validate_active_lane(&request.active_lane, &request.claim)?;
    validate_single_node(&request.release_control)
}

pub(super) fn validate_claim_against_sources(
    claim: &GroupAgentNodeDispatchClaim,
    event: &GroupAgentGraphRunEvent,
    control: &crate::GroupAgentNodeDispatchReleaseControl,
    authorization: &crate::GroupAgentNodeDispatchAuthorization,
    pricing: &crate::GroupAgentNodePricingSnapshot,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    control
        .validate()
        .map_err(|error| invalid(&error.message))?;
    authorization
        .validate_against_release_control(control)
        .map_err(|error| invalid(&error.message))?;
    pricing
        .verify_authorization(authorization)
        .map_err(|error| invalid(&error.message))?;
    let dispatch = &control.dispatch_request;
    validate_claim(claim)?;
    let exact = claim.graph_run_id == control.graph_run.graph_run_id
        && claim.authorization_id == authorization.authorization_id
        && claim.authorization_sha256 == authorization.authorization_sha256
        && claim.dispatch_request_id == dispatch.dispatch_request_id
        && claim.dispatch_request_sha256 == dispatch.dispatch_request_sha256
        && claim.logical_request_sha256 == dispatch.request_sha256
        && claim.request_body_sha256 == dispatch.provider_request_sha256
        && claim.request_body_bytes == dispatch.provider_request_bytes
        && claim.pricing_snapshot_sha256 == pricing.pricing_snapshot_sha256
        && claim.node_id == dispatch.node_id
        && claim.attempt == dispatch.attempt
        && claim.max_cost_usd_micros == authorization.budgets.max_cost_usd_micros
        && claim.consent_contract_version
            == authorization.release_requirements.consent_contract_version
        && claim.project_lane_sha256 == dispatch.project_lane_sha256
        && claim.expected_last_event_sha256 == authorization.expected_last_event_sha256
        && claim.released_at_ms >= dispatch.created_at_ms;
    if !exact {
        return Err(invalid("dispatch claim source bindings disagree"));
    }
    validate_claim_event(claim, event)
}

pub(super) fn validate_claim_event(
    claim: &GroupAgentNodeDispatchClaim,
    event: &GroupAgentGraphRunEvent,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    event.validate().map_err(|error| invalid(&error.message))?;
    let envelope = event.v == GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION
        && event.graph_run_id == claim.graph_run_id
        && event.seq == 4
        && event.expected_sha256().as_deref() == Ok(claim.claim_event_sha256.as_str());
    (envelope && claim_event_fields_match(claim, &event.kind))
        .then_some(())
        .ok_or_else(|| invalid("seq-4 release event disagrees with its dispatch claim"))
}

fn claim_event_fields_match(
    claim: &GroupAgentNodeDispatchClaim,
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
    previous_event_sha256 == &claim.expected_last_event_sha256
        && dispatch_id == &claim.dispatch_id
        && authorization_id == &claim.authorization_id
        && authorization_sha256 == &claim.authorization_sha256
        && dispatch_request_id == &claim.dispatch_request_id
        && dispatch_request_sha256 == &claim.dispatch_request_sha256
        && logical_request_sha256 == &claim.logical_request_sha256
        && request_body_sha256 == &claim.request_body_sha256
        && request_body_bytes == &claim.request_body_bytes
        && pricing_snapshot_sha256 == &claim.pricing_snapshot_sha256
        && node_id == &claim.node_id
        && attempt == &claim.attempt
        && max_cost_usd_micros == &claim.max_cost_usd_micros
        && consent_contract_version == &claim.consent_contract_version
        && lane_ownership_id == &claim.lane_ownership_id
        && project_lane_sha256 == &claim.project_lane_sha256
        && released_at_ms == &claim.released_at_ms
}

pub(super) fn validate_dispatch_authority(
    dispatch: &GroupAgentNodeDispatchRequestRecord,
    claim: &GroupAgentNodeDispatchClaim,
    event: &GroupAgentGraphRunEvent,
    body: &[u8],
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    dispatch
        .validate()
        .map_err(|error| invalid(&error.message))?;
    validate_claim_event(claim, event)?;
    let exact = claim.dispatch_request_id == dispatch.dispatch_request_id
        && claim.dispatch_request_sha256 == dispatch.dispatch_request_sha256
        && claim.logical_request_sha256 == dispatch.request_sha256
        && claim.request_body_sha256 == dispatch.provider_request_sha256
        && claim.request_body_bytes == body.len()
        && dispatch.provider_request_bytes == body.len()
        && group_agent_node_provider_request_sha256(body) == claim.request_body_sha256;
    exact
        .then_some(())
        .ok_or_else(|| invalid("dispatch authority body disagrees with its durable claim"))
}

pub(super) fn validate_artifact(
    value: &GroupAgentNodeTerminalArtifact,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    artifact_validation::validate_artifact(value)
}

pub(super) use artifact_validation::validate_artifact_against_claim;
pub(super) use inspection_validation::validate_lifecycle_inspection;
pub(super) use terminal_validation::{
    validate_receipt_against_control, validate_terminal_control, validate_terminal_receipt,
    validate_terminalize_request,
};

pub(super) fn validate_exact_json(
    value: &impl Serialize,
    json: &str,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    validate_exact_bytes(value, json.as_bytes())
}

fn validate_exact_bytes(
    value: &impl Serialize,
    bytes: &[u8],
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let canonical = super::codec::canonical_json(value)?;
    (canonical.as_bytes() == bytes)
        .then_some(())
        .ok_or_else(|| invalid("input is not exact compact canonical JSON"))
}

fn decode_json<T: DeserializeOwned>(
    bytes: &[u8],
    maximum: usize,
    kind: &str,
) -> Result<T, GroupAgentNodeLifecycleValidationError> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(invalid(&format!("{kind} is outside its byte bound")));
    }
    serde_json::from_slice(bytes).map_err(|_| invalid(&format!("{kind} is invalid JSON")))
}

pub(super) fn validate_single_node(
    control: &crate::GroupAgentNodeDispatchReleaseControl,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let node_id = control.contract.node.node_id.as_str();
    let valid = control.plan.authored_node_ids.as_slice() == [node_id]
        && control.plan.edges.is_empty()
        && control.plan.waves.as_slice() == [vec![node_id.to_owned()]]
        && control.manifest.nodes.len() == 1
        && control.manifest.nodes[0].node_id == node_id
        && control.manifest.edges.is_empty()
        && control.manifest.waves.as_slice() == [vec![node_id.to_owned()]];
    valid
        .then_some(())
        .ok_or_else(|| invalid("effectful dispatch requires exactly one graph node and wave"))
}

pub(super) fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES)
}

pub(super) fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}

pub(super) fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn invalid(message: &str) -> GroupAgentNodeLifecycleValidationError {
    GroupAgentNodeLifecycleValidationError {
        message: message.into(),
    }
}
