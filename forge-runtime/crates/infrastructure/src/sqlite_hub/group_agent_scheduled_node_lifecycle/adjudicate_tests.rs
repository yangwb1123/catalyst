use crate::runtime_domain::*;
use crate::sqlite_hub::scheduled_graph_progress::atomicity_fixture;

#[path = "../../../../domain/src/group_agent_node_execution/scheduled_ready_dispatch_release_test_authorization.rs"]
mod ready_authorization_support;

#[test]
fn wrong_owner_preserves_claimed_legacy_lifecycle() {
    let fixture = atomicity_fixture::ready_fixture();
    let provider_id = fixture.claim_request().claim.provider_request_id.clone();
    fixture.claim().expect("claim legacy lifecycle");

    let error = fixture
        .graph
        .store
        .adjudicate_group_agent_scheduled_node_any_dispatch(&adjudication(
            &provider_id,
            "wrong-owner",
        ))
        .expect_err("wrong owner must fail closed");
    assert!(matches!(error, HubStoreError::Conflict { .. }));

    let stored = fixture
        .graph
        .store
        .inspect_group_agent_scheduled_node_any_lifecycle(&provider_id)
        .expect("inspect preserved lifecycle");
    assert_eq!(
        stored.status(),
        GroupAgentScheduledNodeLifecycleStatus::Claimed
    );
    assert_eq!(stored.claim().lane_ownership_id, "atomicity-lane-owner");
}

#[test]
fn exact_owner_adjudicates_legacy_lifecycle_once() {
    let fixture = atomicity_fixture::ready_fixture();
    let provider_id = fixture.claim_request().claim.provider_request_id.clone();
    fixture.claim().expect("claim legacy lifecycle");
    let request = adjudication(&provider_id, "atomicity-lane-owner");

    let stored = fixture
        .graph
        .store
        .adjudicate_group_agent_scheduled_node_dispatch(&request)
        .expect("adjudicate exact legacy owner");
    assert_eq!(
        stored.status,
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated
    );
    assert!(matches!(
        fixture
            .graph
            .store
            .adjudicate_group_agent_scheduled_node_any_dispatch(&request),
        Err(HubStoreError::Conflict { .. })
    ));
}

#[test]
fn exact_owner_adjudicates_ready_v2_without_family_fallback() {
    let fixture = atomicity_fixture::ready_fixture();
    let request = ready_claim(&fixture);
    let provider_id = request.claim.provider_request_id.clone();
    let owner = request.claim.lane_ownership_id.clone();
    assert!(matches!(
        fixture
            .graph
            .store
            .claim_group_agent_scheduled_ready_node_dispatch(&request)
            .expect("claim ready v2 lifecycle"),
        ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { .. }
    ));
    let adjudication = adjudication(&provider_id, &owner);
    assert!(matches!(
        fixture
            .graph
            .store
            .adjudicate_group_agent_scheduled_node_dispatch(&adjudication),
        Err(HubStoreError::Conflict { .. })
    ));

    let stored = fixture
        .graph
        .store
        .adjudicate_group_agent_scheduled_node_any_dispatch(&adjudication)
        .expect("adjudicate exact ready owner");
    let GroupAgentScheduledNodeAnyLifecycleInspection::Ready(stored) = stored else {
        panic!("ready lifecycle must not fall back to legacy");
    };
    assert_eq!(stored.v, GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION);
    assert_eq!(
        stored.status,
        GroupAgentScheduledNodeLifecycleStatus::Adjudicated
    );
    assert!(stored.active_lane.is_none());
    assert_eq!(stored.adjudicated_at_ms, Some(200));
}

#[test]
fn mixed_ready_release_and_authorization_versions_fail_closed() {
    let fixture = atomicity_fixture::ready_fixture();
    let request = ready_claim(&fixture);
    let provider_id = request.claim.provider_request_id.clone();
    fixture
        .graph
        .store
        .claim_group_agent_scheduled_ready_node_dispatch(&request)
        .expect("claim ready lifecycle");
    mutate_stored_versions(&fixture, false);

    assert!(matches!(
        fixture
            .graph
            .store
            .inspect_group_agent_scheduled_node_any_lifecycle(&provider_id),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn unknown_ready_family_version_never_falls_back() {
    let fixture = atomicity_fixture::ready_fixture();
    let request = ready_claim(&fixture);
    let provider_id = request.claim.provider_request_id.clone();
    fixture
        .graph
        .store
        .claim_group_agent_scheduled_ready_node_dispatch(&request)
        .expect("claim ready lifecycle");
    mutate_stored_versions(&fixture, true);

    assert!(matches!(
        fixture
            .graph
            .store
            .inspect_group_agent_scheduled_node_any_lifecycle(&provider_id),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn legacy_terminalize_rejects_ready_envelope_without_mutation() {
    let fixture = atomicity_fixture::claimed_fixture();
    let mut request = fixture.terminal_request();
    request.v = GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION;
    let provider_id = request
        .control
        .as_ref()
        .expect("terminal control")
        .provider_request_id
        .clone();
    let before = fixture
        .graph
        .store
        .inspect_group_agent_scheduled_node_lifecycle(&provider_id)
        .expect("inspect legacy claim");

    assert!(matches!(
        fixture
            .graph
            .store
            .terminalize_group_agent_scheduled_node_dispatch(&request),
        Err(HubStoreError::Corrupt { .. })
    ));
    let after = fixture
        .graph
        .store
        .inspect_group_agent_scheduled_node_lifecycle(&provider_id)
        .expect("inspect preserved legacy claim");
    assert_eq!(after, before);
}

#[test]
fn ready_terminalize_rejects_legacy_envelope_without_mutation() {
    let fixture = atomicity_fixture::ready_fixture();
    let claim_request = ready_claim(&fixture);
    let result = fixture
        .graph
        .store
        .claim_group_agent_scheduled_ready_node_dispatch(&claim_request)
        .expect("claim ready lifecycle");
    let ClaimGroupAgentScheduledReadyNodeDispatchResult::Claimed { authority } = result else {
        panic!("first ready claim must return authority");
    };
    let (claim, _) = authority.into_parts();
    let artifact =
        crate::sqlite_hub::scheduled_graph_progress::atomicity_terminal::completed_artifact(&claim);
    let request = TerminalizeGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        control: None,
        control_json: None,
        artifact_json: artifact.canonical_json().expect("artifact JSON"),
        receipt: None,
        receipt_json: None,
        terminalized_at_ms: 200,
    };
    assert_ready_wrong_terminal_preserves(&fixture, &claim.provider_request_id, &request);
}

fn assert_ready_wrong_terminal_preserves(
    fixture: &atomicity_fixture::ReadyFixture,
    provider_id: &str,
    request: &TerminalizeGroupAgentScheduledNodeDispatch,
) {
    let before = fixture
        .graph
        .store
        .inspect_group_agent_scheduled_ready_node_lifecycle(provider_id)
        .expect("inspect ready claim");
    assert!(matches!(
        fixture
            .graph
            .store
            .terminalize_group_agent_scheduled_ready_node_dispatch(request),
        Err(HubStoreError::Corrupt { .. })
    ));
    let after = fixture
        .graph
        .store
        .inspect_group_agent_scheduled_ready_node_lifecycle(provider_id)
        .expect("inspect preserved ready claim");
    assert_eq!(after, before);
}

fn mutate_stored_versions(fixture: &atomicity_fixture::ReadyFixture, mutate_release_too: bool) {
    let release = if mutate_release_too {
        "release_control_json=CAST(replace(CAST(release_control_json AS TEXT),'\"v\":2','\"v\":3') AS BLOB),"
    } else {
        ""
    };
    fixture
        .graph
        .connection()
        .execute_batch(&format!(
            "UPDATE group_agent_graph_scheduled_node_dispatch_lifecycles SET {release}\
             authorization_json=CAST(replace(CAST(authorization_json AS TEXT),'\"v\":2',\
             '\"v\":{}') AS BLOB)",
            if mutate_release_too { 3 } else { 1 }
        ))
        .expect("mutate stored lifecycle family versions");
}

fn adjudication(
    provider_request_id: &str,
    expected_lane_ownership_id: &str,
) -> AdjudicateGroupAgentScheduledNodeDispatch {
    AdjudicateGroupAgentScheduledNodeDispatch {
        v: 1,
        provider_request_id: provider_request_id.into(),
        expected_lane_ownership_id: expected_lane_ownership_id.into(),
        adjudicated_at_ms: 200,
    }
}

fn ready_claim(
    fixture: &atomicity_fixture::ReadyFixture,
) -> ClaimGroupAgentScheduledReadyNodeDispatch {
    let source = ready_source(fixture);
    let release = ready_control(fixture, &source);
    let authorization = ready_authorization_support::authorization(&release);
    let pricing = fixture.claim_request().pricing.clone();
    let mut claim = common_claim(&authorization);
    claim.claim_event_sha256 = unsealed_event(&claim)
        .expected_sha256()
        .expect("ready claim event digest");
    let lane = active_lane(&claim);
    let event = sealed_event(&claim);
    let request = ClaimGroupAgentScheduledReadyNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
        release_control_json: release.canonical_json().expect("ready control JSON"),
        authorization_json: authorization.canonical_json().expect("ready auth JSON"),
        pricing_json: pricing.canonical_json().expect("ready pricing JSON"),
        provider_request: source.selected_provider_request.record.clone(),
        provider_request_body: source
            .selected_provider_request
            .provider_request_body
            .clone(),
        claim_json: claim.canonical_json().expect("ready claim JSON"),
        active_lane_json: lane.canonical_json().expect("ready lane JSON"),
        claim_event_json: event.canonical_json().expect("ready event JSON"),
        release_control: release,
        authorization,
        pricing,
        claim,
        active_lane: lane,
        claim_event: event,
    };
    request.validate().expect("valid ready claim");
    request
}

fn ready_source(fixture: &atomicity_fixture::ReadyFixture) -> ScheduledReadyNodeReleaseSource {
    let store = &fixture.graph.store;
    let progress = store
        .snapshot_scheduled_graph_progress("graph-run-1")
        .expect("ready progress");
    let selected = progress
        .nodes
        .iter()
        .find(|node| node.candidate_id.is_some() && node.lifecycle_status.is_none())
        .expect("selected ready node");
    store
        .inspect_scheduled_ready_node_release(
            "graph-run-1",
            &progress.snapshot_sha256,
            selected.execution_ordinal,
            &selected.node_id,
        )
        .expect("ready source")
}

fn ready_control(
    fixture: &atomicity_fixture::ReadyFixture,
    source: &ScheduledReadyNodeReleaseSource,
) -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let selected = source
        .progress_snapshot
        .nodes
        .iter()
        .find(|node| node.candidate_id.is_some() && node.lifecycle_status.is_none())
        .expect("selected ready node");
    let decision = ready_decision(&source.progress_snapshot, selected);
    let provider = &source.selected_provider_request;
    GroupAgentScheduledReadyNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: source.graph_run.run.clone(),
        journal_events: source.graph_run.events.clone(),
        control_snapshot: fixture
            .claim_request()
            .release_control
            .control_snapshot
            .clone(),
        schedule_record: source.schedule.record.clone(),
        schedule: source.schedule.schedule.clone(),
        progress_snapshot: source.progress_snapshot.clone(),
        reconcile_decision: decision,
        scheduled_contract_record: provider.scheduled_contract.record.clone(),
        scheduled_contract: provider.scheduled_contract.candidate.clone(),
        direct_predecessor_receipts: source.direct_predecessor_receipts.clone(),
        predecessor_content_artifact: source.predecessor_content_artifact.clone(),
        provider_request: provider.record.clone(),
        provider_request_json: String::from_utf8(provider.provider_request_body.clone())
            .expect("provider request UTF-8"),
        snapshot_sha256: String::new(),
    }
    .seal()
    .expect("sealed ready control")
}

fn ready_decision(
    progress: &ScheduledGraphProgressSnapshot,
    selected: &ScheduledGraphProgressNode,
) -> ScheduledGraphReconcileDecision {
    ScheduledGraphReconcileDecision {
        v: SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION,
        progress_protocol_version: SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION,
        graph_run_id: progress.graph_run_id.clone(),
        schedule_id: progress.schedule_id.clone(),
        schedule_sha256: progress.schedule_sha256.clone(),
        snapshot_sha256: progress.snapshot_sha256.clone(),
        disposition: ScheduledGraphReconcileDisposition::Ready,
        next_execution_ordinal: Some(selected.execution_ordinal),
        next_node_id: Some(selected.node_id.clone()),
        decision_sha256: String::new(),
    }
    .seal()
    .expect("sealed ready decision")
}

fn common_claim(
    authorization: &GroupAgentScheduledReadyNodeDispatchAuthorization,
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
        lane_ownership_id: "ready-v2-lane-owner".into(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: authorization.expected_last_event_seq,
        expected_last_event_sha256: authorization.expected_last_event_sha256.clone(),
        claim_event_sha256: String::new(),
        released_at_ms: 100,
    }
}

fn active_lane(claim: &GroupAgentScheduledNodeDispatchClaim) -> GroupAgentScheduledNodeActiveLane {
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

fn sealed_event(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeDispatchClaimEvent {
    let mut value = unsealed_event(claim);
    value.event_sha256 = value.expected_sha256().expect("event digest");
    value
}

fn unsealed_event(
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
