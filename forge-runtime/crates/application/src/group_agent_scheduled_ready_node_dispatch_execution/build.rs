use crate::GroupAgentNodeDispatchClaimMetadata;
use crate::group_agent_scheduled_ready_node_dispatch_execution::{
    GroupAgentScheduledReadyNodeDispatchExecutionServiceError, Preflight,
};
use crate::runtime_domain::{
    ClaimGroupAgentScheduledReadyNodeDispatch, GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION, GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION, GroupAgentScheduledNodeActiveLane,
    GroupAgentScheduledNodeCoreTerminalReceiptEnvelope, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchClaimEvent, GroupAgentScheduledNodeTerminalArtifact,
    GroupAgentScheduledNodeTerminalControl, GroupAgentScheduledReadyNodeDispatchReleaseControl,
    TerminalizeGroupAgentScheduledNodeDispatch,
};

type Error = GroupAgentScheduledReadyNodeDispatchExecutionServiceError;

pub(super) fn claim_request(
    preflight: &Preflight,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
) -> Result<ClaimGroupAgentScheduledReadyNodeDispatch, Error> {
    let claim = claim(preflight, metadata)?;
    let lane = lane(&claim);
    let event = claim_event(&claim)?;
    let release = &preflight.authorized.release_control;
    let authorization = &preflight.authorized.authorization;
    let value = ClaimGroupAgentScheduledReadyNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
        release_control: release.clone(),
        release_control_json: release.canonical_json().map_err(|_| invalid())?,
        authorization: authorization.clone(),
        authorization_json: authorization.canonical_json().map_err(|_| invalid())?,
        pricing: preflight.pricing.clone(),
        pricing_json: preflight.pricing_json.clone(),
        provider_request: release.provider_request.clone(),
        provider_request_body: release.provider_request_json.as_bytes().to_vec(),
        claim_json: claim.canonical_json().map_err(|_| invalid())?,
        active_lane_json: lane.canonical_json().map_err(|_| invalid())?,
        claim_event_json: event.canonical_json().map_err(|_| invalid())?,
        claim,
        active_lane: lane,
        claim_event: event,
    };
    value.validate().map_err(|_| invalid())?;
    Ok(value)
}

fn claim(
    preflight: &Preflight,
    metadata: &GroupAgentNodeDispatchClaimMetadata,
) -> Result<GroupAgentScheduledNodeDispatchClaim, Error> {
    let value = &preflight.authorized.authorization;
    let mut claim = GroupAgentScheduledNodeDispatchClaim {
        v: GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
        graph_run_id: value.graph_run_id.clone(),
        provider_request_id: value.scheduled_provider_request_id.clone(),
        dispatch_id: format!("scheduled-node-dispatch-{}", value.authorization_sha256),
        authorization_id: value.authorization_id.clone(),
        authorization_sha256: value.authorization_sha256.clone(),
        provider_request_sha256: value.scheduled_provider_request_sha256.clone(),
        request_body_sha256: value.request_body_sha256.clone(),
        request_body_bytes: value.request_body_bytes,
        pricing_snapshot_sha256: value.pricing_snapshot_sha256.clone(),
        node_id: value.node_id.clone(),
        attempt: value.attempt,
        max_cost_usd_micros: value.budgets.max_cost_usd_micros,
        lane_ownership_id: metadata.lane_ownership_id.clone(),
        project_lane_sha256: value.project_lane_sha256.clone(),
        expected_last_event_seq: value.expected_last_event_seq,
        expected_last_event_sha256: value.expected_last_event_sha256.clone(),
        claim_event_sha256: String::new(),
        released_at_ms: metadata.released_at_ms,
    };
    let event = unsealed_claim_event(&claim);
    claim.claim_event_sha256 = event.expected_sha256().map_err(|_| invalid())?;
    claim.validate().map_err(|_| invalid())?;
    Ok(claim)
}

fn lane(claim: &GroupAgentScheduledNodeDispatchClaim) -> GroupAgentScheduledNodeActiveLane {
    GroupAgentScheduledNodeActiveLane {
        v: GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION,
        project_lane_sha256: claim.project_lane_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        graph_run_id: claim.graph_run_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        claimed_at_ms: claim.released_at_ms,
    }
}

fn claim_event(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> Result<GroupAgentScheduledNodeDispatchClaimEvent, Error> {
    let mut event = unsealed_claim_event(claim);
    event.event_sha256 = event.expected_sha256().map_err(|_| invalid())?;
    event.validate().map_err(|_| invalid())?;
    Ok(event)
}

fn unsealed_claim_event(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeDispatchClaimEvent {
    GroupAgentScheduledNodeDispatchClaimEvent {
        v: GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
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

pub(super) fn terminal_control(
    release: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    claim: &GroupAgentScheduledNodeDispatchClaim,
    artifact: GroupAgentScheduledNodeTerminalArtifact,
) -> Result<GroupAgentScheduledNodeTerminalControl, Error> {
    let mut value = GroupAgentScheduledNodeTerminalControl {
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
    value.snapshot_sha256 = value.expected_sha256().map_err(|_| quarantined())?;
    value.validate().map_err(|_| quarantined())?;
    Ok(value)
}

pub(super) fn terminal_request(
    control: GroupAgentScheduledNodeTerminalControl,
    artifact_json: String,
    receipt: GroupAgentScheduledNodeCoreTerminalReceiptEnvelope,
    terminalized_at_ms: u64,
) -> Result<TerminalizeGroupAgentScheduledNodeDispatch, Error> {
    let control_json = control.canonical_json().map_err(|_| quarantined())?;
    Ok(TerminalizeGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
        control: Some(control),
        control_json: Some(control_json),
        artifact_json,
        receipt: Some(receipt.receipt),
        receipt_json: Some(receipt.receipt_json),
        terminalized_at_ms,
    })
}

pub(super) fn quarantine_request(
    artifact_json: String,
    terminalized_at_ms: u64,
) -> TerminalizeGroupAgentScheduledNodeDispatch {
    TerminalizeGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
        control: None,
        control_json: None,
        artifact_json,
        receipt: None,
        receipt_json: None,
        terminalized_at_ms,
    }
}

fn invalid() -> Error {
    Error::InvalidInput
}

fn quarantined() -> Error {
    Error::PostClaimOutcomeUncertain
}
