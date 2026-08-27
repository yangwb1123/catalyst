#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn build_claim(
    preflight: &Preflight,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
) -> Result<
    GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchExecutionServiceError,
> {
    let authorization = &preflight.authorization;
    let claim = GroupAgentScheduledNodeDispatchClaim {
        v: 1,
        graph_run_id: authorization.graph_run_id.clone(),
        provider_request_id: authorization.scheduled_provider_request_id.clone(),
        dispatch_id: format!(
            "scheduled-node-dispatch-{}",
            authorization.authorization_sha256
        ),
        authorization_id: authorization.authorization_id.clone(),
        authorization_sha256: authorization.authorization_sha256.clone(),
        provider_request_sha256: authorization.scheduled_provider_request_sha256.clone(),
        request_body_sha256: authorization.request_body_sha256.clone(),
        request_body_bytes: authorization.request_body_bytes,
        pricing_snapshot_sha256: authorization.pricing_snapshot_sha256.clone(),
        node_id: authorization.node_id.clone(),
        attempt: authorization.attempt,
        max_cost_usd_micros: authorization.budgets.max_cost_usd_micros,
        lane_ownership_id: metadata.lane_ownership_id.clone(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: authorization.expected_last_event_seq,
        expected_last_event_sha256: authorization.expected_last_event_sha256.clone(),
        claim_event_sha256: String::new(),
        released_at_ms: metadata.released_at_ms,
    };
    let mut event = build_claim_event(&claim);
    event.event_sha256 = event
        .expected_sha256()
        .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
    let mut claim = claim;
    claim.claim_event_sha256 = event.event_sha256;
    claim
        .validate()
        .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
    Ok(claim)
}

pub(super) fn build_claim_event(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeDispatchClaimEvent {
    GroupAgentScheduledNodeDispatchClaimEvent {
        v: 1,
        graph_run_id: claim.graph_run_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        dispatch_id: claim.dispatch_id.clone(),
        authorization_id: claim.authorization_id.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        provider_request_sha256: claim.provider_request_sha256.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        expected_last_event_seq: claim.expected_last_event_seq,
        expected_last_event_sha256: claim.expected_last_event_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        released_at_ms: claim.released_at_ms,
        event_sha256: String::new(),
    }
}

pub(super) fn build_lane(
    claim: &GroupAgentScheduledNodeDispatchClaim,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
) -> GroupAgentScheduledNodeActiveLane {
    GroupAgentScheduledNodeActiveLane {
        v: 1,
        project_lane_sha256: claim.project_lane_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        graph_run_id: claim.graph_run_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        claimed_at_ms: metadata.released_at_ms,
    }
}

#[allow(clippy::too_many_lines)]
pub(crate) fn build_artifact(
    claim: &GroupAgentScheduledNodeDispatchClaim,
    evidence: &CollectedDispatchEvidence,
    terminalized_at_ms: u64,
) -> Result<
    GroupAgentScheduledNodeTerminalArtifact,
    GroupAgentScheduledNodeDispatchExecutionServiceError,
> {
    let artifact_kind =
        if evidence.classification == GroupAgentNodeTerminalClassification::Completed {
            GroupAgentScheduledNodeTerminalArtifactKind::Result
        } else {
            GroupAgentScheduledNodeTerminalArtifactKind::Uncertainty
        };
    let mut artifact = artifact_fields(claim, evidence, artifact_kind, terminalized_at_ms);
    artifact.output_bytes = artifact.output_text.len();
    finalize_artifact(&mut artifact)?;
    Ok(artifact)
}

/// Builds the terminal artifact struct from the claim bindings and collected
/// provider evidence; digest/id bookkeeping is applied by `finalize_artifact`.
fn artifact_fields(
    claim: &GroupAgentScheduledNodeDispatchClaim,
    evidence: &CollectedDispatchEvidence,
    artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind,
    terminalized_at_ms: u64,
) -> GroupAgentScheduledNodeTerminalArtifact {
    GroupAgentScheduledNodeTerminalArtifact {
        v: 1,
        terminal_artifact_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        artifact_kind,
        graph_run_id: claim.graph_run_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        provider_request_sha256: claim.provider_request_sha256.clone(),
        request_body_sha256: claim.request_body_sha256.clone(),
        pricing_snapshot_sha256: claim.pricing_snapshot_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        provider_poll_started: evidence.provider_poll_started,
        terminal_seen: evidence.terminal_seen,
        stream_eof_seen: evidence.stream_eof_seen,
        classification: evidence.classification,
        output_text: evidence.output.clone(),
        output_bytes: 0,
        output_sha256: group_agent_scheduled_node_terminal_output_sha256(&evidence.output),
        usage_observed: evidence.usage.is_some(),
        input_tokens: evidence.usage.map_or(0, |usage| usage.input_tokens),
        output_tokens: evidence.usage.map_or(0, |usage| usage.output_tokens),
        actual_cost_calculated: false,
        actual_cost_usd_micros: 0,
        retry_authorized: false,
        created_at_ms: terminalized_at_ms,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    }
}

/// Applies digest, content-addressed ID and canonical-byte bookkeeping, then
/// runs the domain validator over the finished artifact.
fn finalize_artifact(
    artifact: &mut GroupAgentScheduledNodeTerminalArtifact,
) -> Result<(), GroupAgentScheduledNodeDispatchExecutionServiceError> {
    artifact.output_bytes = artifact.output_text.len();
    let digest = artifact.expected_sha256().map_err(|_| {
        GroupAgentScheduledNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
    })?;
    artifact.artifact_sha256.clone_from(&digest);
    artifact.artifact_id = format!("scheduled-node-terminal-artifact-{digest}");
    settle_artifact_bytes(artifact)?;
    artifact.validate().map_err(|_| {
        GroupAgentScheduledNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
    })
}

fn settle_artifact_bytes(
    artifact: &mut GroupAgentScheduledNodeTerminalArtifact,
) -> Result<(), GroupAgentScheduledNodeDispatchExecutionServiceError> {
    for _ in 0..8 {
        let bytes = artifact
            .canonical_json()
            .map_err(|_| {
                GroupAgentScheduledNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
            })?
            .len();
        if artifact.artifact_bytes == bytes {
            return Ok(());
        }
        artifact.artifact_bytes = bytes;
    }
    Err(GroupAgentScheduledNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain)
}

pub(super) fn build_claim_request(
    preflight: &Preflight,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
) -> Result<
    ClaimGroupAgentScheduledNodeDispatch,
    GroupAgentScheduledNodeDispatchExecutionServiceError,
> {
    let claim = build_claim(preflight, metadata)?;
    let claim_json = claim
        .canonical_json()
        .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
    let active_lane = build_lane(&claim, metadata);
    let active_lane_json = serde_json::to_string(&active_lane)
        .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
    let mut claim_event = build_claim_event(&claim);
    claim_event.event_sha256 = claim_event
        .expected_sha256()
        .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
    let claim_event_json = claim_event
        .canonical_json()
        .map_err(|_| GroupAgentScheduledNodeDispatchExecutionServiceError::InvalidInput)?;
    Ok(ClaimGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        release_control: preflight.export.release_control.clone(),
        release_control_json: preflight.export.canonical_json.clone(),
        authorization: preflight.authorization.clone(),
        authorization_json: preflight.authorization_json.clone(),
        pricing: preflight.pricing.clone(),
        pricing_json: preflight.pricing_json.clone(),
        provider_request: preflight.export.release_control.provider_request.clone(),
        provider_request_body: preflight
            .export
            .release_control
            .provider_request_json
            .as_bytes()
            .to_vec(),
        claim,
        claim_json,
        active_lane,
        active_lane_json,
        claim_event,
        claim_event_json,
    })
}

pub(super) fn build_control(
    release: &GroupAgentScheduledNodeDispatchReleaseControl,
    claim: &GroupAgentScheduledNodeDispatchClaim,
    artifact: GroupAgentScheduledNodeTerminalArtifact,
) -> Result<
    GroupAgentScheduledNodeTerminalControl,
    GroupAgentScheduledNodeDispatchExecutionServiceError,
> {
    let mut control = GroupAgentScheduledNodeTerminalControl {
        v: 1,
        scheduler_protocol_version: release.scheduler_protocol_version,
        terminal_control_protocol_version: GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
        release_control_snapshot_sha256: release.snapshot_sha256.clone(),
        graph_run_id: claim.graph_run_id.clone(),
        graph_id: release.graph_run.graph_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        provider_request_sha256: claim.provider_request_sha256.clone(),
        request_body_sha256: claim.request_body_sha256.clone(),
        expected_last_event_seq: claim.expected_last_event_seq,
        expected_last_event_sha256: claim.expected_last_event_sha256.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        artifact,
        snapshot_sha256: String::new(),
    };
    control.snapshot_sha256 = control.expected_sha256().map_err(|_| {
        GroupAgentScheduledNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
    })?;
    control.validate().map_err(|_| {
        GroupAgentScheduledNodeDispatchExecutionServiceError::PostClaimOutcomeUncertain
    })?;
    Ok(control)
}

pub(crate) fn limits(
    budgets: &crate::runtime_domain::GroupAgentNodeExecutionBudgets,
    max_result_bytes: usize,
) -> DispatchCollectionLimits {
    DispatchCollectionLimits {
        model_output_bytes: budgets.max_model_output_bytes,
        result_bytes: max_result_bytes,
        output_tokens: budgets.max_output_tokens,
        events: budgets.max_model_events,
        timeout_ms: budgets.timeout_ms,
    }
}
