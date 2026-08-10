use crate::runtime_domain::{
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION, GroupAgentNodeDispatchClaim,
    GroupAgentNodeTerminalArtifact, GroupAgentNodeTerminalArtifactKind,
    GroupAgentNodeTerminalClassification, group_agent_node_terminal_artifact_id,
    group_agent_node_terminal_output_sha256,
};

use super::GroupAgentNodeDispatchAdjudicationServiceError;

/// Builds the operator-constructed hard-crash artifact: classification
/// `HardCrash` with the all-false no-evidence shape accepted by both
/// validators (`provider_poll_started ∨ (¬terminal_seen ∧ ¬stream_eof_seen)`),
/// no usage, and no cost. All immutable digests come from the stored claim.
pub(super) fn build_hard_crash_artifact(
    claim: &GroupAgentNodeDispatchClaim,
    created_at_ms: u64,
) -> Result<GroupAgentNodeTerminalArtifact, GroupAgentNodeDispatchAdjudicationServiceError> {
    let mut artifact = GroupAgentNodeTerminalArtifact {
        v: GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION,
        terminal_artifact_protocol_version: GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION,
        artifact_kind: GroupAgentNodeTerminalArtifactKind::Uncertainty,
        graph_run_id: claim.graph_run_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        dispatch_request_sha256: claim.dispatch_request_sha256.clone(),
        logical_request_sha256: claim.logical_request_sha256.clone(),
        request_body_sha256: claim.request_body_sha256.clone(),
        pricing_snapshot_sha256: claim.pricing_snapshot_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        provider_poll_started: false,
        terminal_seen: false,
        stream_eof_seen: false,
        classification: GroupAgentNodeTerminalClassification::HardCrash,
        output_text: String::new(),
        output_bytes: 0,
        output_sha256: group_agent_node_terminal_output_sha256(""),
        usage_observed: false,
        input_tokens: 0,
        output_tokens: 0,
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    };
    artifact.artifact_bytes = artifact
        .canonical_payload_json()
        .map_err(|_| invalid())?
        .len();
    artifact.artifact_sha256 = artifact.expected_sha256().map_err(|_| invalid())?;
    artifact.artifact_id = group_agent_node_terminal_artifact_id(&artifact.artifact_sha256);
    artifact.validate().map_err(|_| invalid())?;
    Ok(artifact)
}

fn invalid() -> GroupAgentNodeDispatchAdjudicationServiceError {
    GroupAgentNodeDispatchAdjudicationServiceError::InvalidInput
}
