use crate::{
    OpenAiResponsesProvider,
    runtime_domain::{
        Cancellation, GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION,
        GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION, GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunStore,
        GroupAgentGraphStore, GroupAgentNodeProviderKind, HubStoreError, Message, ModelRequest,
        PrepareGroupAgentNodeDispatchRequest, group_agent_node_destination_sha256,
        group_agent_node_dispatch_request_id, group_agent_node_provider_request_sha256,
    },
    sqlite_hub::{group_agent_graph, group_agent_graph_run},
};

use super::super::{atomicity_tests as contract_atomicity, write as contract_write};
use super::write;

#[test]
fn late_reread_fault_rolls_back_request_event_and_v3_transition() {
    let (fixture, request) = fixture_and_request();
    let mut connection = fixture.store.connect().expect("validated connection");

    let error = write::prepare_with_before_reread(&mut connection, &request, || {
        Err(HubStoreError::Corrupt {
            message: "injected late dispatch reread fault".into(),
        })
    })
    .expect_err("late fault aborts request preparation");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_v2_unchanged(&connection);
    assert!(connection.is_autocommit());
}

#[test]
fn journal_head_cas_failure_rolls_back_trigger_mutation_and_all_new_rows() {
    let (fixture, request) = fixture_and_request();
    let mut connection = fixture.store.connect().expect("validated connection");
    connection
        .execute_batch(
            "CREATE TRIGGER mutate_head_after_dispatch_request
             AFTER INSERT ON group_agent_graph_node_dispatch_requests
             BEGIN
               UPDATE group_agent_graph_run_events SET event_sha256=zeroblob(32)
               WHERE graph_run_id=NEW.graph_run_id AND seq=2;
             END;",
        )
        .expect("install head mutation trigger");

    let error = write::prepare(&mut connection, &request)
        .expect_err("head CAS rejects late journal mutation");
    assert!(matches!(error, HubStoreError::Conflict { .. }));
    assert_v2_unchanged(&connection);
    let stored_head: Vec<u8> = connection
        .query_row(
            "SELECT event_sha256 FROM group_agent_graph_run_events
             WHERE graph_run_id='graph-run-1' AND seq=2",
            [],
            |row| row.get(0),
        )
        .expect("read rolled-back event head");
    assert_eq!(stored_head, decode_hex(&request.expected_last_event_sha256));
    assert!(connection.is_autocommit());
}

fn fixture_and_request() -> (
    group_agent_graph::atomicity_tests::Fixture,
    PrepareGroupAgentNodeDispatchRequest,
) {
    let fixture = group_agent_graph::atomicity_tests::fixture();
    let graph = fixture
        .store
        .prepare_group_agent_graph(&fixture.request)
        .expect("prepare graph")
        .inspection;
    let run = fixture
        .store
        .begin_group_agent_graph_run(&group_agent_graph_run::atomicity_tests::request(&graph))
        .expect("prepare Graph Run")
        .inspection;
    let admission = contract_atomicity::admission(&graph, &run);
    let mut connection = fixture.store.connect().expect("contract connection");
    let contract = contract_write::admit(&mut connection, &admission)
        .expect("admit contract")
        .inspection;
    (fixture, dispatch_request(&contract))
}

fn dispatch_request(
    inspection: &crate::runtime_domain::GroupAgentNodeExecutionContractInspection,
) -> PrepareGroupAgentNodeDispatchRequest {
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
        idempotency_key: "dispatch-request-key".into(),
        prepared_at_ms: 50,
    };
    request.dispatch_request_sha256 = request.expected_sha256().expect("dispatch request digest");
    request.dispatch_request_id =
        group_agent_node_dispatch_request_id(&request.dispatch_request_sha256);
    request.event = preparation_event(&request);
    request.event_json = request.event.canonical_json().expect("canonical event");
    request
}

fn provider_body(contract: &crate::runtime_domain::GroupAgentNodeExecutionContract) -> Vec<u8> {
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
        .expect("encode provider request")
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

fn assert_v2_unchanged(connection: &rusqlite::Connection) {
    assert_eq!(
        row_count(connection, "group_agent_graph_node_dispatch_requests"),
        0
    );
    assert_eq!(row_count(connection, "group_agent_graph_run_events"), 2);
    let row: (i64, String, i64, i64) = connection
        .query_row(
            "SELECT run_version,status,dispatch_request_present,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("base v2 Graph Run");
    assert_eq!(row, (2, "awaiting_core_dispatch".into(), 0, 2));
}

fn row_count(connection: &rusqlite::Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn decode_hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex ASCII");
            u8::from_str_radix(text, 16).expect("valid hex")
        })
        .collect()
}
