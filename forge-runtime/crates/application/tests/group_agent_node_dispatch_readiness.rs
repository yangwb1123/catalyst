#![allow(dead_code)]

mod group_agent_node_execution_support;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{
    AdmitGroupAgentNodeExecutionContractInput, GroupAgentNodeDispatchReadinessService,
    GroupAgentNodeDispatchReleaseControlService, GroupAgentNodeDispatchRequestCodec,
    GroupAgentNodeDispatchRequestService, GroupAgentNodeExecutionContractService,
    PrepareGroupAgentNodeDispatchRequestInput,
};
use forge_runtime_domain::{
    GROUP_AGENT_NODE_PRICING_COST_ALGORITHM, GROUP_AGENT_NODE_PRICING_CURRENCY,
    GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION, GROUP_AGENT_NODE_PRICING_PROVENANCE,
    GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION, GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
    GroupAgentNodeDestinationRegistry, GroupAgentNodeDestinationRegistryError,
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeDispatchReleaseControl,
    GroupAgentNodeExecutionContract, GroupAgentNodePricingQuote, GroupAgentNodePricingSnapshot,
    Message, ModelRequest, ProviderError, group_agent_node_destination_sha256,
    group_agent_node_dispatch_authorization_id,
};
use forge_runtime_infrastructure::RegisteredGroupAgentNodeProviderFactory;
use serde::Serialize;

use group_agent_node_execution_support::{FixtureBundle, MemoryContractHub, fixture};

#[derive(Serialize)]
struct ProviderBody<'a> {
    include: [&'static str; 1],
    input: [ProviderInput<'a>; 1],
    instructions: &'a str,
    max_output_tokens: u32,
    model: &'a str,
    store: bool,
    stream: bool,
    tools: [&'static str; 0],
}

#[derive(Serialize)]
struct ProviderInput<'a> {
    content: &'a str,
    role: &'static str,
    r#type: &'static str,
}

struct ExactJsonCodec;

#[derive(Default)]
struct RejectingDestinationRegistry {
    calls: AtomicUsize,
}

impl GroupAgentNodeDestinationRegistry for RejectingDestinationRegistry {
    fn resolve(
        &self,
        _authorization: &GroupAgentNodeDispatchAuthorization,
        _pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(GroupAgentNodeDestinationRegistryError::Rejected)
    }
}

impl GroupAgentNodeDispatchRequestCodec for ExactJsonCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        let Message::User { text } = &request.messages[0] else {
            return Err(provider_error());
        };
        serde_json::to_vec(&ProviderBody {
            include: ["reasoning.encrypted_content"],
            input: [ProviderInput {
                content: text,
                role: "user",
                r#type: "message",
            }],
            instructions: &request.system_prompt,
            max_output_tokens: request.max_output_tokens,
            model,
            store: false,
            stream: true,
            tools: [],
        })
        .map_err(|_| provider_error())
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        (self.encode_request(model, expected)? == actual)
            .then_some(())
            .ok_or_else(provider_error)
    }
}

#[test]
fn readiness_revalidates_current_authorization_destination_pricing_and_budget() {
    let (fixture, pricing) = priced_fixture();
    let (release, readiness) = prepared_services(&fixture);
    let control = release.export(&fixture.run.run.graph_run_id).unwrap();
    let authorization = authorization_from_fixture(&control.canonical_json);
    let verified = readiness
        .verify(
            &fixture.run.run.graph_run_id,
            &authorization,
            &pricing.canonical_json().unwrap(),
        )
        .expect("effect-free readiness");
    assert_eq!(verified.quote.max_cost_usd_micros, 840_960);
    assert_eq!(
        verified.authorization.pricing_snapshot_sha256,
        pricing.pricing_snapshot_sha256
    );
}

#[test]
fn readiness_accepts_the_exact_quote_budget_and_rejects_one_micro_less() {
    for (budget, accepted) in [(840_960, true), (840_959, false)] {
        let (fixture, pricing) = priced_fixture_with_budget(budget);
        let (release, readiness) = prepared_services(&fixture);
        let control = release.export(&fixture.run.run.graph_run_id).unwrap();
        let authorization = authorization_from_fixture(&control.canonical_json);
        let result = readiness.verify(
            &fixture.run.run.graph_run_id,
            &authorization,
            &pricing.canonical_json().unwrap(),
        );
        assert_eq!(result.is_ok(), accepted, "budget {budget}");
    }
}

#[test]
fn readiness_must_call_the_injected_destination_registry() {
    let (fixture, pricing) = priced_fixture();
    let registry = Arc::new(RejectingDestinationRegistry::default());
    let (release, readiness) = prepared_services_with_registry(&fixture, registry.clone());
    let control = release.export(&fixture.run.run.graph_run_id).unwrap();
    let authorization = authorization_from_fixture(&control.canonical_json);
    let result = readiness.verify(
        &fixture.run.run.graph_run_id,
        &authorization,
        &pricing.canonical_json().unwrap(),
    );

    assert!(result.is_err());
    assert_eq!(registry.calls.load(Ordering::SeqCst), 1);
}

fn priced_fixture() -> (FixtureBundle, GroupAgentNodePricingSnapshot) {
    priced_fixture_with_budget(1_000_000)
}

fn priced_fixture_with_budget(
    max_cost_usd_micros: u64,
) -> (FixtureBundle, GroupAgentNodePricingSnapshot) {
    let mut fixture = fixture();
    let mut contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&fixture.contract_json).expect("contract");
    let pricing = pricing_snapshot(&contract);
    contract
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&pricing.pricing_snapshot_sha256);
    contract.budgets.max_cost_usd_micros = max_cost_usd_micros;
    let digest = contract.expected_sha256().expect("contract digest");
    contract.contract_id = format!("node-contract-{digest}");
    contract.contract_sha256 = digest;
    contract.validate().expect("repriced contract");
    fixture.contract_json = contract.canonical_json().expect("contract JSON");
    (fixture, pricing)
}

fn pricing_snapshot(contract: &GroupAgentNodeExecutionContract) -> GroupAgentNodePricingSnapshot {
    let mut pricing = GroupAgentNodePricingSnapshot {
        v: GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION,
        pricing_protocol_version: GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION,
        provider_kind: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        destination_sha256: group_agent_node_destination_sha256(
            contract.provider.kind,
            &contract.provider.endpoint,
            &contract.provider.model,
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
    pricing.pricing_snapshot_sha256 = pricing.expected_sha256().unwrap();
    pricing.validate().expect("pricing snapshot");
    pricing
}

fn prepared_services(
    fixture: &FixtureBundle,
) -> (
    GroupAgentNodeDispatchReleaseControlService,
    GroupAgentNodeDispatchReadinessService,
) {
    prepared_services_with_registry(
        fixture,
        Arc::new(RegisteredGroupAgentNodeProviderFactory::new()),
    )
}

fn prepared_services_with_registry(
    fixture: &FixtureBundle,
    destinations: Arc<dyn GroupAgentNodeDestinationRegistry>,
) -> (
    GroupAgentNodeDispatchReleaseControlService,
    GroupAgentNodeDispatchReadinessService,
) {
    let hub = Arc::new(MemoryContractHub::new(fixture));
    let codec = Arc::new(ExactJsonCodec);
    GroupAgentNodeExecutionContractService::new(hub.clone(), hub.clone(), hub.clone())
        .admit(&AdmitGroupAgentNodeExecutionContractInput {
            graph_run_id: fixture.run.run.graph_run_id.clone(),
            contract_json: fixture.contract_json.clone(),
            idempotency_key: "contract-key".into(),
            admitted_at_ms: 80,
        })
        .expect("admit");
    GroupAgentNodeDispatchRequestService::new(
        hub.clone(),
        hub.clone(),
        hub.clone(),
        hub.clone(),
        codec.clone(),
    )
    .prepare(&PrepareGroupAgentNodeDispatchRequestInput {
        graph_run_id: fixture.run.run.graph_run_id.clone(),
        idempotency_key: "dispatch-key".into(),
        prepared_at_ms: 90,
    })
    .expect("prepare dispatch");
    (
        GroupAgentNodeDispatchReleaseControlService::new(hub.clone(), hub.clone(), codec.clone()),
        GroupAgentNodeDispatchReadinessService::new(hub.clone(), hub, codec, destinations),
    )
}

fn authorization_from_fixture(control_json: &str) -> String {
    let control: GroupAgentNodeDispatchReleaseControl =
        serde_json::from_str(control_json).expect("release control");
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-dispatch-authorization-v1.json"
    )))
    .expect("release fixture");
    let source: GroupAgentNodeDispatchAuthorization =
        serde_json::from_str(fixture["canonical_authorization_json"].as_str().unwrap())
            .expect("authorization");
    rebuild_authorization(source, &control)
}

fn rebuild_authorization(
    mut value: GroupAgentNodeDispatchAuthorization,
    control: &GroupAgentNodeDispatchReleaseControl,
) -> String {
    let dispatch = &control.dispatch_request;
    let contract = &control.contract;
    value
        .release_control_snapshot_sha256
        .clone_from(&control.snapshot_sha256);
    value.expected_last_event_sha256 = control.journal_events[2].expected_sha256().unwrap();
    value.contract_id.clone_from(&contract.contract_id);
    value.contract_sha256.clone_from(&contract.contract_sha256);
    value
        .dispatch_request_id
        .clone_from(&dispatch.dispatch_request_id);
    value
        .dispatch_request_sha256
        .clone_from(&dispatch.dispatch_request_sha256);
    value
        .logical_request_sha256
        .clone_from(&dispatch.request_sha256);
    value
        .request_body_sha256
        .clone_from(&dispatch.provider_request_sha256);
    value.request_body_bytes = dispatch.provider_request_bytes;
    value.node_id.clone_from(&contract.node.node_id);
    value.project_id.clone_from(&contract.node.project_id);
    value
        .project_lane_sha256
        .clone_from(&contract.node.project_lane_sha256);
    value.provider_kind = contract.provider.kind;
    value.endpoint.clone_from(&contract.provider.endpoint);
    value.model.clone_from(&contract.provider.model);
    value
        .destination_sha256
        .clone_from(&dispatch.destination_sha256);
    value
        .pricing_snapshot_sha256
        .clone_from(&dispatch.pricing_snapshot_sha256);
    value.budgets.clone_from(&contract.budgets);
    value.failure.clone_from(&contract.failure);
    value.authorization_sha256 = value.expected_sha256().unwrap();
    value.authorization_id =
        group_agent_node_dispatch_authorization_id(&value.authorization_sha256);
    value.canonical_json().expect("authorization JSON")
}

fn provider_error() -> ProviderError {
    ProviderError::new("test_codec", "request bytes disagree", false)
}
