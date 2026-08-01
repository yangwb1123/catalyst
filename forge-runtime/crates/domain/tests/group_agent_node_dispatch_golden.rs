use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION, GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
    GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION, GroupAgentGraphRunEvent,
    GroupAgentGraphRunEventKind, GroupAgentNodeDispatchRequestRecord,
    GroupAgentNodeExecutionContract, group_agent_node_destination_sha256,
    group_agent_node_dispatch_request_id, group_agent_node_provider_request_sha256,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct SharedFixture {
    expected: SharedExpected,
}

#[derive(Deserialize)]
struct SharedExpected {
    canonical_contract_json: String,
    canonical_provider_request_body_json: String,
    provider_request_bytes: usize,
    provider_request_sha256: String,
    destination_sha256: String,
    admission_event_sha256: String,
    canonical_dispatch_request_payload_json: String,
    dispatch_request_sha256: String,
    dispatch_request_id: String,
    prepared_at_ms: u64,
    canonical_preparation_event_json: String,
    preparation_event_sha256: String,
}

#[test]
fn shared_go_contract_and_provider_body_have_the_exact_rust_dispatch_identity() {
    let fixture: SharedFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-node-execution-contract-v1.json"
    )))
    .expect("shared fixture");
    let expected = fixture.expected;
    let contract: GroupAgentNodeExecutionContract =
        serde_json::from_str(&expected.canonical_contract_json).expect("contract JSON");
    let body = expected.canonical_provider_request_body_json.as_bytes();
    let record = dispatch_record(&contract, &expected);

    assert_eq!(body.len(), expected.provider_request_bytes);
    assert_eq!(
        group_agent_node_provider_request_sha256(body),
        expected.provider_request_sha256
    );
    assert_eq!(
        group_agent_node_destination_sha256(
            contract.provider.kind,
            &contract.provider.endpoint,
            &contract.provider.model,
        ),
        expected.destination_sha256
    );
    assert_eq!(
        record.canonical_payload_json().expect("dispatch payload"),
        expected.canonical_dispatch_request_payload_json
    );
    assert_eq!(
        record.expected_sha256().expect("dispatch digest"),
        expected.dispatch_request_sha256
    );
    assert_eq!(record.dispatch_request_id, expected.dispatch_request_id);
    record.validate().expect("shared dispatch record");
    let event = preparation_event(&record, expected.prepared_at_ms);
    assert_eq!(
        event.canonical_json().expect("preparation event"),
        expected.canonical_preparation_event_json
    );
    assert_eq!(
        event.expected_sha256().expect("preparation event digest"),
        expected.preparation_event_sha256
    );
    let mut wrong_run = event;
    wrong_run.graph_run_id = "another-valid-run".into();
    assert!(record.validate_preparation_event(&wrong_run).is_err());
}

fn dispatch_record(
    contract: &GroupAgentNodeExecutionContract,
    expected: &SharedExpected,
) -> GroupAgentNodeDispatchRequestRecord {
    GroupAgentNodeDispatchRequestRecord {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        dispatch_request_id: group_agent_node_dispatch_request_id(
            &expected.dispatch_request_sha256,
        ),
        graph_run_id: contract.graph_run_id.clone(),
        contract_id: contract.contract_id.clone(),
        node_id: contract.node.node_id.clone(),
        attempt: contract.node.attempt,
        contract_sha256: contract.contract_sha256.clone(),
        request_sha256: contract.request.request_sha256.clone(),
        project_lane_sha256: contract.node.project_lane_sha256.clone(),
        provider: contract.provider.kind,
        endpoint: contract.provider.endpoint.clone(),
        model: contract.provider.model.clone(),
        pricing_snapshot_sha256: contract.budgets.pricing_snapshot_sha256.clone(),
        provider_request_sha256: expected.provider_request_sha256.clone(),
        provider_request_bytes: expected.provider_request_bytes,
        destination_sha256: expected.destination_sha256.clone(),
        dispatch_request_sha256: expected.dispatch_request_sha256.clone(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: 2,
        expected_last_event_sha256: expected.admission_event_sha256.clone(),
        created_at_ms: 90,
    }
}

fn preparation_event(
    record: &GroupAgentNodeDispatchRequestRecord,
    prepared_at_ms: u64,
) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        graph_run_id: record.graph_run_id.clone(),
        seq: 3,
        kind: GroupAgentGraphRunEventKind::NodeDispatchRequestPrepared {
            previous_event_sha256: record.expected_last_event_sha256.clone(),
            contract_id: record.contract_id.clone(),
            contract_sha256: record.contract_sha256.clone(),
            dispatch_request_id: record.dispatch_request_id.clone(),
            dispatch_request_sha256: record.dispatch_request_sha256.clone(),
            request_body_sha256: record.provider_request_sha256.clone(),
            request_body_bytes: record.provider_request_bytes,
            logical_request_sha256: record.request_sha256.clone(),
            node_id: record.node_id.clone(),
            attempt: record.attempt,
            project_lane_sha256: record.project_lane_sha256.clone(),
            codec_protocol_version: record.codec_protocol_version,
            provider_kind: record.provider,
            destination_sha256: record.destination_sha256.clone(),
            pricing_snapshot_sha256: record.pricing_snapshot_sha256.clone(),
            prepared_at_ms,
        },
    }
}
