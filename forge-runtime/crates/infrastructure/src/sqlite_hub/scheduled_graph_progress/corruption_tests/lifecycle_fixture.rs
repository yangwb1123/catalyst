use crate::runtime_domain::{
    ClaimGroupAgentScheduledNodeDispatch, GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_PRICING_COST_ALGORITHM, GROUP_AGENT_NODE_PRICING_CURRENCY,
    GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION, GROUP_AGENT_NODE_PRICING_PROVENANCE,
    GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION, GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION, GroupAgentGraphExecutionScheduleStore,
    GroupAgentGraphRunStore, GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind,
    GroupAgentScheduledNodeActiveLane, GroupAgentScheduledNodeContractStore,
    GroupAgentScheduledNodeDispatchAtomicTransitionRequirement,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchClaimEvent, GroupAgentScheduledNodeDispatchConsentRequirement,
    GroupAgentScheduledNodeDispatchCredentialPreflight,
    GroupAgentScheduledNodeDispatchDestinationPreflight,
    GroupAgentScheduledNodeDispatchPricingPreflight,
    GroupAgentScheduledNodeDispatchProjectLaneClaim,
    GroupAgentScheduledNodeDispatchProviderHealthCheck,
    GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeDispatchReleaseRequirements, GroupAgentScheduledNodeLifecycleStore,
    GroupAgentScheduledNodeProviderRequestStore, GroupAgentScheduledNodeSuccessorRequirement,
    group_agent_node_destination_sha256, group_agent_scheduled_node_dispatch_authorization_id,
};
use serde_json::{Map, Value, json};

use super::super::{
    atomicity_terminal, sqlite_group_agent_graph_execution_schedule_support as schedule_support,
    sqlite_group_agent_graph_run_support::Fixture,
    sqlite_group_agent_scheduled_node_contract_support as contract_support,
    sqlite_group_agent_scheduled_node_provider_request_support as provider_support,
};

pub(super) fn claimed_fixture() -> Fixture {
    claimed_source().0
}

pub(super) fn terminalized_fixture() -> Fixture {
    let (fixture, release, claim) = claimed_source();
    let terminal = atomicity_terminal::terminal_request(&release, &claim);
    fixture
        .store
        .terminalize_group_agent_scheduled_node_dispatch(&terminal)
        .expect("terminalize scheduled dispatch");
    fixture
}

fn claimed_source() -> (
    Fixture,
    GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeDispatchClaim,
) {
    let (fixture, pricing, provider_id) = prepared_priced_fixture();
    let release = release_control(&fixture, &provider_id);
    let authorization = authorization(&release);
    let request = claim_request(release.clone(), authorization, pricing);
    let claim = request.claim.clone();
    fixture
        .store
        .claim_group_agent_scheduled_node_dispatch(&request)
        .expect("claim scheduled dispatch");
    (fixture, release, claim)
}

fn prepared_priced_fixture() -> (Fixture, GroupAgentNodePricingSnapshot, String) {
    let (fixture, mut admission) = contract_support::prepared_fixture();
    let pricing = pricing_snapshot(
        admission.candidate.provider.kind,
        &admission.candidate.provider.endpoint,
        &admission.candidate.provider.model,
    );
    admission
        .candidate
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&pricing.pricing_snapshot_sha256);
    provider_support::resign_candidate_digests(&mut admission.candidate);
    admission.candidate_json = admission
        .candidate
        .canonical_json()
        .expect("candidate JSON");
    admission.validate().expect("repriced candidate admission");
    let source = fixture
        .store
        .admit_group_agent_scheduled_node_contract(&admission)
        .expect("admit repriced candidate")
        .inspection;
    let request = provider_support::request(&source, "priced-provider-key", 60);
    let provider_id = request.provider_request_id.clone();
    fixture
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("prepare repriced provider request");
    (fixture, pricing, provider_id)
}

fn pricing_snapshot(
    provider: GroupAgentNodeProviderKind,
    endpoint: &str,
    model: &str,
) -> GroupAgentNodePricingSnapshot {
    let mut pricing = GroupAgentNodePricingSnapshot {
        v: GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
        pricing_protocol_version: GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
        provider_kind: provider,
        endpoint: endpoint.into(),
        model: model.into(),
        destination_sha256: group_agent_node_destination_sha256(provider, endpoint, model),
        currency: GROUP_AGENT_NODE_PRICING_CURRENCY.into(),
        token_unit: GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
        input_usd_micros_per_token_unit: 1,
        output_usd_micros_per_token_unit: 244_140_380,
        max_input_tokens: 1,
        cost_algorithm: GROUP_AGENT_NODE_PRICING_COST_ALGORITHM.into(),
        provenance: GROUP_AGENT_NODE_PRICING_PROVENANCE.into(),
        vendor_attestation_present: false,
        pricing_snapshot_sha256: String::new(),
    };
    pricing.pricing_snapshot_sha256 = pricing.expected_sha256().expect("pricing digest");
    pricing.validate().expect("valid pricing snapshot");
    pricing
}

fn release_control(
    fixture: &Fixture,
    provider_id: &str,
) -> GroupAgentScheduledNodeDispatchReleaseControl {
    let run = fixture
        .store
        .inspect_group_agent_graph_run("graph-run-1")
        .expect("inspect release Run");
    let source = schedule_support::request(fixture, "release-source", 40);
    let schedule = fixture
        .store
        .inspect_group_agent_graph_execution_schedule(&source.schedule.schedule_id)
        .expect("inspect release schedule");
    let provider = fixture
        .store
        .inspect_group_agent_scheduled_node_provider_request(provider_id)
        .expect("inspect release provider request");
    let provider_request_json =
        String::from_utf8(provider.provider_request_body).expect("provider request UTF-8");
    let mut release = GroupAgentScheduledNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: run.run,
        journal_events: run.events,
        control_snapshot: source.control_snapshot,
        schedule_record: schedule.record,
        schedule: schedule.schedule,
        scheduled_contract_record: provider.scheduled_contract.record,
        scheduled_contract: provider.scheduled_contract.candidate,
        provider_request: provider.record,
        provider_request_json,
        snapshot_sha256: String::new(),
    };
    release.snapshot_sha256 = release.expected_sha256().expect("release digest");
    release.validate().expect("valid release control");
    release
}

fn authorization(
    release: &GroupAgentScheduledNodeDispatchReleaseControl,
) -> GroupAgentScheduledNodeDispatchAuthorization {
    let mut object = Map::new();
    merge(&mut object, authorization_source(release));
    merge(&mut object, authorization_execution(release));
    merge(&mut object, authorization_state());
    let mut value: GroupAgentScheduledNodeDispatchAuthorization =
        serde_json::from_value(Value::Object(object)).expect("authorization fields");
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_scheduled_node_dispatch_authorization_id(&value.authorization_sha256);
    value
        .validate_against_release_control(release)
        .expect("authorization bound to release");
    value
}

fn authorization_source(release: &GroupAgentScheduledNodeDispatchReleaseControl) -> Value {
    let source = &release.control_snapshot;
    let request = &release.provider_request;
    json!({
        "v": GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_VERSION,
        "scheduler_protocol_version": source.scheduler_protocol_version,
        "dispatch_authorization_protocol_version": GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
        "graph_run_id": source.graph_run_id, "graph_id": source.graph_id,
        "group_run_id": source.manifest.source.group_run_id, "group_id": source.manifest.source.group_id,
        "source_snapshot_sha256": source.source_snapshot_sha256,
        "graph_manifest_sha256": source.graph_manifest_sha256, "core_plan_sha256": source.core_plan_sha256,
        "control_snapshot_sha256": source.snapshot_sha256,
        "release_control_snapshot_sha256": release.snapshot_sha256,
        "schedule_id": release.schedule.schedule_id, "schedule_sha256": release.schedule.schedule_sha256,
        "scheduled_contract_id": release.scheduled_contract.contract_id,
        "scheduled_contract_sha256": release.scheduled_contract.contract_sha256,
        "scheduled_provider_request_id": request.provider_request_id,
        "scheduled_provider_request_sha256": request.prepared_request_sha256,
        "logical_request_id": request.logical_request_id, "logical_request_sha256": request.logical_request_sha256,
        "request_body_sha256": request.provider_request_sha256, "request_body_bytes": request.provider_request_bytes,
        "expected_last_event_seq": source.last_event_seq, "expected_last_event_sha256": source.last_event_sha256,
        "authorization_id": "", "authorization_sha256": ""
    })
}

fn authorization_execution(release: &GroupAgentScheduledNodeDispatchReleaseControl) -> Value {
    let contract = &release.scheduled_contract;
    let request = &release.provider_request;
    json!({
        "execution_ordinal": contract.node.execution_ordinal, "node_id": contract.node.node_id,
        "attempt": contract.node.attempt, "project_id": contract.node.project_id,
        "project_lane_sha256": contract.node.project_lane_sha256,
        "same_project_policy": contract.node.same_project_policy,
        "provider_kind": contract.provider.kind, "endpoint": contract.provider.endpoint,
        "model": contract.provider.model, "destination_sha256": request.destination_sha256,
        "pricing_snapshot_sha256": request.pricing_snapshot_sha256,
        "budgets": contract.budgets, "failure": contract.failure,
        "release_requirements": release_requirements()
    })
}

fn authorization_state() -> Value {
    json!({
        "lifecycle_contract_admission_authorized": true,
        "execution_authority_release_authorized": true,
        "dispatch_authority_release_authorized": true,
        "scheduled_contract_candidate_present": true, "provider_request_prepared": true,
        "lifecycle_contract_admitted": false, "execution_authority_released": false,
        "dispatch_authority_released": false, "project_lane_claimed": false,
        "provider_request_sent": false, "progress_observed": false,
        "terminal_receipt_recorded": false, "successor_advance_authorized": false
    })
}

fn release_requirements() -> GroupAgentScheduledNodeDispatchReleaseRequirements {
    GroupAgentScheduledNodeDispatchReleaseRequirements {
        consent: GroupAgentScheduledNodeDispatchConsentRequirement::FreshOffMachine,
        consent_contract_version: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
        credential_preflight:
            GroupAgentScheduledNodeDispatchCredentialPreflight::HeaderSafeEnvironment,
        destination_preflight:
            GroupAgentScheduledNodeDispatchDestinationPreflight::ExactRegisteredDestination,
        pricing_preflight:
            GroupAgentScheduledNodeDispatchPricingPreflight::ExactSnapshotWithinMaxCost,
        project_lane_claim:
            GroupAgentScheduledNodeDispatchProjectLaneClaim::GlobalExclusiveUntilTerminal,
        provider_health_check: GroupAgentScheduledNodeDispatchProviderHealthCheck::Forbidden,
        atomic_transition: GroupAgentScheduledNodeDispatchAtomicTransitionRequirement::ExactPristineHeadAdmissionReleaseAndLaneClaim,
        successor: GroupAgentScheduledNodeSuccessorRequirement::VerifiedIntermediateTerminalReceiptBeforeSuccessor,
    }
}

fn merge(target: &mut Map<String, Value>, source: Value) {
    let Value::Object(source) = source else {
        panic!("authorization source must be an object");
    };
    target.extend(source);
}

fn claim_request(
    release: GroupAgentScheduledNodeDispatchReleaseControl,
    authorization: GroupAgentScheduledNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
) -> ClaimGroupAgentScheduledNodeDispatch {
    let mut claim = claim_candidate(&authorization);
    let mut event = claim_event(&claim);
    event.event_sha256 = event.expected_sha256().expect("claim event digest");
    claim.claim_event_sha256.clone_from(&event.event_sha256);
    claim.validate().expect("valid scheduled claim");
    let lane = active_lane(&claim);
    let request = ClaimGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        release_control_json: release.canonical_json().expect("release JSON"),
        authorization_json: authorization.canonical_json().expect("authorization JSON"),
        pricing_json: pricing.canonical_json().expect("pricing JSON"),
        provider_request: release.provider_request.clone(),
        provider_request_body: release.provider_request_json.as_bytes().to_vec(),
        claim_json: claim.canonical_json().expect("claim JSON"),
        active_lane_json: lane.canonical_json().expect("lane JSON"),
        claim_event_json: event.canonical_json().expect("claim event JSON"),
        release_control: release,
        authorization,
        pricing,
        claim,
        active_lane: lane,
        claim_event: event,
    };
    request.validate().expect("valid scheduled claim request");
    request
}

fn claim_candidate(
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
) -> GroupAgentScheduledNodeDispatchClaim {
    GroupAgentScheduledNodeDispatchClaim {
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
        lane_ownership_id: "scheduled-corruption-lane".into(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: authorization.expected_last_event_seq,
        expected_last_event_sha256: authorization.expected_last_event_sha256.clone(),
        claim_event_sha256: String::new(),
        released_at_ms: 70,
    }
}

fn claim_event(
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

fn active_lane(claim: &GroupAgentScheduledNodeDispatchClaim) -> GroupAgentScheduledNodeActiveLane {
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
        claimed_at_ms: claim.released_at_ms,
    }
}
