use crate::runtime_domain::{
    ClaimGroupAgentNodeDispatch, GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION,
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GROUP_AGENT_NODE_ACTIVE_LANE_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION, GROUP_AGENT_NODE_LIFECYCLE_VERSION,
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION, GROUP_AGENT_NODE_TERMINAL_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_TERMINAL_CONTROL_VERSION, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentNodeActiveLane,
    GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleInspection, GroupAgentNodePricingSnapshot,
    GroupAgentNodeTerminalArtifact, GroupAgentNodeTerminalArtifactKind,
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalControl,
    TerminalizeGroupAgentNodeDispatch, group_agent_node_terminal_artifact_id,
    group_agent_node_terminal_output_sha256,
};

use crate::ExportGroupAgentNodeDispatchReleaseControl;

use super::{
    GroupAgentNodeDispatchClaimMetadata, GroupAgentNodeDispatchExecutionServiceError,
    collector::CollectedDispatchEvidence,
};

pub(super) fn build_claim_request(
    export: ExportGroupAgentNodeDispatchReleaseControl,
    authorization: GroupAgentNodeDispatchAuthorization,
    authorization_json: String,
    pricing: GroupAgentNodePricingSnapshot,
    pricing_json: String,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
) -> Result<ClaimGroupAgentNodeDispatch, GroupAgentNodeDispatchExecutionServiceError> {
    let control = &export.release_control;
    let previous = control.journal_events[2]
        .expected_sha256()
        .map_err(|_| invalid())?;
    let event = claim_event(control, &authorization, metadata, previous);
    let event_json = event.canonical_json().map_err(|_| invalid())?;
    let event_sha256 = event.expected_sha256().map_err(|_| invalid())?;
    let claim = claim(control, &authorization, metadata, &event_sha256);
    let active_lane = active_lane(&claim);
    let request = ClaimGroupAgentNodeDispatch {
        v: GROUP_AGENT_NODE_LIFECYCLE_VERSION,
        release_control: export.release_control,
        release_control_json: export.canonical_json,
        authorization,
        authorization_json,
        pricing,
        pricing_json,
        claim_json: claim.canonical_json().map_err(|_| invalid())?,
        active_lane_json: active_lane.canonical_json().map_err(|_| invalid())?,
        claim,
        active_lane,
        event,
        event_json,
    };
    request.validate().map_err(|_| invalid())?;
    Ok(request)
}

fn claim_event(
    control: &crate::runtime_domain::GroupAgentNodeDispatchReleaseControl,
    authorization: &GroupAgentNodeDispatchAuthorization,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
    previous_event_sha256: String,
) -> GroupAgentGraphRunEvent {
    let request = &control.dispatch_request;
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION,
        graph_run_id: control.graph_run.graph_run_id.clone(),
        seq: 4,
        kind: GroupAgentGraphRunEventKind::NodeDispatchReleased {
            previous_event_sha256,
            dispatch_id: metadata.dispatch_id.clone(),
            authorization_id: authorization.authorization_id.clone(),
            authorization_sha256: authorization.authorization_sha256.clone(),
            dispatch_request_id: request.dispatch_request_id.clone(),
            dispatch_request_sha256: request.dispatch_request_sha256.clone(),
            logical_request_sha256: request.request_sha256.clone(),
            request_body_sha256: request.provider_request_sha256.clone(),
            request_body_bytes: request.provider_request_bytes,
            pricing_snapshot_sha256: authorization.pricing_snapshot_sha256.clone(),
            node_id: authorization.node_id.clone(),
            attempt: authorization.attempt,
            max_cost_usd_micros: authorization.budgets.max_cost_usd_micros,
            consent_contract_version: authorization.release_requirements.consent_contract_version,
            lane_ownership_id: metadata.lane_ownership_id.clone(),
            project_lane_sha256: authorization.project_lane_sha256.clone(),
            released_at_ms: metadata.released_at_ms,
        },
    }
}

fn claim(
    control: &crate::runtime_domain::GroupAgentNodeDispatchReleaseControl,
    authorization: &GroupAgentNodeDispatchAuthorization,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
    claim_event_sha256: &str,
) -> GroupAgentNodeDispatchClaim {
    let request = &control.dispatch_request;
    GroupAgentNodeDispatchClaim {
        v: GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION,
        graph_run_id: control.graph_run.graph_run_id.clone(),
        dispatch_id: metadata.dispatch_id.clone(),
        authorization_id: authorization.authorization_id.clone(),
        authorization_sha256: authorization.authorization_sha256.clone(),
        dispatch_request_id: request.dispatch_request_id.clone(),
        dispatch_request_sha256: request.dispatch_request_sha256.clone(),
        logical_request_sha256: request.request_sha256.clone(),
        request_body_sha256: request.provider_request_sha256.clone(),
        request_body_bytes: request.provider_request_bytes,
        pricing_snapshot_sha256: authorization.pricing_snapshot_sha256.clone(),
        node_id: authorization.node_id.clone(),
        attempt: authorization.attempt,
        max_cost_usd_micros: authorization.budgets.max_cost_usd_micros,
        consent_contract_version: authorization.release_requirements.consent_contract_version,
        lane_ownership_id: metadata.lane_ownership_id.clone(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: 3,
        expected_last_event_sha256: authorization.expected_last_event_sha256.clone(),
        claim_event_sha256: claim_event_sha256.into(),
        released_at_ms: metadata.released_at_ms,
    }
}

fn active_lane(claim: &GroupAgentNodeDispatchClaim) -> GroupAgentNodeActiveLane {
    GroupAgentNodeActiveLane {
        v: GROUP_AGENT_NODE_ACTIVE_LANE_VERSION,
        project_lane_sha256: claim.project_lane_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        graph_run_id: claim.graph_run_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        claimed_at_ms: claim.released_at_ms,
    }
}

pub(super) fn build_artifact(
    claim: &GroupAgentNodeDispatchClaim,
    authorization: &GroupAgentNodeDispatchAuthorization,
    pricing: &GroupAgentNodePricingSnapshot,
    mut evidence: CollectedDispatchEvidence,
    created_at_ms: u64,
) -> Result<GroupAgentNodeTerminalArtifact, GroupAgentNodeDispatchExecutionServiceError> {
    let mut classification = evidence.classification;
    let priced_usage = evidence.usage.and_then(|usage| {
        pricing
            .actual_cost_usd_micros(usage.input_tokens, usage.output_tokens, authorization)
            .ok()
            .map(|cost| (usage, cost))
    });
    let invalid_usage = evidence.usage.is_some() && priced_usage.is_none();
    if invalid_usage {
        evidence.usage = None;
        classification = GroupAgentNodeTerminalClassification::LocalLimit;
    }
    let (usage, cost) = priced_usage
        .map(|(usage, cost)| (usage, Some(cost)))
        .unwrap_or_default();
    let result = matches!(
        classification,
        GroupAgentNodeTerminalClassification::Completed
            | GroupAgentNodeTerminalClassification::Length
    );
    let mut artifact = artifact_candidate(
        claim,
        evidence,
        classification,
        usage,
        cost,
        result,
        created_at_ms,
    );
    artifact.artifact_bytes = artifact
        .canonical_payload_json()
        .map_err(|_| invalid())?
        .len();
    artifact.artifact_sha256 = artifact.expected_sha256().map_err(|_| invalid())?;
    artifact.artifact_id = group_agent_node_terminal_artifact_id(&artifact.artifact_sha256);
    artifact.validate().map_err(|_| invalid())?;
    Ok(artifact)
}

#[allow(clippy::too_many_arguments)]
fn artifact_candidate(
    claim: &GroupAgentNodeDispatchClaim,
    evidence: CollectedDispatchEvidence,
    classification: GroupAgentNodeTerminalClassification,
    usage: crate::runtime_domain::Usage,
    cost: Option<u64>,
    result: bool,
    created_at_ms: u64,
) -> GroupAgentNodeTerminalArtifact {
    GroupAgentNodeTerminalArtifact {
        v: GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION,
        terminal_artifact_protocol_version: GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION,
        artifact_kind: if result {
            GroupAgentNodeTerminalArtifactKind::Result
        } else {
            GroupAgentNodeTerminalArtifactKind::Uncertainty
        },
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
        provider_poll_started: evidence.provider_poll_started,
        terminal_seen: evidence.terminal_seen,
        stream_eof_seen: evidence.stream_eof_seen,
        classification,
        output_bytes: evidence.output.len(),
        output_sha256: group_agent_node_terminal_output_sha256(&evidence.output),
        output_text: evidence.output,
        usage_observed: evidence.usage.is_some(),
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        actual_cost_calculated: cost.is_some(),
        actual_cost_usd_micros: cost.unwrap_or_default(),
        retry_authorized: false,
        created_at_ms,
        artifact_id: String::new(),
        artifact_bytes: 0,
        artifact_sha256: String::new(),
    }
}

pub(super) fn build_terminal_control(
    release: &crate::runtime_domain::GroupAgentNodeDispatchReleaseControl,
    authorization: GroupAgentNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
    lifecycle: &GroupAgentNodeLifecycleInspection,
    artifact: GroupAgentNodeTerminalArtifact,
) -> Result<GroupAgentNodeTerminalControl, GroupAgentNodeDispatchExecutionServiceError> {
    let mut control = GroupAgentNodeTerminalControl {
        v: GROUP_AGENT_NODE_TERMINAL_CONTROL_VERSION,
        scheduler_protocol_version: release.scheduler_protocol_version,
        terminal_control_protocol_version: GROUP_AGENT_NODE_TERMINAL_CONTROL_PROTOCOL_VERSION,
        graph_run: lifecycle.graph_run.run.clone(),
        plan: release.plan.clone(),
        manifest: release.manifest.clone(),
        journal_events: lifecycle.graph_run.events.clone(),
        contract_record: release.contract_record.clone(),
        contract: release.contract.clone(),
        dispatch_request: release.dispatch_request.clone(),
        provider_request_json: release.provider_request_json.clone(),
        authorization,
        pricing,
        active_lane: lifecycle.active_lane.clone().ok_or_else(invalid)?,
        claim: lifecycle.claim.clone(),
        artifact,
        snapshot_sha256: String::new(),
    };
    control.snapshot_sha256 = control.expected_sha256().map_err(|_| invalid())?;
    control.validate().map_err(|_| invalid())?;
    Ok(control)
}

pub(super) fn build_terminalize_request(
    control: GroupAgentNodeTerminalControl,
    envelope: GroupAgentNodeCoreTerminalReceiptEnvelope,
    terminalized_at_ms: u64,
) -> Result<TerminalizeGroupAgentNodeDispatch, GroupAgentNodeDispatchExecutionServiceError> {
    envelope
        .validate_against_control(&control)
        .map_err(|_| invalid())?;
    let receipt = envelope.receipt;
    let previous_event_sha256 = control.claim.claim_event_sha256.clone();
    let event = GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION,
        graph_run_id: control.graph_run.graph_run_id.clone(),
        seq: 5,
        kind: GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
            previous_event_sha256,
            dispatch_id: control.claim.dispatch_id.clone(),
            lane_ownership_id: control.claim.lane_ownership_id.clone(),
            project_lane_sha256: control.claim.project_lane_sha256.clone(),
            artifact_id: control.artifact.artifact_id.clone(),
            artifact_sha256: control.artifact.artifact_sha256.clone(),
            terminal_receipt_id: receipt.receipt_id.clone(),
            terminal_receipt_sha256: receipt.receipt_sha256.clone(),
            node_id: receipt.node_id.clone(),
            attempt: receipt.attempt,
            node_outcome: receipt.node_outcome,
            wave_index: receipt.wave_index,
            wave_outcome: receipt.wave_outcome,
            graph_status: receipt.graph_status,
            retry_authorized: false,
            lane_released: true,
            terminalized_at_ms,
        },
    };
    let request = TerminalizeGroupAgentNodeDispatch {
        v: GROUP_AGENT_NODE_LIFECYCLE_VERSION,
        control_json: control.canonical_json().map_err(|_| invalid())?,
        artifact_json: control.artifact.canonical_json().map_err(|_| invalid())?,
        receipt_json: envelope.receipt_json,
        event_json: event.canonical_json().map_err(|_| invalid())?,
        control,
        receipt,
        event,
        terminalized_at_ms,
    };
    request.validate().map_err(|_| invalid())?;
    Ok(request)
}

fn invalid() -> GroupAgentNodeDispatchExecutionServiceError {
    GroupAgentNodeDispatchExecutionServiceError::InvalidInput
}
