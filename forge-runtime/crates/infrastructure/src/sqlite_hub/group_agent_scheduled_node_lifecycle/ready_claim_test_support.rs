use std::fmt::Write;

use crate::runtime_domain::*;
use crate::sqlite_hub::scheduled_graph_progress::atomicity_fixture;

mod encoding;

use crate::sqlite_hub::scheduled_graph_progress::{
    sqlite_group_agent_graph_execution_schedule_support as schedule_support,
    sqlite_group_agent_graph_run_support::{Fixture as GraphFixture, encode_graph_manifest},
    sqlite_group_agent_scheduled_node_contract_support as contract_support,
    sqlite_group_agent_scheduled_node_provider_request_support as provider_support,
};
use encoding::{
    active_lane, common_claim, legacy_claim, reseal_ready_claim, sealed_event, unsealed_event,
};

#[path = "../scheduled_graph_progress/atomicity_authorization.rs"]
#[allow(clippy::duplicate_mod)]
mod legacy_authorization_support;
#[path = "../../../../domain/src/group_agent_node_execution/scheduled_ready_dispatch_release_test_authorization.rs"]
#[allow(clippy::duplicate_mod)]
mod ready_authorization_support;

pub(super) struct FamilyFixture {
    pub fixture: atomicity_fixture::ReadyFixture,
    pub ready: ClaimGroupAgentScheduledReadyNodeDispatch,
    pub legacy: ClaimGroupAgentScheduledNodeDispatch,
}

pub(super) struct StaleFixture {
    pub graph: GraphFixture,
    pub ready: ClaimGroupAgentScheduledReadyNodeDispatch,
    pub progress_mutator: ClaimGroupAgentScheduledNodeDispatch,
}

pub(super) fn simple_ready_request(
    fixture: &atomicity_fixture::ReadyFixture,
    owner: &str,
    released_at_ms: u64,
) -> ClaimGroupAgentScheduledReadyNodeDispatch {
    let source = ready_source(&fixture.graph.store, "graph-run-1", 0);
    ready_request(
        &source,
        fixture.claim_request().pricing.clone(),
        owner,
        released_at_ms,
    )
}

pub(super) fn rebind_ephemeral_claim(
    request: &ClaimGroupAgentScheduledReadyNodeDispatch,
    owner: &str,
    released_at_ms: u64,
) -> ClaimGroupAgentScheduledReadyNodeDispatch {
    let mut changed = request.clone();
    changed.claim.lane_ownership_id = owner.into();
    changed.claim.released_at_ms = released_at_ms;
    reseal_ready_claim(&mut changed);
    changed.validate().expect("valid rebound ready claim");
    changed
}

pub(super) fn competing_family_fixture() -> FamilyFixture {
    let mut fixture = atomicity_fixture::ready_fixture();
    let ready = simple_ready_request(&fixture, "ready-family-owner", 100);
    let sibling = prepare_graph_sibling(&fixture.graph);
    fixture.graph.graph = sibling;
    fixture
        .graph
        .store
        .begin_group_agent_graph_run(&fixture.graph.request("graph-run-2", "run-key-2", 31))
        .expect("begin competing scheduled Graph Run");
    let (admission, provider, pricing) =
        prepare_initial_source(&fixture.graph, "graph-run-2", "family-two");
    let legacy = legacy_request(&fixture.graph, &admission, &provider, &pricing, 101);
    assert_ne!(
        ready.claim.provider_request_id,
        legacy.claim.provider_request_id
    );
    assert_eq!(
        ready.claim.project_lane_sha256,
        legacy.claim.project_lane_sha256
    );
    FamilyFixture {
        fixture,
        ready,
        legacy,
    }
}

fn prepare_graph_sibling(graph: &GraphFixture) -> GroupAgentGraphInspection {
    let manifest = graph.graph.manifest.clone();
    let (bytes, digest) = encode_graph_manifest(&manifest);
    let mut digest_hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut digest_hex, "{byte:02x}").expect("write sibling digest");
    }
    graph
        .store
        .prepare_group_agent_graph(&PrepareGroupAgentGraph {
            v: GROUP_AGENT_GRAPH_VERSION,
            graph_id: "graph-family-sibling".into(),
            manifest,
            manifest_json: String::from_utf8(bytes).expect("sibling manifest UTF-8"),
            manifest_sha256: digest_hex,
            idempotency_key: "graph-family-sibling-key".into(),
            created_at_ms: 21,
        })
        .expect("prepare competing sibling Graph")
        .inspection
}

pub(super) fn stale_progress_fixture() -> StaleFixture {
    let graph = GraphFixture::diamond();
    graph
        .store
        .begin_group_agent_graph_run(&graph.request("graph-run-1", "run-key", 30))
        .expect("begin diamond scheduled Graph Run");
    let schedule = schedule_support::request(&graph, "stale-schedule", 40);
    graph
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit diamond schedule");
    let (admission, pricing) = repriced_admission(schedule, "stale-contract", 50);
    let initial = graph
        .store
        .admit_group_agent_scheduled_node_contract(&admission)
        .expect("admit initial diamond contract")
        .inspection;
    let successor = provider_support::admit_backend_successor(&graph.store, &admission);
    let initial_provider = prepare_provider(&graph, &initial, "stale-provider-0", 60);
    let successor_provider = prepare_provider(&graph, &successor, "stale-provider-1", 61);
    let source = ready_source(&graph.store, "graph-run-1", 0);
    let ready = ready_request(&source, pricing.clone(), "stale-ready-owner", 100);
    let progress_mutator = legacy_request(&graph, &admission, &successor_provider, &pricing, 101);
    assert_eq!(
        ready.claim.provider_request_id,
        initial_provider.record.provider_request_id
    );
    assert_ne!(
        ready.claim.project_lane_sha256,
        progress_mutator.claim.project_lane_sha256
    );
    StaleFixture {
        graph,
        ready,
        progress_mutator,
    }
}

fn prepare_initial_source(
    graph: &GraphFixture,
    run_id: &str,
    key: &str,
) -> (
    AdmitGroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentNodePricingSnapshot,
) {
    let schedule = schedule_support::request_for_run(graph, run_id, &format!("{key}-schedule"), 40);
    graph
        .store
        .admit_group_agent_graph_execution_schedule(&schedule)
        .expect("admit competing schedule");
    let (admission, pricing) = repriced_admission(schedule, &format!("{key}-contract"), 50);
    let contract = graph
        .store
        .admit_group_agent_scheduled_node_contract(&admission)
        .expect("admit competing contract")
        .inspection;
    let provider = prepare_provider(graph, &contract, &format!("{key}-provider"), 60);
    (admission, provider, pricing)
}

fn repriced_admission(
    schedule: AdmitGroupAgentGraphExecutionSchedule,
    key: &str,
    admitted_at_ms: u64,
) -> (
    AdmitGroupAgentScheduledNodeContractCandidate,
    GroupAgentNodePricingSnapshot,
) {
    let mut admission = contract_support::admission(schedule, key, admitted_at_ms);
    let pricing = pricing_snapshot(&admission.candidate);
    admission
        .candidate
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&pricing.pricing_snapshot_sha256);
    provider_support::resign_candidate_digests(&mut admission.candidate);
    admission.candidate_json = admission
        .candidate
        .canonical_json()
        .expect("repriced candidate JSON");
    admission.validate().expect("valid repriced admission");
    (admission, pricing)
}

fn prepare_provider(
    graph: &GraphFixture,
    contract: &GroupAgentScheduledNodeContractInspection,
    key: &str,
    prepared_at_ms: u64,
) -> GroupAgentScheduledNodeProviderRequestInspection {
    let request = provider_support::request(contract, key, prepared_at_ms);
    graph
        .store
        .prepare_group_agent_scheduled_node_provider_request(&request)
        .expect("prepare scheduled provider request")
        .inspection
}

fn ready_source(
    store: &crate::sqlite_hub::SqliteHubStore,
    graph_run_id: &str,
    execution_ordinal: usize,
) -> ScheduledReadyNodeReleaseSource {
    let progress = store
        .snapshot_scheduled_graph_progress(graph_run_id)
        .expect("snapshot ready progress");
    let node = &progress.nodes[execution_ordinal];
    store
        .inspect_scheduled_ready_node_release(
            graph_run_id,
            &progress.snapshot_sha256,
            execution_ordinal,
            &node.node_id,
        )
        .expect("inspect ready source")
}

fn ready_request(
    source: &ScheduledReadyNodeReleaseSource,
    pricing: GroupAgentNodePricingSnapshot,
    owner: &str,
    released_at_ms: u64,
) -> ClaimGroupAgentScheduledReadyNodeDispatch {
    let release = ready_control(source);
    let authorization = ready_authorization_support::authorization(&release);
    let mut claim = common_claim(&authorization, owner, released_at_ms);
    claim.claim_event_sha256 = unsealed_event(&claim)
        .expected_sha256()
        .expect("ready claim event digest");
    let active_lane = active_lane(&claim);
    let claim_event = sealed_event(&claim);
    let request = ClaimGroupAgentScheduledReadyNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_LIFECYCLE_VERSION,
        release_control_json: release.canonical_json().expect("ready control JSON"),
        authorization_json: authorization
            .canonical_json()
            .expect("ready authorization JSON"),
        pricing_json: pricing.canonical_json().expect("ready pricing JSON"),
        provider_request: source.selected_provider_request.record.clone(),
        provider_request_body: source
            .selected_provider_request
            .provider_request_body
            .clone(),
        claim_json: claim.canonical_json().expect("ready claim JSON"),
        active_lane_json: active_lane.canonical_json().expect("ready lane JSON"),
        claim_event_json: claim_event.canonical_json().expect("ready event JSON"),
        release_control: release,
        authorization,
        pricing,
        claim,
        active_lane,
        claim_event,
    };
    request.validate().expect("valid ready claim request");
    request
}

fn ready_control(
    source: &ScheduledReadyNodeReleaseSource,
) -> GroupAgentScheduledReadyNodeDispatchReleaseControl {
    let selected =
        &source.progress_snapshot.nodes[source.selected_provider_request.record.execution_ordinal];
    GroupAgentScheduledReadyNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: source.graph_run.run.clone(),
        journal_events: source.graph_run.events.clone(),
        control_snapshot: control_snapshot(source),
        schedule_record: source.schedule.record.clone(),
        schedule: source.schedule.schedule.clone(),
        progress_snapshot: source.progress_snapshot.clone(),
        reconcile_decision: ready_decision(&source.progress_snapshot, selected),
        scheduled_contract_record: source
            .selected_provider_request
            .scheduled_contract
            .record
            .clone(),
        scheduled_contract: source
            .selected_provider_request
            .scheduled_contract
            .candidate
            .clone(),
        direct_predecessor_receipts: source.direct_predecessor_receipts.clone(),
        predecessor_content_artifact: source.predecessor_content_artifact.clone(),
        provider_request: source.selected_provider_request.record.clone(),
        provider_request_json: String::from_utf8(
            source
                .selected_provider_request
                .provider_request_body
                .clone(),
        )
        .expect("provider request UTF-8"),
        snapshot_sha256: String::new(),
    }
    .seal()
    .expect("seal ready control")
}

fn control_snapshot(source: &ScheduledReadyNodeReleaseSource) -> GroupAgentGraphControlSnapshot {
    let run = &source.graph_run;
    let mut snapshot = GroupAgentGraphControlSnapshot {
        v: GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION,
        scheduler_protocol_version: run.run.scheduler_protocol_version,
        graph_run_version: run.run.v,
        graph_run_id: run.run.graph_run_id.clone(),
        graph_id: run.run.graph_id.clone(),
        source_snapshot_sha256: run.run.source_snapshot_sha256.clone(),
        graph_manifest_sha256: run.run.graph_manifest_sha256.clone(),
        core_plan_sha256: run.run.plan_sha256.clone(),
        last_event_seq: run.run.last_event_seq,
        last_event_sha256: run
            .events
            .last()
            .expect("run event")
            .expected_sha256()
            .expect("event digest"),
        execution_contract_present: run.run.execution_contract_present,
        dispatch_authority_released: run.run.dispatch_authority_released,
        plan: run.plan.clone(),
        manifest: source.graph.manifest.clone(),
        snapshot_sha256: String::new(),
    };
    snapshot.snapshot_sha256 = snapshot.expected_sha256().expect("control digest");
    snapshot
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
    .expect("seal ready decision")
}

fn legacy_request(
    graph: &GraphFixture,
    admission: &AdmitGroupAgentScheduledNodeContractCandidate,
    provider: &GroupAgentScheduledNodeProviderRequestInspection,
    pricing: &GroupAgentNodePricingSnapshot,
    released_at_ms: u64,
) -> ClaimGroupAgentScheduledNodeDispatch {
    let run = graph
        .store
        .inspect_group_agent_graph_run(&provider.record.graph_run_id)
        .expect("inspect legacy release run");
    let schedule = graph
        .store
        .inspect_group_agent_graph_execution_schedule(&provider.record.schedule_id)
        .expect("inspect legacy release schedule");
    let mut release = GroupAgentScheduledNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: run.run,
        journal_events: run.events,
        control_snapshot: admission.control_snapshot.clone(),
        schedule_record: schedule.record,
        schedule: schedule.schedule,
        scheduled_contract_record: provider.scheduled_contract.record.clone(),
        scheduled_contract: provider.scheduled_contract.candidate.clone(),
        provider_request: provider.record.clone(),
        provider_request_json: String::from_utf8(provider.provider_request_body.clone())
            .expect("provider request UTF-8"),
        snapshot_sha256: String::new(),
    };
    release.snapshot_sha256 = release.expected_sha256().expect("legacy control digest");
    release.validate().expect("valid legacy control");
    let authorization = legacy_authorization_support::authorization(&release, pricing);
    legacy_claim_request(
        release,
        authorization,
        pricing.clone(),
        provider,
        released_at_ms,
    )
}

fn legacy_claim_request(
    release: GroupAgentScheduledNodeDispatchReleaseControl,
    authorization: GroupAgentScheduledNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
    provider: &GroupAgentScheduledNodeProviderRequestInspection,
    released_at_ms: u64,
) -> ClaimGroupAgentScheduledNodeDispatch {
    let mut claim = legacy_claim(&authorization, released_at_ms);
    claim.claim_event_sha256 = unsealed_event(&claim)
        .expected_sha256()
        .expect("claim event digest");
    let lane = active_lane(&claim);
    let event = sealed_event(&claim);
    let request = ClaimGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        release_control_json: release.canonical_json().expect("legacy control JSON"),
        authorization_json: authorization
            .canonical_json()
            .expect("legacy authorization JSON"),
        pricing_json: pricing.canonical_json().expect("legacy pricing JSON"),
        provider_request: provider.record.clone(),
        provider_request_body: provider.provider_request_body.clone(),
        claim_json: claim.canonical_json().expect("legacy claim JSON"),
        active_lane_json: lane.canonical_json().expect("legacy lane JSON"),
        claim_event_json: event.canonical_json().expect("legacy event JSON"),
        release_control: release,
        authorization,
        pricing,
        claim,
        active_lane: lane,
        claim_event: event,
    };
    request.validate().expect("valid legacy claim request");
    request
}

fn pricing_snapshot(
    candidate: &GroupAgentScheduledNodeContractCandidate,
) -> GroupAgentNodePricingSnapshot {
    let provider = &candidate.provider;
    let mut value = GroupAgentNodePricingSnapshot {
        v: GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
        pricing_protocol_version: GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
        provider_kind: provider.kind,
        endpoint: provider.endpoint.clone(),
        model: provider.model.clone(),
        destination_sha256: group_agent_node_destination_sha256(
            provider.kind,
            &provider.endpoint,
            &provider.model,
        ),
        currency: GROUP_AGENT_NODE_PRICING_CURRENCY.into(),
        token_unit: GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
        input_usd_micros_per_token_unit: 2_000_000,
        output_usd_micros_per_token_unit: 10_000_000,
        max_input_tokens: 400_000,
        cost_algorithm: GROUP_AGENT_NODE_PRICING_COST_ALGORITHM.into(),
        provenance: GROUP_AGENT_NODE_PRICING_PROVENANCE.into(),
        vendor_attestation_present: false,
        pricing_snapshot_sha256: String::new(),
    };
    value.pricing_snapshot_sha256 = value.expected_sha256().expect("pricing digest");
    value.validate().expect("valid pricing snapshot");
    value
}
