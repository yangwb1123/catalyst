#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
mod sqlite_group_agent_node_dispatch_request_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;

use forge_runtime_domain::{
    GroupAgentGraphRunStore, GroupAgentNodeDispatchRequestStore,
    PrepareGroupAgentNodeDispatchRequest, group_agent_node_provider_request_sha256,
};
use rusqlite::params;

use sqlite_group_agent_node_dispatch_request_support::{admitted_fixture, recanonicalize, request};

#[test]
fn reader_rejects_self_consistent_body_that_disagrees_with_contract() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    seed_dispatch(&fixture, &candidate);
    let mut replacement = candidate.clone();
    replacement.provider_request_body = b"{}".to_vec();
    replacement.provider_request_sha256 =
        group_agent_node_provider_request_sha256(&replacement.provider_request_body);
    recanonicalize(&mut replacement);
    persist_body_drift(&fixture, &replacement);

    assert_corrupt(&fixture.store.inspect_group_agent_graph_run("graph-run-1"));
}

#[test]
fn reader_rejects_event_beyond_declared_journal_head() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    seed_dispatch(&fixture, &candidate);
    fixture
        .connection()
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON;
             INSERT INTO group_agent_graph_run_events
             SELECT graph_run_id,4,event_version,kind,event_blob,event_bytes,
                    event_sha256,created_at_ms
             FROM group_agent_graph_run_events WHERE seq=3;",
        )
        .expect("append hidden fourth event");

    assert_corrupt(&fixture.store.inspect_group_agent_graph_run("graph-run-1"));
}

fn persist_body_drift(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    replacement: &PrepareGroupAgentNodeDispatchRequest,
) {
    let mut connection = fixture.connection();
    let transaction = connection.transaction().expect("body drift transaction");
    transaction
        .execute(
            "UPDATE group_agent_graph_node_dispatch_requests
             SET id=?1,provider_request_blob=?2,provider_request_bytes=?3,
                 provider_request_sha256=?4,dispatch_request_sha256=?5",
            params![
                replacement.dispatch_request_id,
                replacement.provider_request_body,
                i64::try_from(replacement.provider_request_body.len()).expect("body length"),
                decode_hex(&replacement.provider_request_sha256),
                decode_hex(&replacement.dispatch_request_sha256),
            ],
        )
        .expect("persist self-consistent body drift");
    let event_sha256 = replacement.event.expected_sha256().expect("event digest");
    transaction
        .execute(
            "UPDATE group_agent_graph_run_events
             SET event_blob=?1,event_bytes=?2,event_sha256=?3 WHERE seq=3",
            params![
                replacement.event_json.as_bytes(),
                i64::try_from(replacement.event_json.len()).expect("event length"),
                decode_hex(&event_sha256),
            ],
        )
        .expect("persist rebound preparation event");
    transaction
        .execute_batch(
            "UPDATE group_agent_graph_runs SET journal_bytes=(
               SELECT sum(event_bytes) FROM group_agent_graph_run_events
               WHERE graph_run_id='graph-run-1'
             ) WHERE id='graph-run-1';",
        )
        .expect("update journal bytes");
    transaction.commit().expect("commit body drift");
}

fn seed_dispatch(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    request: &PrepareGroupAgentNodeDispatchRequest,
) {
    fixture
        .store
        .prepare_group_agent_node_dispatch_request(request)
        .expect("seed request");
}

fn assert_corrupt<T>(result: &Result<T, forge_runtime_domain::HubStoreError>) {
    assert!(matches!(
        result,
        Err(forge_runtime_domain::HubStoreError::Corrupt { .. })
    ));
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
