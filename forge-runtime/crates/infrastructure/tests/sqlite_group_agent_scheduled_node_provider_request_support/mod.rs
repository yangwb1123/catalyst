use forge_runtime_domain::{
    Cancellation, GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION, GroupAgentScheduledNodeContractInspection,
    GroupAgentScheduledNodeContractStore, Message, ModelRequest,
    PrepareGroupAgentScheduledNodeProviderRequest, group_agent_node_destination_sha256,
    group_agent_node_provider_request_sha256, group_agent_scheduled_node_provider_request_id,
};
use forge_runtime_infrastructure::OpenAiResponsesProvider;

use super::{
    sqlite_group_agent_graph_run_support::Fixture,
    sqlite_group_agent_scheduled_node_contract_support as contract_support,
};

pub fn prepared_fixture() -> (Fixture, PrepareGroupAgentScheduledNodeProviderRequest) {
    let (fixture, candidate) = contract_support::prepared_fixture();
    let source = fixture
        .store
        .admit_group_agent_scheduled_node_contract(&candidate)
        .expect("admit scheduled provider-request source")
        .inspection;
    let request = request(&source, "scheduled-provider-key", 60);
    (fixture, request)
}

pub fn request(
    source: &GroupAgentScheduledNodeContractInspection,
    key: &str,
    prepared_at_ms: u64,
) -> PrepareGroupAgentScheduledNodeProviderRequest {
    let candidate = &source.candidate;
    let body = provider_body(source);
    let mut request = PrepareGroupAgentScheduledNodeProviderRequest {
        v: GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION,
        provider_request_id: String::new(),
        graph_run_id: candidate.graph_run_id.clone(),
        schedule_id: candidate.schedule_id.clone(),
        scheduled_contract_id: candidate.contract_id.clone(),
        execution_ordinal: candidate.node.execution_ordinal,
        node_id: candidate.node.node_id.clone(),
        attempt: candidate.node.attempt,
        scheduled_contract_sha256: candidate.contract_sha256.clone(),
        logical_request_id: candidate.request.request_id.clone(),
        logical_request_sha256: candidate.request.request_sha256.clone(),
        schedule_sha256: candidate.schedule_sha256.clone(),
        project_lane_sha256: candidate.node.project_lane_sha256.clone(),
        provider: candidate.provider.kind,
        endpoint: candidate.provider.endpoint.clone(),
        model: candidate.provider.model.clone(),
        destination_sha256: group_agent_node_destination_sha256(
            candidate.provider.kind,
            &candidate.provider.endpoint,
            &candidate.provider.model,
        ),
        pricing_snapshot_sha256: candidate.budgets.pricing_snapshot_sha256.clone(),
        provider_request_sha256: group_agent_node_provider_request_sha256(&body),
        provider_request_body: body,
        prepared_request_sha256: String::new(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: candidate.expected_last_event_seq,
        expected_last_event_sha256: candidate.expected_last_event_sha256.clone(),
        provider_request_prepared: true,
        provider_request_sent: false,
        lifecycle_contract_admitted: false,
        execution_authority_released: false,
        dispatch_authority_released: false,
        project_lane_claimed: false,
        progress_observed: false,
        successor_advance_authorized: false,
        idempotency_key: key.into(),
        prepared_at_ms,
    };
    reidentify(&mut request);
    request
}

pub fn provider_body(source: &GroupAgentScheduledNodeContractInspection) -> Vec<u8> {
    OpenAiResponsesProvider::encode_request_bytes(
        &source.candidate.provider.model,
        &model_request(source),
    )
    .expect("encode scheduled provider request")
}

pub fn reidentify(request: &mut PrepareGroupAgentScheduledNodeProviderRequest) {
    request.destination_sha256 =
        group_agent_node_destination_sha256(request.provider, &request.endpoint, &request.model);
    request.provider_request_sha256 =
        group_agent_node_provider_request_sha256(&request.provider_request_body);
    request.prepared_request_sha256 = request
        .expected_sha256()
        .expect("scheduled provider-request digest");
    request.provider_request_id =
        group_agent_scheduled_node_provider_request_id(&request.prepared_request_sha256);
    request
        .validate()
        .expect("valid scheduled provider-request candidate");
}

pub fn reencode_and_reidentify(request: &mut PrepareGroupAgentScheduledNodeProviderRequest) {
    let logical = ModelRequest {
        system_prompt: "You coordinate delivery for a session group. Follow the node task and acceptance criteria exactly. Do not act outside the supplied workspace and tool policy.".into(),
        messages: vec![Message::User {
            text: "Execute graph node 'frontend' task: Implement the frontend client. Acceptance: UI calls the backend contract and handles SSO redirects.".into(),
        }],
        tools: Vec::new(),
        max_output_tokens: 1024,
        cancellation: Cancellation::default(),
    };
    request.provider_request_body =
        OpenAiResponsesProvider::encode_request_bytes(&request.model, &logical)
            .expect("re-encode divergent provider request");
    reidentify(request);
}

fn model_request(source: &GroupAgentScheduledNodeContractInspection) -> ModelRequest {
    ModelRequest {
        system_prompt: source.candidate.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: source.candidate.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: source.candidate.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    }
}

pub fn admit_backend_successor(
    store: &forge_runtime_infrastructure::SqliteHubStore,
    initial_admit: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
) -> forge_runtime_domain::GroupAgentScheduledNodeContractInspection {
    use forge_runtime_domain::{
        GroupAgentScheduledNodeContractScope, GroupAgentScheduledNodeSuccessorStore,
    };
    let mut backend = initial_admit.candidate.clone();
    backend.contract_scope = GroupAgentScheduledNodeContractScope::ScheduleSuccessorOnly;
    let node = schedule_node(initial_admit, 1);
    bind_backend_node(&mut backend, initial_admit, &node);
    backend.request.predecessor_terminal_receipts.clear();
    backend.request.required_predecessor_node_ids.clear();
    backend.request.user_prompt = format!(
        "{{\"v\":2,\"node_id\":\"{}\",\"task\":\"backend task\",\"acceptance\":\"ok\"}}",
        node.node_id
    );
    resign_candidate_digests(&mut backend);
    let backend_json = backend.canonical_json().expect("backend canonical JSON");
    let mut backend_admit = initial_admit.clone();
    backend_admit.idempotency_key = "scheduled-backend-key".into();
    backend_admit.candidate = backend;
    backend_admit.candidate_json = backend_json;
    store
        .admit_group_agent_scheduled_node_successor(&backend_admit)
        .expect("admit backend successor")
        .inspection
}

pub fn schedule_node(
    initial_admit: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    ordinal: usize,
) -> forge_runtime_domain::GroupAgentGraphExecutionScheduleNode {
    initial_admit
        .schedule
        .nodes
        .iter()
        .find(|node| node.execution_ordinal == ordinal)
        .expect("ordinal schedule node")
        .clone()
}

pub fn bind_backend_node(
    backend: &mut forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
    initial_admit: &forge_runtime_domain::AdmitGroupAgentScheduledNodeContractCandidate,
    node: &forge_runtime_domain::GroupAgentGraphExecutionScheduleNode,
) {
    backend.node.execution_ordinal = 1;
    backend.node.node_id.clone_from(&node.node_id);
    backend.node.authored_node_index = node.authored_node_index;
    backend.node.topology_wave_index = node.topology_wave_index;
    backend.node.project_lane_sha256.clone_from(&node.project_lane_sha256);
    let project_id = initial_admit
        .control_snapshot
        .manifest
        .nodes
        .iter()
        .find(|manifest_node| manifest_node.node_id == node.node_id)
        .map(|manifest_node| manifest_node.project_id.clone())
        .expect("backend manifest project");
    backend.node.project_id.clone_from(&project_id);
    backend.request.execution_ordinal = 1;
    backend.request.node_id.clone_from(&node.node_id);
}

pub fn resign_candidate_digests(
    candidate: &mut forge_runtime_domain::GroupAgentScheduledNodeContractCandidate,
) {
    candidate.request.user_prompt_bytes = candidate.request.user_prompt.len();
    candidate.request.user_prompt_sha256 =
        forge_runtime_domain::group_agent_prompt_sha256(&candidate.request.user_prompt);
    let request_digest = candidate.request.expected_sha256().expect("request digest");
    candidate.request.request_id = format!("scheduled-node-request-{request_digest}");
    candidate.request.request_sha256 = request_digest;
    let contract_digest = candidate.expected_sha256().expect("contract digest");
    candidate.contract_id = format!("scheduled-node-contract-{contract_digest}");
    candidate.contract_sha256 = contract_digest;
}

