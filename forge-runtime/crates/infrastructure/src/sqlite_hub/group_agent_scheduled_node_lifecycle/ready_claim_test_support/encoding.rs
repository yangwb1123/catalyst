use crate::runtime_domain::*;

pub(super) fn common_claim(
    authorization: &GroupAgentScheduledReadyNodeDispatchAuthorization,
    owner: &str,
    released_at_ms: u64,
) -> GroupAgentScheduledNodeDispatchClaim {
    GroupAgentScheduledNodeDispatchClaim {
        v: GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
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
        lane_ownership_id: owner.into(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: authorization.expected_last_event_seq,
        expected_last_event_sha256: authorization.expected_last_event_sha256.clone(),
        claim_event_sha256: String::new(),
        released_at_ms,
    }
}

pub(super) fn legacy_claim(
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
    released_at_ms: u64,
) -> GroupAgentScheduledNodeDispatchClaim {
    GroupAgentScheduledNodeDispatchClaim {
        v: GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
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
        lane_ownership_id: "legacy-family-owner".into(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: authorization.expected_last_event_seq,
        expected_last_event_sha256: authorization.expected_last_event_sha256.clone(),
        claim_event_sha256: String::new(),
        released_at_ms,
    }
}

pub(super) fn reseal_ready_claim(request: &mut ClaimGroupAgentScheduledReadyNodeDispatch) {
    request.claim.claim_event_sha256 = String::new();
    request.claim.claim_event_sha256 = unsealed_event(&request.claim)
        .expected_sha256()
        .expect("ready claim event digest");
    request.active_lane = active_lane(&request.claim);
    request.claim_event = sealed_event(&request.claim);
    request.claim_json = request.claim.canonical_json().expect("ready claim JSON");
    request.active_lane_json = request
        .active_lane
        .canonical_json()
        .expect("ready lane JSON");
    request.claim_event_json = request
        .claim_event
        .canonical_json()
        .expect("ready event JSON");
}

pub(super) fn active_lane(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeActiveLane {
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

pub(super) fn sealed_event(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeDispatchClaimEvent {
    let mut event = unsealed_event(claim);
    event.event_sha256 = event.expected_sha256().expect("event digest");
    event
}

pub(super) fn unsealed_event(
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
