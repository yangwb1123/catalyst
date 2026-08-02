mod terminal;

pub use terminal::terminal_request;

use forge_runtime_domain::{
    ClaimGroupAgentNodeDispatch, GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GROUP_AGENT_NODE_ACTIVE_LANE_VERSION,
    GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION, GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION, GROUP_AGENT_NODE_LIFECYCLE_VERSION,
    GROUP_AGENT_NODE_PRICING_COST_ALGORITHM, GROUP_AGENT_NODE_PRICING_CURRENCY,
    GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION, GROUP_AGENT_NODE_PRICING_PROVENANCE,
    GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION, GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunStore,
    GroupAgentNodeActiveLane, GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchClaim,
    GroupAgentNodeDispatchConsentRequirement, GroupAgentNodeDispatchCredentialPreflight,
    GroupAgentNodeDispatchDestinationPreflight, GroupAgentNodeDispatchPricingPreflight,
    GroupAgentNodeDispatchProjectLaneClaim, GroupAgentNodeDispatchProviderHealthCheck,
    GroupAgentNodeDispatchReleaseControl, GroupAgentNodeDispatchReleaseRequirements,
    GroupAgentNodeDispatchRequestStore, GroupAgentNodeExecutionContractStore,
    GroupAgentNodePricingSnapshot, group_agent_node_dispatch_authorization_id,
};

use super::{
    sqlite_group_agent_graph_run_support::Fixture,
    sqlite_group_agent_node_dispatch_request_support,
    sqlite_group_agent_node_execution_contract_support,
};

pub struct ClaimFixture {
    pub fixture: Fixture,
    pub request: ClaimGroupAgentNodeDispatch,
    pub release: GroupAgentNodeDispatchReleaseControl,
    pub authorization: GroupAgentNodeDispatchAuthorization,
    pub pricing: GroupAgentNodePricingSnapshot,
}

struct PreparedClaim {
    request: ClaimGroupAgentNodeDispatch,
    release: GroupAgentNodeDispatchReleaseControl,
    authorization: GroupAgentNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
}

pub fn claim_fixture() -> ClaimFixture {
    claim_fixture_for(
        "graph-run-1",
        "run-key",
        "contract-key",
        "request-key",
        "dispatch-1",
        "lane-1",
    )
}

pub fn claim_fixture_for(
    graph_run_id: &str,
    run_key: &str,
    contract_key: &str,
    request_key: &str,
    dispatch_id: &str,
    lane_id: &str,
) -> ClaimFixture {
    let fixture = Fixture::single_node();
    let prepared = prepare_claim(
        &fixture,
        graph_run_id,
        run_key,
        contract_key,
        request_key,
        dispatch_id,
        lane_id,
    );
    ClaimFixture {
        fixture,
        request: prepared.request,
        release: prepared.release,
        authorization: prepared.authorization,
        pricing: prepared.pricing,
    }
}

pub fn prepare_claim_request(
    fixture: &Fixture,
    graph_run_id: &str,
    run_key: &str,
    contract_key: &str,
    request_key: &str,
    dispatch_id: &str,
    lane_id: &str,
) -> ClaimGroupAgentNodeDispatch {
    prepare_claim(
        fixture,
        graph_run_id,
        run_key,
        contract_key,
        request_key,
        dispatch_id,
        lane_id,
    )
    .request
}

fn prepare_claim(
    fixture: &Fixture,
    graph_run_id: &str,
    run_key: &str,
    contract_key: &str,
    request_key: &str,
    dispatch_id: &str,
    lane_id: &str,
) -> PreparedClaim {
    let (pricing, inspection) =
        prepare_dispatch_source(fixture, graph_run_id, run_key, contract_key, request_key);
    let release = release_control(fixture, &inspection);
    let authorization = authorization(&release);
    let request = claim_request(
        release.clone(),
        authorization.clone(),
        pricing.clone(),
        dispatch_id,
        lane_id,
    );
    PreparedClaim {
        request,
        release,
        authorization,
        pricing,
    }
}

fn prepare_dispatch_source(
    fixture: &Fixture,
    graph_run_id: &str,
    run_key: &str,
    contract_key: &str,
    request_key: &str,
) -> (
    GroupAgentNodePricingSnapshot,
    forge_runtime_domain::GroupAgentNodeDispatchRequestInspection,
) {
    fixture
        .store
        .begin_group_agent_graph_run(&fixture.request(graph_run_id, run_key, 30))
        .expect("begin single-node Graph Run");
    let mut admission = sqlite_group_agent_node_execution_contract_support::request_for_run(
        fixture,
        graph_run_id,
        contract_key,
        40,
    );
    let pricing = pricing(&admission.contract);
    admission
        .contract
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&pricing.pricing_snapshot_sha256);
    sqlite_group_agent_node_execution_contract_support::recanonicalize(&mut admission);
    let admitted = fixture
        .store
        .admit_group_agent_node_execution_contract(&admission)
        .expect("admit repriced contract");
    let dispatch = sqlite_group_agent_node_dispatch_request_support::request(
        fixture,
        &admitted.inspection.record.contract_id,
        request_key,
        50,
    );
    fixture
        .store
        .prepare_group_agent_node_dispatch_request(&dispatch)
        .expect("prepare dispatch request");
    let inspection = fixture
        .store
        .inspect_group_agent_node_dispatch_request(&dispatch.dispatch_request_id)
        .expect("inspect dispatch request");
    (pricing, inspection)
}

fn release_control(
    fixture: &Fixture,
    request: &forge_runtime_domain::GroupAgentNodeDispatchRequestInspection,
) -> GroupAgentNodeDispatchReleaseControl {
    let mut control = GroupAgentNodeDispatchReleaseControl {
        v: GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: request.contract.graph_run.run.clone(),
        plan: request.contract.graph_run.plan.clone(),
        manifest: fixture.graph.manifest.clone(),
        journal_events: request.contract.graph_run.events.clone(),
        contract_record: request.contract.record.clone(),
        contract: request.contract.contract.clone(),
        dispatch_request: request.record.clone(),
        provider_request_json: String::from_utf8(request.provider_request_body.clone())
            .expect("provider body UTF-8"),
        snapshot_sha256: String::new(),
    };
    control.snapshot_sha256 = control.expected_sha256().expect("release digest");
    control.validate().expect("valid release control");
    control
}

fn pricing(
    contract: &forge_runtime_domain::GroupAgentNodeExecutionContract,
) -> GroupAgentNodePricingSnapshot {
    let mut pricing = GroupAgentNodePricingSnapshot {
        v: GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
        pricing_protocol_version: GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
        provider_kind: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        destination_sha256: forge_runtime_domain::group_agent_node_destination_sha256(
            contract.provider.kind,
            &contract.provider.endpoint,
            &contract.provider.model,
        ),
        currency: GROUP_AGENT_NODE_PRICING_CURRENCY.into(),
        token_unit: GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
        input_usd_micros_per_token_unit: 1_000,
        output_usd_micros_per_token_unit: 2_000,
        max_input_tokens: 1_000,
        cost_algorithm: GROUP_AGENT_NODE_PRICING_COST_ALGORITHM.into(),
        provenance: GROUP_AGENT_NODE_PRICING_PROVENANCE.into(),
        vendor_attestation_present: false,
        pricing_snapshot_sha256: String::new(),
    };
    pricing.pricing_snapshot_sha256 = pricing.expected_sha256().expect("pricing digest");
    pricing.validate().expect("valid pricing");
    pricing
}

fn authorization(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> GroupAgentNodeDispatchAuthorization {
    let mut value = authorization_candidate(control);
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_node_dispatch_authorization_id(&value.authorization_sha256);
    value
        .validate_against_release_control(control)
        .expect("valid authorization");
    value
}

fn authorization_candidate(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> GroupAgentNodeDispatchAuthorization {
    let contract = &control.contract;
    let request = &control.dispatch_request;
    GroupAgentNodeDispatchAuthorization {
        v: GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION,
        scheduler_protocol_version: control.scheduler_protocol_version,
        dispatch_authorization_protocol_version:
            GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION,
        graph_run_id: control.graph_run.graph_run_id.clone(),
        graph_id: control.graph_run.graph_id.clone(),
        group_run_id: control.manifest.source.group_run_id.clone(),
        source_snapshot_sha256: control.graph_run.source_snapshot_sha256.clone(),
        graph_manifest_sha256: control.graph_run.graph_manifest_sha256.clone(),
        core_plan_sha256: control.graph_run.plan_sha256.clone(),
        release_control_snapshot_sha256: control.snapshot_sha256.clone(),
        expected_last_event_seq: 3,
        expected_last_event_sha256: control.journal_events[2].expected_sha256().unwrap(),
        contract_id: contract.contract_id.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        dispatch_request_id: request.dispatch_request_id.clone(),
        dispatch_request_sha256: request.dispatch_request_sha256.clone(),
        logical_request_sha256: request.request_sha256.clone(),
        request_body_sha256: request.provider_request_sha256.clone(),
        request_body_bytes: request.provider_request_bytes,
        node_id: contract.node.node_id.clone(),
        attempt: contract.node.attempt,
        project_id: contract.node.project_id.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        same_project_policy: contract.node.same_project_policy,
        provider_kind: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        destination_sha256: request.destination_sha256.clone(),
        pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
        budgets: contract.budgets.clone(),
        release_requirements: requirements(),
        failure: contract.failure.clone(),
        execution_contract_present: true,
        dispatch_request_present: true,
        dispatch_authority_release_authorized: true,
        dispatch_authority_released: false,
        authorization_id: String::new(),
        authorization_sha256: String::new(),
    }
}

fn requirements() -> GroupAgentNodeDispatchReleaseRequirements {
    GroupAgentNodeDispatchReleaseRequirements {
        consent: GroupAgentNodeDispatchConsentRequirement::FreshOffMachine,
        consent_contract_version: GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
        credential_preflight: GroupAgentNodeDispatchCredentialPreflight::HeaderSafeEnvironment,
        destination_preflight:
            GroupAgentNodeDispatchDestinationPreflight::ExactRegisteredDestination,
        pricing_preflight: GroupAgentNodeDispatchPricingPreflight::ExactSnapshotWithinMaxCost,
        project_lane_claim: GroupAgentNodeDispatchProjectLaneClaim::GlobalExclusiveUntilTerminal,
        provider_health_check: GroupAgentNodeDispatchProviderHealthCheck::Forbidden,
    }
}

fn claim_request(
    release: GroupAgentNodeDispatchReleaseControl,
    authorization: GroupAgentNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
    dispatch_id: &str,
    lane_id: &str,
) -> ClaimGroupAgentNodeDispatch {
    pricing
        .verify_authorization(&authorization)
        .expect("authorized pricing");
    let max_cost_usd_micros = authorization.budgets.max_cost_usd_micros;
    let previous = release.journal_events[2].expected_sha256().unwrap();
    let event = claim_event(
        &release,
        &authorization,
        max_cost_usd_micros,
        dispatch_id,
        lane_id,
        previous,
    );
    let claim_head = event.expected_sha256().unwrap();
    let claim = claim(
        &release,
        &authorization,
        max_cost_usd_micros,
        dispatch_id,
        lane_id,
        claim_head,
    );
    let active_lane = active_lane(&claim);
    let request = ClaimGroupAgentNodeDispatch {
        v: GROUP_AGENT_NODE_LIFECYCLE_VERSION,
        release_control_json: release.canonical_json().unwrap(),
        authorization_json: authorization.canonical_json().unwrap(),
        pricing_json: pricing.canonical_json().unwrap(),
        claim_json: claim.canonical_json().unwrap(),
        active_lane_json: active_lane.canonical_json().unwrap(),
        event_json: event.canonical_json().unwrap(),
        release_control: release,
        authorization,
        pricing,
        claim,
        active_lane,
        event,
    };
    request.validate().expect("valid claim request");
    request
}

fn claim_event(
    release: &GroupAgentNodeDispatchReleaseControl,
    authorization: &GroupAgentNodeDispatchAuthorization,
    max_cost: u64,
    dispatch_id: &str,
    lane_id: &str,
    previous: String,
) -> GroupAgentGraphRunEvent {
    let request = &release.dispatch_request;
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION,
        graph_run_id: release.graph_run.graph_run_id.clone(),
        seq: 4,
        kind: GroupAgentGraphRunEventKind::NodeDispatchReleased {
            previous_event_sha256: previous,
            dispatch_id: dispatch_id.into(),
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
            max_cost_usd_micros: max_cost,
            consent_contract_version: GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
            lane_ownership_id: lane_id.into(),
            project_lane_sha256: authorization.project_lane_sha256.clone(),
            released_at_ms: 100,
        },
    }
}

fn claim(
    release: &GroupAgentNodeDispatchReleaseControl,
    authorization: &GroupAgentNodeDispatchAuthorization,
    max_cost: u64,
    dispatch_id: &str,
    lane_id: &str,
    claim_head: String,
) -> GroupAgentNodeDispatchClaim {
    let request = &release.dispatch_request;
    GroupAgentNodeDispatchClaim {
        v: GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION,
        graph_run_id: release.graph_run.graph_run_id.clone(),
        dispatch_id: dispatch_id.into(),
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
        max_cost_usd_micros: max_cost,
        consent_contract_version: GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION,
        lane_ownership_id: lane_id.into(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: 3,
        expected_last_event_sha256: release.journal_events[2].expected_sha256().unwrap(),
        claim_event_sha256: claim_head,
        released_at_ms: 100,
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
