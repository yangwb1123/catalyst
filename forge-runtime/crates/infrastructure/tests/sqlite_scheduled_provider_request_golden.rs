use forge_runtime_domain::{
    Cancellation, GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION, GroupAgentScheduledNodeContractCandidate,
    Message, ModelRequest, PrepareGroupAgentScheduledNodeProviderRequest,
    group_agent_node_destination_sha256, group_agent_node_provider_request_sha256,
    group_agent_scheduled_node_provider_request_id,
};
use forge_runtime_infrastructure::OpenAiResponsesProvider;

#[test]
fn fixed_scheduled_contract_encodes_the_exact_production_request_and_envelope_golden() {
    let candidate = fixture_candidate();
    let body = OpenAiResponsesProvider::encode_request_bytes(
        &candidate.provider.model,
        &model_request(&candidate),
    )
    .expect("encode fixed scheduled request");
    let request = prepared_request(&candidate, body.clone());

    assert_eq!(
        String::from_utf8(body).expect("UTF-8 provider body"),
        r#"{"include":["reasoning.encrypted_content"],"input":[{"content":"{\"v\":2,\"node_id\":\"frontend\",\"task\":\"Implement browser flow for café users.\",\"acceptance\":\"Browser uses the shared issuer.\"}","role":"user","type":"message"}],"instructions":"Execute exactly one frozen Group Agent Graph node. Follow the manager instruction, complete only the assigned task, and return a text result that can be checked against the acceptance criteria. Tools, network, workspace access, memory, and writeback are unavailable.\n\nManager instruction:\nCoordinate frontend, backend, and SSO <safely>.","max_output_tokens":4096,"model":"gpt-5.6-sol","store":false,"stream":true,"tools":[]}"#
    );
    assert_eq!(
        request.provider_request_sha256,
        "b2c2fbe92570461603b50692226591b6cd43c5aebef34d4dead01a428faccbff"
    );
    assert_eq!(
        request.prepared_request_sha256,
        "7ef2b085513f247b632e8a6be67cae2a29321eeb9d6f09a177dbcc0a525dd4a7"
    );
    assert_eq!(
        request.provider_request_id,
        "scheduled-node-provider-request-7ef2b085513f247b632e8a6be67cae2a29321eeb9d6f09a177dbcc0a525dd4a7"
    );
}

fn fixture_candidate() -> GroupAgentScheduledNodeContractCandidate {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-scheduled-node-contract-v2.json"
    )))
    .expect("scheduled contract fixture");
    GroupAgentScheduledNodeContractCandidate::decode_exact(
        fixture["expected"]["canonical_contract_json"]
            .as_str()
            .expect("canonical contract fixture"),
    )
    .expect("decode fixed scheduled contract")
}

fn model_request(candidate: &GroupAgentScheduledNodeContractCandidate) -> ModelRequest {
    ModelRequest {
        system_prompt: candidate.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: candidate.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: candidate.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    }
}

fn prepared_request(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    body: Vec<u8>,
) -> PrepareGroupAgentScheduledNodeProviderRequest {
    let mut request = request_without_identity(candidate, body);
    request.prepared_request_sha256 = request.expected_sha256().expect("envelope digest");
    request.provider_request_id =
        group_agent_scheduled_node_provider_request_id(&request.prepared_request_sha256);
    request.validate().expect("valid fixed prepared request");
    request
}

fn request_without_identity(
    candidate: &GroupAgentScheduledNodeContractCandidate,
    body: Vec<u8>,
) -> PrepareGroupAgentScheduledNodeProviderRequest {
    PrepareGroupAgentScheduledNodeProviderRequest {
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
        idempotency_key: "scheduled-provider-golden-key".into(),
        prepared_at_ms: 60,
    }
}
