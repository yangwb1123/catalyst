use std::sync::Arc;

use forge_runtime_domain::{
    ClaimGroupAgentScheduledNodeDispatch, ClaimGroupAgentScheduledNodeDispatchResult,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GROUP_AGENT_NODE_PRICING_COST_ALGORITHM,
    GROUP_AGENT_NODE_PRICING_CURRENCY, GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
    GROUP_AGENT_NODE_PRICING_PROVENANCE, GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
    GROUP_AGENT_NODE_PRICING_TOKEN_UNIT, GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION, GroupAgentGraphExecutionScheduleInspection,
    GroupAgentGraphExecutionScheduleStore, GroupAgentGraphRunStore, GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeActiveLane, GroupAgentScheduledNodeContractStore,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeDispatchClaim,
    GroupAgentScheduledNodeDispatchClaimEvent, GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeLifecycleStore, GroupAgentScheduledNodeProviderRequestInspection,
    GroupAgentScheduledNodeProviderRequestStore, HubStoreError,
    group_agent_node_destination_sha256,
};
use forge_runtime_infrastructure::SqliteHubStore;

use super::{
    legacy_authorization_support,
    sqlite_group_agent_graph_execution_schedule_support as schedule_support,
    sqlite_group_agent_graph_run_support::Fixture as GraphFixture,
    sqlite_group_agent_scheduled_node_contract_support as contract_support,
    sqlite_group_agent_scheduled_node_provider_request_support as provider_support,
};

mod predecessor_terminal;
mod successor_content;

pub struct Fixture {
    graph: GraphFixture,
    pub reader: Arc<SqliteHubStore>,
    claim: ClaimGroupAgentScheduledNodeDispatch,
    pricing_json: String,
}

impl Fixture {
    pub fn new() -> Self {
        let graph = schedule_support::prepared_fixture();
        let schedule_request = schedule_support::request(&graph, "schedule-key", 40);
        let schedule = graph
            .store
            .admit_group_agent_graph_execution_schedule(&schedule_request)
            .expect("admit race schedule")
            .inspection;
        let mut admission = contract_support::admission(schedule_request, "contract-key", 50);
        let pricing = pricing_snapshot(&admission);
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
        let contract = graph
            .store
            .admit_group_agent_scheduled_node_contract(&admission)
            .expect("admit race contract")
            .inspection;
        let request = provider_support::request(&contract, "provider-key", 60);
        let provider = graph
            .store
            .prepare_group_agent_scheduled_node_provider_request(&request)
            .expect("prepare race provider request")
            .inspection;
        let release = release_control(&graph, &admission, schedule, &provider);
        let authorization = legacy_authorization_support::authorization(&release, &pricing);
        let pricing_json = pricing.canonical_json().expect("race pricing JSON");
        let claim = claim_request(release, authorization, pricing, &provider);
        let reader = Arc::new(
            SqliteHubStore::open_existing_current_live_read_only(&graph.database)
                .expect("open exact-current race reader"),
        );
        Self {
            graph,
            reader,
            claim,
            pricing_json,
        }
    }

    pub fn with_predecessor_content() -> Self {
        successor_content::fixture()
    }

    pub fn claim(&self) -> Result<ClaimGroupAgentScheduledNodeDispatchResult, HubStoreError> {
        self.graph
            .store
            .claim_group_agent_scheduled_node_dispatch(&self.claim)
    }

    pub fn writer(&self) -> Arc<SqliteHubStore> {
        Arc::new(self.graph.store.clone())
    }

    pub fn pricing_json(&self) -> &str {
        &self.pricing_json
    }

    pub fn provider_request_id(&self) -> &str {
        &self.claim.authorization.scheduled_provider_request_id
    }

    pub fn authorization_json(&self) -> &str {
        &self.claim.authorization_json
    }
}

fn pricing_snapshot(
    admission: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
) -> GroupAgentNodePricingSnapshot {
    let provider = &admission.candidate.provider;
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

fn release_control(
    graph: &GraphFixture,
    source: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    schedule: GroupAgentGraphExecutionScheduleInspection,
    provider: &GroupAgentScheduledNodeProviderRequestInspection,
) -> GroupAgentScheduledNodeDispatchReleaseControl {
    let run = graph
        .store
        .inspect_group_agent_graph_run(&source.graph_run_id)
        .expect("inspect race Graph Run");
    let mut value = GroupAgentScheduledNodeDispatchReleaseControl {
        v: GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        release_control_protocol_version:
            GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION,
        graph_run: run.run,
        journal_events: run.events,
        control_snapshot: source.control_snapshot.clone(),
        schedule_record: schedule.record,
        schedule: schedule.schedule,
        scheduled_contract_record: provider.scheduled_contract.record.clone(),
        scheduled_contract: provider.scheduled_contract.candidate.clone(),
        provider_request: provider.record.clone(),
        provider_request_json: String::from_utf8(provider.provider_request_body.clone())
            .expect("provider request UTF-8"),
        snapshot_sha256: String::new(),
    };
    value.snapshot_sha256 = value.expected_sha256().expect("release control digest");
    value.validate().expect("valid release control");
    value
}

fn claim_request(
    release: GroupAgentScheduledNodeDispatchReleaseControl,
    authorization: GroupAgentScheduledNodeDispatchAuthorization,
    pricing: GroupAgentNodePricingSnapshot,
    provider: &GroupAgentScheduledNodeProviderRequestInspection,
) -> ClaimGroupAgentScheduledNodeDispatch {
    let mut claim = dispatch_claim(&authorization);
    claim.claim_event_sha256 = claim_event(&claim)
        .expected_sha256()
        .expect("claim event digest");
    let event = claim_event(&claim);
    let lane = active_lane(&claim);
    let value = ClaimGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        release_control_json: release.canonical_json().expect("release control JSON"),
        authorization_json: authorization.canonical_json().expect("authorization JSON"),
        pricing_json: pricing.canonical_json().expect("pricing JSON"),
        provider_request: provider.record.clone(),
        provider_request_body: provider.provider_request_body.clone(),
        claim_json: claim.canonical_json().expect("claim JSON"),
        active_lane_json: lane.canonical_json().expect("active lane JSON"),
        claim_event_json: event.canonical_json().expect("claim event JSON"),
        release_control: release,
        authorization,
        pricing,
        claim,
        active_lane: lane,
        claim_event: event,
    };
    value.validate().expect("valid race claim request");
    value
}

fn dispatch_claim(
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
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
        lane_ownership_id: "ready-race-lane-owner".into(),
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
    let mut value = GroupAgentScheduledNodeDispatchClaimEvent {
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
    };
    value.event_sha256 = value.expected_sha256().expect("claim event digest");
    value
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
