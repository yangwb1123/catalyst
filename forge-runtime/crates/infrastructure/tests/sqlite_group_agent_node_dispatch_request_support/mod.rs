use forge_runtime_domain::{
    Cancellation, GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
    GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION, GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentNodeExecutionContractStore,
    GroupAgentNodeProviderKind, Message, ModelRequest, PrepareGroupAgentNodeDispatchRequest,
    group_agent_node_destination_sha256, group_agent_node_dispatch_request_id,
    group_agent_node_provider_request_sha256,
};
use forge_runtime_infrastructure::OpenAiResponsesProvider;

use crate::{
    sqlite_group_agent_graph_run_support::Fixture,
    sqlite_group_agent_node_execution_contract_support::{
        prepared_fixture, request as contract_request,
    },
};

pub fn admitted_fixture() -> (Fixture, String) {
    let fixture = prepared_fixture();
    let admitted = fixture
        .store
        .admit_group_agent_node_execution_contract(&contract_request(&fixture, "contract-key", 40))
        .expect("admit Node Execution Contract");
    (fixture, admitted.inspection.record.contract_id)
}

pub fn request(
    fixture: &Fixture,
    contract_id: &str,
    key: &str,
    prepared_at_ms: u64,
) -> PrepareGroupAgentNodeDispatchRequest {
    let inspection = fixture
        .store
        .inspect_group_agent_node_execution_contract(contract_id)
        .expect("inspect admitted contract");
    let contract = &inspection.contract;
    let body = provider_body(contract);
    let mut request = PrepareGroupAgentNodeDispatchRequest {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        dispatch_request_id: String::new(),
        graph_run_id: contract.graph_run_id.clone(),
        contract_id: contract.contract_id.clone(),
        contract_sha256: contract.contract_sha256.clone(),
        node_id: contract.node.node_id.clone(),
        attempt: contract.node.attempt,
        request_sha256: contract.request.request_sha256.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        provider: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        pricing_snapshot_sha256: contract.budgets.pricing_snapshot_sha256.clone(),
        provider_request_sha256: group_agent_node_provider_request_sha256(&body),
        provider_request_body: body,
        destination_sha256: group_agent_node_destination_sha256(
            contract.provider.kind,
            &contract.provider.endpoint,
            &contract.provider.model,
        ),
        dispatch_request_sha256: String::new(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: 2,
        expected_last_event_sha256: inspection
            .admission_event
            .expected_sha256()
            .expect("admission head"),
        event: placeholder_event(&contract.graph_run_id),
        event_json: String::new(),
        idempotency_key: key.into(),
        prepared_at_ms,
    };
    recanonicalize(&mut request);
    request
}

fn provider_body(contract: &forge_runtime_domain::GroupAgentNodeExecutionContract) -> Vec<u8> {
    let logical = ModelRequest {
        system_prompt: contract.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: contract.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: contract.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    };
    OpenAiResponsesProvider::encode_request_bytes(&contract.provider.model, &logical)
        .expect("encode exact provider request")
}

pub fn recanonicalize(request: &mut PrepareGroupAgentNodeDispatchRequest) {
    request.dispatch_request_sha256 = request.expected_sha256().expect("dispatch request digest");
    request.dispatch_request_id =
        group_agent_node_dispatch_request_id(&request.dispatch_request_sha256);
    request.event = preparation_event(request);
    request.event_json = request
        .event
        .canonical_json()
        .expect("canonical seq-3 event");
    request
        .validate()
        .expect("valid dispatch preparation fixture");
}

fn preparation_event(request: &PrepareGroupAgentNodeDispatchRequest) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        graph_run_id: request.graph_run_id.clone(),
        seq: 3,
        kind: GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256: request.expected_last_event_sha256.clone(),
            contract_id: request.contract_id.clone(),
            contract_sha256: request.contract_sha256.clone(),
            dispatch_request_id: request.dispatch_request_id.clone(),
            dispatch_request_sha256: request.dispatch_request_sha256.clone(),
            request_body_sha256: request.provider_request_sha256.clone(),
            request_body_bytes: request.provider_request_body.len(),
            logical_request_sha256: request.request_sha256.clone(),
            node_id: request.node_id.clone(),
            attempt: request.attempt,
            project_lane_sha256: request.project_lane_sha256.clone(),
            codec_protocol_version: request.codec_protocol_version,
            provider_kind: request.provider,
            destination_sha256: request.destination_sha256.clone(),
            pricing_snapshot_sha256: request.pricing_snapshot_sha256.clone(),
            prepared_at_ms: request.prepared_at_ms,
        },
    }
}

fn placeholder_event(graph_run_id: &str) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        graph_run_id: graph_run_id.into(),
        seq: 3,
        kind: GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256: String::new(),
            contract_id: String::new(),
            contract_sha256: String::new(),
            dispatch_request_id: String::new(),
            dispatch_request_sha256: String::new(),
            request_body_sha256: String::new(),
            request_body_bytes: 0,
            logical_request_sha256: String::new(),
            node_id: String::new(),
            attempt: 0,
            project_lane_sha256: String::new(),
            codec_protocol_version: 0,
            provider_kind: GroupAgentNodeProviderKind::OpenAiResponses,
            destination_sha256: String::new(),
            pricing_snapshot_sha256: String::new(),
            prepared_at_ms: 0,
        },
    }
}
