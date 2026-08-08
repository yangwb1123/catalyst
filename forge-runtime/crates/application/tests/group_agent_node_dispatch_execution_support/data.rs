use std::sync::Arc;

use forge_runtime_application::{
    AdmitGroupAgentNodeExecutionContractInput, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchReleaseControl, GroupAgentNodeDispatchReleaseControlService,
    GroupAgentNodeDispatchRequestCodec, GroupAgentNodeDispatchRequestService,
    GroupAgentNodeExecutionContract, GroupAgentNodeExecutionContractService,
    PrepareGroupAgentNodeDispatchRequestInput,
};
use forge_runtime_domain::{
    GROUP_AGENT_NODE_PRICING_COST_ALGORITHM, GROUP_AGENT_NODE_PRICING_CURRENCY,
    GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION, GROUP_AGENT_NODE_PRICING_PROVENANCE,
    GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION, GROUP_AGENT_NODE_PRICING_TOKEN_UNIT,
    GroupAgentNodePricingSnapshot, Message, ModelRequest, ProviderError,
    group_agent_node_destination_sha256, group_agent_node_dispatch_authorization_id,
};
use serde::Serialize;

use crate::group_agent_node_execution_support::{
    FixtureBundle, MemoryContractHub, single_node_fixture,
};

pub(crate) struct PreparedExecution {
    pub(crate) fixture: FixtureBundle,
    pub(crate) hub: Arc<MemoryContractHub>,
    pub(crate) codec: Arc<ExactJsonCodec>,
    pub(crate) authorization_json: String,
    pub(crate) pricing_json: String,
}
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

pub(crate) struct ExactJsonCodec;

impl GroupAgentNodeDispatchRequestCodec for ExactJsonCodec {
    fn encode_request(
        &self,
        model: &str,
        request: &ModelRequest,
    ) -> Result<Vec<u8>, ProviderError> {
        let Message::User { text } = &request.messages[0] else {
            return Err(codec_error());
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
        .map_err(|_| codec_error())
    }

    fn validate_exact_request(
        &self,
        model: &str,
        expected: &ModelRequest,
        actual: &[u8],
    ) -> Result<(), ProviderError> {
        (self.encode_request(model, expected)? == actual)
            .then_some(())
            .ok_or_else(codec_error)
    }
}

pub(crate) fn prepare() -> PreparedExecution {
    prepare_with_result_limit(None)
}

pub(super) fn prepare_with_max_result_bytes(max_result_bytes: usize) -> PreparedExecution {
    prepare_with_result_limit(Some(max_result_bytes))
}

fn prepare_with_result_limit(max_result_bytes: Option<usize>) -> PreparedExecution {
    let (fixture, pricing) = priced_fixture(max_result_bytes);
    let hub = Arc::new(MemoryContractHub::new(&fixture));
    let codec = Arc::new(ExactJsonCodec);
    prepare_dispatch(&fixture, &hub, &codec);
    let release =
        GroupAgentNodeDispatchReleaseControlService::new(hub.clone(), hub.clone(), codec.clone());
    let control = release
        .export(&fixture.run.run.graph_run_id)
        .expect("release control")
        .release_control;
    let authorization = authorization(&control);
    let authorization_json = authorization.canonical_json().expect("authorization JSON");
    let pricing_json = pricing.canonical_json().expect("pricing JSON");
    PreparedExecution {
        fixture,
        hub,
        codec,
        authorization_json,
        pricing_json,
    }
}

fn prepare_dispatch(
    fixture: &FixtureBundle,
    hub: &Arc<MemoryContractHub>,
    codec: &Arc<ExactJsonCodec>,
) {
    GroupAgentNodeExecutionContractService::new(hub.clone(), hub.clone(), hub.clone())
        .admit(&AdmitGroupAgentNodeExecutionContractInput {
            graph_run_id: fixture.run.run.graph_run_id.clone(),
            contract_json: fixture.contract_json.clone(),
            idempotency_key: "contract-key".into(),
            admitted_at_ms: 80,
        })
        .expect("admit contract");
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
}

fn priced_fixture(
    max_result_bytes: Option<usize>,
) -> (FixtureBundle, GroupAgentNodePricingSnapshot) {
    let mut fixture = single_node_fixture();
    let mut contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&fixture.contract_json).expect("contract");
    let pricing = pricing_snapshot(&contract);
    contract
        .budgets
        .pricing_snapshot_sha256
        .clone_from(&pricing.pricing_snapshot_sha256);
    contract.budgets.max_cost_usd_micros = 1_000_000;
    if let Some(max_result_bytes) = max_result_bytes {
        contract.result.max_result_bytes = max_result_bytes;
    }
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
    pricing.pricing_snapshot_sha256 = pricing.expected_sha256().expect("pricing digest");
    pricing.validate().expect("pricing snapshot");
    pricing
}

fn authorization(
    control: &GroupAgentNodeDispatchReleaseControl,
) -> GroupAgentNodeDispatchAuthorization {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-dispatch-authorization-v1.json"
    )))
    .expect("authorization fixture");
    let json = fixture["canonical_authorization_json"]
        .as_str()
        .expect("authorization fixture JSON");
    let source = serde_json::from_str(json).expect("authorization fixture value");
    rebuild_authorization(source, control)
}

fn rebuild_authorization(
    mut value: GroupAgentNodeDispatchAuthorization,
    control: &GroupAgentNodeDispatchReleaseControl,
) -> GroupAgentNodeDispatchAuthorization {
    bind_source(&mut value, control);
    bind_request(&mut value, control);
    bind_destination(&mut value, control);
    value.authorization_sha256 = value.expected_sha256().expect("authorization digest");
    value.authorization_id =
        group_agent_node_dispatch_authorization_id(&value.authorization_sha256);
    value
        .validate_against_release_control(control)
        .expect("valid authorization");
    value
}

fn bind_source(
    value: &mut GroupAgentNodeDispatchAuthorization,
    control: &GroupAgentNodeDispatchReleaseControl,
) {
    value
        .graph_run_id
        .clone_from(&control.graph_run.graph_run_id);
    value.graph_id.clone_from(&control.graph_run.graph_id);
    value
        .group_run_id
        .clone_from(&control.manifest.source.group_run_id);
    value
        .source_snapshot_sha256
        .clone_from(&control.graph_run.source_snapshot_sha256);
    value
        .graph_manifest_sha256
        .clone_from(&control.graph_run.graph_manifest_sha256);
    value
        .core_plan_sha256
        .clone_from(&control.graph_run.plan_sha256);
    value
        .release_control_snapshot_sha256
        .clone_from(&control.snapshot_sha256);
    value.expected_last_event_seq = 3;
    value.expected_last_event_sha256 = control.journal_events[2]
        .expected_sha256()
        .expect("seq-3 digest");
}

fn bind_request(
    value: &mut GroupAgentNodeDispatchAuthorization,
    control: &GroupAgentNodeDispatchReleaseControl,
) {
    let dispatch = &control.dispatch_request;
    let contract = &control.contract;
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
    value.attempt = contract.node.attempt;
    value.project_id.clone_from(&contract.node.project_id);
    value
        .project_lane_sha256
        .clone_from(&contract.node.project_lane_sha256);
    value.same_project_policy = contract.node.same_project_policy;
}

fn bind_destination(
    value: &mut GroupAgentNodeDispatchAuthorization,
    control: &GroupAgentNodeDispatchReleaseControl,
) {
    let dispatch = &control.dispatch_request;
    let contract = &control.contract;
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
}

fn codec_error() -> ProviderError {
    ProviderError::new("test_codec", "request bytes disagree", false)
}
