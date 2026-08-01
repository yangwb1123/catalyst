#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
mod sqlite_group_agent_node_dispatch_request_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;

use forge_runtime_domain::{
    Cancellation, GroupAgentGraphRunEventKind, GroupAgentGraphRunStore,
    GroupAgentNodeDispatchRequestStore, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractStore, HubEntity, HubStoreError, Message, ModelRequest,
    PrepareGroupAgentNodeDispatchRequestDisposition, group_agent_node_provider_request_sha256,
    group_agent_prompt_sha256,
};
use forge_runtime_infrastructure::OpenAiResponsesProvider;
use rusqlite::params;

use sqlite_group_agent_node_dispatch_request_support::{admitted_fixture, recanonicalize, request};
use sqlite_group_agent_node_execution_contract_support::{
    prepared_fixture, request_for_run as contract_request,
};

type PromptMutation = fn(&mut GroupAgentNodeExecutionContract);

#[test]
fn locally_consistent_manager_and_task_prompt_drift_is_corrupt() {
    let cases: [(&str, PromptMutation); 2] = [
        ("manager instruction", drift_system_prompt),
        ("node task and acceptance", drift_user_prompt),
    ];
    for (name, mutate) in cases {
        let (fixture, contract_id) = admitted_fixture();
        let mut candidate = request(&fixture, &contract_id, "dispatch-key", 50);
        let (contract, admission_head) = persist_prompt_drift(&fixture, &contract_id, mutate);
        bind_candidate_to_contract(&mut candidate, &contract, &admission_head);

        assert_corrupt(
            &fixture
                .store
                .prepare_group_agent_node_dispatch_request(&candidate),
            name,
        );
        assert_no_request_write(&fixture);
    }
}

#[test]
fn locally_consistent_seq2_replacement_conflicts_with_the_old_candidate_head() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    let new_head = replace_admission_time(&fixture, &contract_id);
    assert_ne!(candidate.expected_last_event_sha256, new_head);

    assert!(matches!(
        fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentNodeDispatchRequest,
            ..
        })
    ));
    assert_no_request_write(&fixture);
}

#[test]
fn every_v2_stored_byte_count_drift_is_corrupt_before_writes() {
    let cases = [
        (
            "Core Plan",
            "UPDATE group_agent_graph_runs SET plan_bytes=plan_bytes+1",
        ),
        (
            "execution contract",
            "UPDATE group_agent_graph_node_execution_contracts \
             SET contract_bytes=contract_bytes+1",
        ),
        (
            "seq-2 event",
            "UPDATE group_agent_graph_run_events SET event_bytes=event_bytes+1 WHERE seq=2",
        ),
    ];
    for (name, statement) in cases {
        let (fixture, contract_id) = admitted_fixture();
        let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
        fixture
            .connection()
            .execute_batch(&format!("PRAGMA ignore_check_constraints=ON; {statement}"))
            .expect("persist byte-count drift");

        assert_corrupt(
            &fixture
                .store
                .prepare_group_agent_node_dispatch_request(&candidate),
            name,
        );
        assert_no_request_write(&fixture);
    }
}

#[test]
fn stored_provider_request_byte_count_drift_is_corrupt_before_replay() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    fixture
        .store
        .prepare_group_agent_node_dispatch_request(&candidate)
        .expect("seed exact request");
    fixture
        .connection()
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON;
             UPDATE group_agent_graph_node_dispatch_requests
             SET provider_request_bytes=provider_request_bytes+1",
        )
        .expect("persist request byte-count drift");

    assert_corrupt(
        &fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
        "provider request",
    );
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_node_dispatch_request(&candidate.dispatch_request_id),
        "provider request inspection",
    );
}

#[test]
fn two_runs_on_one_project_lane_can_both_prepare_without_claiming_it() {
    let fixture = prepared_fixture();
    fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-2", "run-key-2", 31))
        .expect("seed second Graph Run");
    let first_contract = admit_contract(&fixture, "graph-run-1", "contract-key-1", 40);
    let second_contract = admit_contract(&fixture, "graph-run-2", "contract-key-2", 41);
    let first = request(&fixture, &first_contract, "dispatch-key-1", 50);
    let second = request(&fixture, &second_contract, "dispatch-key-2", 51);
    assert_eq!(first.project_lane_sha256, second.project_lane_sha256);

    let first_result = fixture
        .store
        .prepare_group_agent_node_dispatch_request(&first)
        .expect("prepare first lane request");
    let second_result = fixture
        .store
        .prepare_group_agent_node_dispatch_request(&second)
        .expect("prepare second lane request");
    assert_eq!(
        first_result.disposition,
        PrepareGroupAgentNodeDispatchRequestDisposition::Created
    );
    assert_eq!(
        second_result.disposition,
        PrepareGroupAgentNodeDispatchRequestDisposition::Created
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        2
    );
    assert_passive_v3(&fixture, "graph-run-1");
    assert_passive_v3(&fixture, "graph-run-2");
}

fn admit_contract(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    graph_run_id: &str,
    key: &str,
    admitted_at_ms: u64,
) -> String {
    fixture
        .store
        .admit_group_agent_node_execution_contract(&contract_request(
            fixture,
            graph_run_id,
            key,
            admitted_at_ms,
        ))
        .expect("admit execution contract")
        .inspection
        .record
        .contract_id
}

fn assert_passive_v3(fixture: &sqlite_group_agent_graph_run_support::Fixture, graph_run_id: &str) {
    let run = fixture
        .store
        .inspect_group_agent_graph_run(graph_run_id)
        .expect("inspect passive v3 Run");
    assert!(run.run.dispatch_request_present);
    assert!(!run.run.dispatch_authority_released);
}

fn persist_prompt_drift(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    contract_id: &str,
    mutate: PromptMutation,
) -> (GroupAgentNodeExecutionContract, String) {
    let mut inspection = fixture
        .store
        .inspect_group_agent_node_execution_contract(contract_id)
        .expect("inspect contract before Prompt drift");
    let original_id = inspection.record.contract_id.clone();
    mutate(&mut inspection.contract);
    rehash_request_and_contract(&mut inspection.contract);
    let contract_json = inspection
        .contract
        .canonical_json()
        .expect("drifted contract JSON");
    let event = &mut inspection.admission_event;
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
        contract_id,
        contract_sha256,
        contract_bytes,
        request_sha256,
        ..
    } = &mut event.kind
    else {
        panic!("admission event");
    };
    contract_id.clone_from(&inspection.contract.contract_id);
    contract_sha256.clone_from(&inspection.contract.contract_sha256);
    *contract_bytes = contract_json.len();
    request_sha256.clone_from(&inspection.contract.request.request_sha256);
    let event_json = event.canonical_json().expect("drifted admission event");
    let event_sha256 = event.expected_sha256().expect("drifted admission digest");
    persist_contract_and_event(
        fixture,
        &original_id,
        &inspection.contract,
        contract_json.as_bytes(),
        event_json.as_bytes(),
        &event_sha256,
    );
    (inspection.contract, event_sha256)
}

fn rehash_request_and_contract(contract: &mut GroupAgentNodeExecutionContract) {
    let request = &mut contract.request;
    request.system_prompt_bytes = request.system_prompt.len();
    request.system_prompt_sha256 = group_agent_prompt_sha256(&request.system_prompt);
    request.user_prompt_bytes = request.user_prompt.len();
    request.user_prompt_sha256 = group_agent_prompt_sha256(&request.user_prompt);
    request.request_sha256 = request.expected_sha256().expect("drifted request digest");
    contract.contract_sha256 = contract.expected_sha256().expect("drifted contract digest");
    contract.contract_id = format!("node-contract-{}", contract.contract_sha256);
    contract.validate().expect("locally valid drifted contract");
}

fn persist_contract_and_event(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    original_id: &str,
    contract: &GroupAgentNodeExecutionContract,
    contract_json: &[u8],
    event_json: &[u8],
    event_sha256: &str,
) {
    let mut connection = fixture.connection();
    let transaction = connection.transaction().expect("Prompt drift transaction");
    transaction
        .execute(
            "UPDATE group_agent_graph_node_execution_contracts
             SET id=?1,contract_blob=?2,contract_bytes=?3,contract_sha256=?4,request_sha256=?5
             WHERE id=?6",
            params![
                contract.contract_id,
                contract_json,
                i64::try_from(contract_json.len()).expect("contract length"),
                decode_hex(&contract.contract_sha256),
                decode_hex(&contract.request.request_sha256),
                original_id,
            ],
        )
        .expect("persist Prompt-drifted contract");
    transaction
        .execute(
            "UPDATE group_agent_graph_run_events
             SET event_blob=?1,event_bytes=?2,event_sha256=?3 WHERE seq=2",
            params![
                event_json,
                i64::try_from(event_json.len()).expect("event length"),
                decode_hex(event_sha256),
            ],
        )
        .expect("persist Prompt-drifted admission event");
    update_journal_bytes(&transaction);
    transaction.commit().expect("commit Prompt drift");
}

fn bind_candidate_to_contract(
    candidate: &mut forge_runtime_domain::PrepareGroupAgentNodeDispatchRequest,
    contract: &GroupAgentNodeExecutionContract,
    admission_head: &str,
) {
    candidate.contract_id.clone_from(&contract.contract_id);
    candidate
        .contract_sha256
        .clone_from(&contract.contract_sha256);
    candidate
        .request_sha256
        .clone_from(&contract.request.request_sha256);
    candidate.provider_request_body = provider_body(contract);
    candidate.provider_request_sha256 =
        group_agent_node_provider_request_sha256(&candidate.provider_request_body);
    candidate.expected_last_event_sha256 = admission_head.into();
    recanonicalize(candidate);
}

fn provider_body(contract: &GroupAgentNodeExecutionContract) -> Vec<u8> {
    let request = ModelRequest {
        system_prompt: contract.request.system_prompt.clone(),
        messages: vec![Message::User {
            text: contract.request.user_prompt.clone(),
        }],
        tools: Vec::new(),
        max_output_tokens: contract.budgets.max_output_tokens,
        cancellation: Cancellation::default(),
    };
    OpenAiResponsesProvider::encode_request_bytes(&contract.provider.model, &request)
        .expect("encode drifted provider body")
}

fn replace_admission_time(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    contract_id: &str,
) -> String {
    let mut inspection = fixture
        .store
        .inspect_group_agent_node_execution_contract(contract_id)
        .expect("inspect contract before receipt replacement");
    let replacement_time = inspection.record.created_at_ms + 1;
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted { admitted_at_ms, .. } =
        &mut inspection.admission_event.kind
    else {
        panic!("admission event");
    };
    *admitted_at_ms = replacement_time;
    let event_json = inspection
        .admission_event
        .canonical_json()
        .expect("replacement event JSON");
    let event_sha256 = inspection
        .admission_event
        .expected_sha256()
        .expect("replacement event digest");
    let mut connection = fixture.connection();
    let transaction = connection.transaction().expect("receipt drift transaction");
    transaction
        .execute(
            "UPDATE group_agent_graph_node_execution_contracts SET created_at_ms=?1 WHERE id=?2",
            params![
                i64::try_from(replacement_time).expect("replacement time"),
                contract_id
            ],
        )
        .expect("update contract receipt time");
    transaction
        .execute(
            "UPDATE group_agent_graph_run_events
             SET event_blob=?1,event_bytes=?2,event_sha256=?3,created_at_ms=?4 WHERE seq=2",
            params![
                event_json.as_bytes(),
                i64::try_from(event_json.len()).expect("event length"),
                decode_hex(&event_sha256),
                i64::try_from(replacement_time).expect("replacement time"),
            ],
        )
        .expect("replace seq-2 receipt");
    update_journal_bytes(&transaction);
    transaction.commit().expect("commit receipt replacement");
    event_sha256
}

fn update_journal_bytes(transaction: &rusqlite::Transaction<'_>) {
    transaction
        .execute(
            "UPDATE group_agent_graph_runs SET journal_bytes=(
               SELECT sum(event_bytes) FROM group_agent_graph_run_events
               WHERE graph_run_id='graph-run-1'
             ) WHERE id='graph-run-1'",
            [],
        )
        .expect("update journal bytes");
}

fn drift_system_prompt(contract: &mut GroupAgentNodeExecutionContract) {
    contract.request.system_prompt = "locally consistent manager instruction drift".into();
}

fn drift_user_prompt(contract: &mut GroupAgentNodeExecutionContract) {
    contract.request.user_prompt = "locally consistent node task and acceptance drift".into();
}

fn assert_no_request_write(fixture: &sqlite_group_agent_graph_run_support::Fixture) {
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        0
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 2);
}

fn assert_corrupt<T>(result: &Result<T, HubStoreError>, case: &str) {
    assert!(
        matches!(result, Err(HubStoreError::Corrupt { .. })),
        "{case} did not report corruption"
    );
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
