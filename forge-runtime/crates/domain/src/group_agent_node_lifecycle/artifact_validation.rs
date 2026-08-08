use super::{
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION, GroupAgentNodeActiveLane,
    GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleValidationError,
    GroupAgentNodeTerminalArtifact, GroupAgentNodeTerminalArtifactKind,
    GroupAgentNodeTerminalClassification, MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES, codec,
    group_agent_node_terminal_artifact_id, group_agent_node_terminal_output_sha256,
};
use crate::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeExecutionContract,
    GroupAgentNodePricingSnapshot, MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES,
    MAX_GROUP_AGENT_NODE_RESULT_BYTES,
};

use super::validation::{invalid, is_digest, valid_identifier};

pub(super) fn validate_artifact(
    artifact: &GroupAgentNodeTerminalArtifact,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    validate_header(artifact)?;
    validate_output(artifact)?;
    validate_evidence_flags(artifact)?;
    validate_artifact_identity(artifact)
}

fn validate_header(
    artifact: &GroupAgentNodeTerminalArtifact,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let valid = artifact.v == GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION
        && artifact.terminal_artifact_protocol_version
            == GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION
        && valid_identifier(&artifact.graph_run_id)
        && valid_identifier(&artifact.node_id)
        && artifact.attempt == 1
        && valid_identifier(&artifact.dispatch_id)
        && is_digest(&artifact.claim_event_sha256)
        && is_digest(&artifact.authorization_sha256)
        && is_digest(&artifact.dispatch_request_sha256)
        && is_digest(&artifact.logical_request_sha256)
        && is_digest(&artifact.request_body_sha256)
        && is_digest(&artifact.pricing_snapshot_sha256)
        && valid_identifier(&artifact.lane_ownership_id)
        && is_digest(&artifact.project_lane_sha256)
        && !artifact.retry_authorized
        && i64::try_from(artifact.created_at_ms).is_ok();
    valid
        .then_some(())
        .ok_or_else(|| invalid("invalid Group Agent Node terminal artifact header"))
}

fn validate_output(
    artifact: &GroupAgentNodeTerminalArtifact,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let valid = artifact.output_text.len() == artifact.output_bytes
        && artifact.output_bytes <= MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES
        && artifact.output_bytes <= MAX_GROUP_AGENT_NODE_RESULT_BYTES
        && artifact.output_sha256 == group_agent_node_terminal_output_sha256(&artifact.output_text);
    valid
        .then_some(())
        .ok_or_else(|| invalid("terminal artifact output identity or bound disagrees"))
}

fn validate_evidence_flags(
    artifact: &GroupAgentNodeTerminalArtifact,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    validate_observed_usage(artifact)?;
    let valid = match artifact.artifact_kind {
        GroupAgentNodeTerminalArtifactKind::Result => valid_result(artifact),
        GroupAgentNodeTerminalArtifactKind::Uncertainty => valid_uncertainty(artifact),
    };
    valid
        .then_some(())
        .ok_or_else(|| invalid("terminal artifact classification and evidence flags disagree"))
}

fn validate_observed_usage(
    artifact: &GroupAgentNodeTerminalArtifact,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let usage = if artifact.usage_observed {
        artifact.input_tokens >= 1 && artifact.output_tokens >= 1
    } else {
        artifact.input_tokens == 0 && artifact.output_tokens == 0
    };
    let cost = if artifact.actual_cost_calculated {
        artifact.usage_observed && artifact.actual_cost_usd_micros >= 1
    } else {
        artifact.actual_cost_usd_micros == 0
    };
    (usage && cost)
        .then_some(())
        .ok_or_else(|| invalid("terminal artifact usage or actual cost flags disagree"))
}

fn valid_result(artifact: &GroupAgentNodeTerminalArtifact) -> bool {
    let result_class = matches!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::Completed
            | GroupAgentNodeTerminalClassification::Length
    );
    let completed_output = artifact.classification
        != GroupAgentNodeTerminalClassification::Completed
        || !artifact.output_text.is_empty();
    result_class
        && completed_output
        && artifact.provider_poll_started
        && artifact.terminal_seen
        && artifact.stream_eof_seen
        && artifact.usage_observed
        && artifact.actual_cost_calculated
}

fn valid_uncertainty(artifact: &GroupAgentNodeTerminalArtifact) -> bool {
    let class = matches!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::ProviderError
            | GroupAgentNodeTerminalClassification::HttpError
            | GroupAgentNodeTerminalClassification::TransportError
            | GroupAgentNodeTerminalClassification::Timeout
            | GroupAgentNodeTerminalClassification::Cancelled
            | GroupAgentNodeTerminalClassification::EofBeforeTerminal
            | GroupAgentNodeTerminalClassification::MissingUsage
            | GroupAgentNodeTerminalClassification::ToolCall
            | GroupAgentNodeTerminalClassification::ProtocolError
            | GroupAgentNodeTerminalClassification::TrailingData
            | GroupAgentNodeTerminalClassification::LocalLimit
            | GroupAgentNodeTerminalClassification::HardCrash
    );
    let chronology =
        artifact.provider_poll_started || (!artifact.terminal_seen && !artifact.stream_eof_seen);
    let missing_usage = artifact.classification
        != GroupAgentNodeTerminalClassification::MissingUsage
        || (artifact.terminal_seen
            && artifact.stream_eof_seen
            && !artifact.usage_observed
            && !artifact.actual_cost_calculated);
    class && chronology && missing_usage
}

fn validate_artifact_identity(
    artifact: &GroupAgentNodeTerminalArtifact,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let payload = codec::artifact_payload_json(artifact)?;
    let digest = codec::artifact_digest(artifact)?;
    let valid = (1..=MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES).contains(&payload.len())
        && artifact.artifact_bytes == payload.len()
        && artifact.artifact_sha256 == digest
        && artifact.artifact_id == group_agent_node_terminal_artifact_id(&digest)
        && artifact.canonical_json()?.len() <= MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES;
    valid
        .then_some(())
        .ok_or_else(|| invalid("terminal artifact content identity disagrees"))
}

pub(super) fn validate_artifact_against_claim(
    artifact: &GroupAgentNodeTerminalArtifact,
    claim: &GroupAgentNodeDispatchClaim,
    lane: &GroupAgentNodeActiveLane,
    authorization: &GroupAgentNodeDispatchAuthorization,
    pricing: &GroupAgentNodePricingSnapshot,
    contract: &GroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    validate_artifact_against_persisted_claim(artifact, claim)?;
    lane.validate_against_claim(claim)?;
    authorization
        .validate()
        .map_err(|error| invalid(&error.message))?;
    pricing
        .verify_authorization(authorization)
        .map_err(|error| invalid(&error.message))?;
    contract
        .validate()
        .map_err(|error| invalid(&error.message))?;
    if pricing.pricing_snapshot_sha256 != claim.pricing_snapshot_sha256 {
        return Err(invalid("terminal artifact pricing and claim disagree"));
    }
    validate_authorization_and_contract_bindings(claim, authorization, contract)?;
    validate_contract_output_bound(artifact, contract)?;
    if artifact.usage_observed {
        let checked_cost = pricing
            .actual_cost_usd_micros(artifact.input_tokens, artifact.output_tokens, authorization)
            .map_err(|error| invalid(&error.message))?;
        if artifact.actual_cost_calculated && checked_cost != artifact.actual_cost_usd_micros {
            return Err(invalid("terminal artifact actual cost disagrees"));
        }
    }
    Ok(())
}

pub(super) fn validate_artifact_against_persisted_claim(
    artifact: &GroupAgentNodeTerminalArtifact,
    claim: &GroupAgentNodeDispatchClaim,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    validate_artifact(artifact)?;
    claim.validate()?;
    validate_claim_bindings(artifact, claim)
}

fn validate_claim_bindings(
    artifact: &GroupAgentNodeTerminalArtifact,
    claim: &GroupAgentNodeDispatchClaim,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let exact = artifact.graph_run_id == claim.graph_run_id
        && artifact.node_id == claim.node_id
        && artifact.attempt == claim.attempt
        && artifact.dispatch_id == claim.dispatch_id
        && artifact.claim_event_sha256 == claim.claim_event_sha256
        && artifact.authorization_sha256 == claim.authorization_sha256
        && artifact.dispatch_request_sha256 == claim.dispatch_request_sha256
        && artifact.logical_request_sha256 == claim.logical_request_sha256
        && artifact.request_body_sha256 == claim.request_body_sha256
        && artifact.pricing_snapshot_sha256 == claim.pricing_snapshot_sha256
        && artifact.lane_ownership_id == claim.lane_ownership_id
        && artifact.project_lane_sha256 == claim.project_lane_sha256
        && artifact.created_at_ms >= claim.released_at_ms;
    exact
        .then_some(())
        .ok_or_else(|| invalid("terminal artifact immutable claim bindings disagree"))
}

fn validate_authorization_and_contract_bindings(
    claim: &GroupAgentNodeDispatchClaim,
    authorization: &GroupAgentNodeDispatchAuthorization,
    contract: &GroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let exact = authorization.graph_run_id == claim.graph_run_id
        && authorization.authorization_id == claim.authorization_id
        && authorization.authorization_sha256 == claim.authorization_sha256
        && authorization.dispatch_request_id == claim.dispatch_request_id
        && authorization.dispatch_request_sha256 == claim.dispatch_request_sha256
        && authorization.logical_request_sha256 == claim.logical_request_sha256
        && authorization.request_body_sha256 == claim.request_body_sha256
        && authorization.request_body_bytes == claim.request_body_bytes
        && authorization.pricing_snapshot_sha256 == claim.pricing_snapshot_sha256
        && authorization.node_id == claim.node_id
        && authorization.attempt == claim.attempt
        && authorization.budgets.max_cost_usd_micros == claim.max_cost_usd_micros
        && authorization.release_requirements.consent_contract_version
            == claim.consent_contract_version
        && authorization.project_lane_sha256 == claim.project_lane_sha256
        && contract.graph_run_id == claim.graph_run_id
        && contract.graph_id == authorization.graph_id
        && contract.source_snapshot_sha256 == authorization.source_snapshot_sha256
        && contract.graph_manifest_sha256 == authorization.graph_manifest_sha256
        && contract.core_plan_sha256 == authorization.core_plan_sha256
        && contract.contract_id == authorization.contract_id
        && contract.contract_sha256 == authorization.contract_sha256
        && contract.node.node_id == claim.node_id
        && contract.node.attempt == claim.attempt
        && contract.node.project_id == authorization.project_id
        && contract.node.project_lane_sha256 == claim.project_lane_sha256
        && contract.node.same_project_policy == authorization.same_project_policy
        && contract.provider.kind == authorization.provider_kind
        && contract.provider.endpoint == authorization.endpoint
        && contract.provider.model == authorization.model
        && contract.request.request_sha256 == claim.logical_request_sha256
        && contract.budgets == authorization.budgets
        && contract.failure == authorization.failure;
    exact
        .then_some(())
        .ok_or_else(|| invalid("terminal artifact authorization or contract bindings disagree"))
}

fn validate_contract_output_bound(
    artifact: &GroupAgentNodeTerminalArtifact,
    contract: &GroupAgentNodeExecutionContract,
) -> Result<(), GroupAgentNodeLifecycleValidationError> {
    let valid = artifact.output_bytes <= contract.budgets.max_model_output_bytes
        && artifact.output_bytes <= contract.result.max_result_bytes;
    valid
        .then_some(())
        .ok_or_else(|| invalid("terminal artifact exceeds its contract output bound"))
}
