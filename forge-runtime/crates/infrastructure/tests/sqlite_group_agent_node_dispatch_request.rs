#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
mod sqlite_group_agent_node_dispatch_request_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;

use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    GroupAgentGraphRunEventKind, GroupAgentGraphRunStatus, GroupAgentGraphRunStore,
    GroupAgentNodeDispatchRequestStore, GroupAgentNodeExecutionContractStore, HubEntity,
    HubStoreError, PrepareGroupAgentNodeDispatchRequestDisposition,
};
use rusqlite::params;

use sqlite_group_agent_node_dispatch_request_support::{admitted_fixture, recanonicalize, request};

#[test]
fn created_replay_and_views_preserve_exact_original_and_v3_state() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    let created = fixture
        .store
        .prepare_group_agent_node_dispatch_request(&candidate)
        .expect("prepare dispatch request");
    assert_eq!(
        created.disposition,
        PrepareGroupAgentNodeDispatchRequestDisposition::Created
    );
    assert_eq!(
        created.inspection.provider_request_body,
        candidate.provider_request_body
    );
    assert_eq!(created.inspection.preparation_event, candidate.event);

    let replay = request(&fixture, &contract_id, "dispatch-key", 999);
    let replayed = fixture
        .store
        .prepare_group_agent_node_dispatch_request(&replay)
        .expect("replay ignores candidate time");
    assert_eq!(
        replayed.disposition,
        PrepareGroupAgentNodeDispatchRequestDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_eq!(replayed.inspection.record.created_at_ms, 50);

    assert_request_views(&fixture, &created.inspection);
    assert_v3_run(&fixture, &candidate.event);
}

fn assert_request_views(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    expected: &forge_runtime_domain::GroupAgentNodeDispatchRequestInspection,
) {
    let inspected = fixture
        .store
        .inspect_group_agent_node_dispatch_request(&expected.record.dispatch_request_id)
        .expect("inspect request");
    assert_eq!(inspected, *expected);
    let listed = fixture
        .store
        .list_group_agent_node_dispatch_requests(Some("graph-run-1"), 10)
        .expect("list request metadata");
    assert_eq!(listed.as_slice(), std::slice::from_ref(&expected.record));
}

fn assert_v3_run(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    event: &forge_runtime_domain::GroupAgentGraphRunEvent,
) {
    let run = fixture
        .store
        .inspect_group_agent_graph_run("graph-run-1")
        .expect("inspect v3 Graph Run");
    assert_eq!(run.run.v, 3);
    assert_eq!(
        run.run.status,
        GroupAgentGraphRunStatus::AwaitingDispatchAuthorization
    );
    assert!(run.run.execution_contract_present);
    assert!(run.run.dispatch_request_present);
    assert!(!run.run.dispatch_authority_released);
    assert_eq!(run.events.len(), 3);
    assert_eq!(run.events[2], *event);
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        1
    );
}

#[test]
fn second_key_and_stale_head_conflict_without_partial_rows() {
    let (stale_fixture, stale_contract) = admitted_fixture();
    let mut stale = request(&stale_fixture, &stale_contract, "stale-key", 50);
    stale.expected_last_event_sha256 = "a".repeat(64);
    recanonicalize(&mut stale);
    assert_conflict(
        &stale_fixture
            .store
            .prepare_group_agent_node_dispatch_request(&stale),
    );
    assert_v2_unchanged(&stale_fixture);

    let (fixture, contract_id) = admitted_fixture();
    let original = request(&fixture, &contract_id, "dispatch-key", 50);
    fixture
        .store
        .prepare_group_agent_node_dispatch_request(&original)
        .expect("seed request");
    let second_key = request(&fixture, &contract_id, "second-key", 51);
    assert_conflict(
        &fixture
            .store
            .prepare_group_agent_node_dispatch_request(&second_key),
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        1
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 3);
}

#[test]
fn stored_body_corruption_wins_over_replay_and_metadata_list_stays_content_free() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    fixture
        .store
        .prepare_group_agent_node_dispatch_request(&candidate)
        .expect("seed request");
    fixture
        .connection()
        .execute_batch(
            "UPDATE group_agent_graph_node_dispatch_requests
             SET provider_request_blob=zeroblob(length(provider_request_blob))",
        )
        .expect("corrupt exact body only");

    let listed = fixture
        .store
        .list_group_agent_node_dispatch_requests(Some("graph-run-1"), 10)
        .expect("metadata list does not load exact body");
    assert_eq!(listed.len(), 1);
    assert_corrupt(
        &fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
    );
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_node_dispatch_request(&candidate.dispatch_request_id),
    );
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_graph_run(&candidate.graph_run_id),
    );
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_node_execution_contract(&candidate.contract_id),
    );
}

#[test]
fn orphaned_stored_request_is_corrupt_before_replay_semantics() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    fixture
        .store
        .prepare_group_agent_node_dispatch_request(&candidate)
        .expect("seed request");
    fixture
        .connection()
        .execute_batch(
            "PRAGMA foreign_keys=OFF;
             DELETE FROM group_agent_graph_runs WHERE id='graph-run-1';",
        )
        .expect("orphan stored request fixture");

    assert_corrupt(
        &fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
    );
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_node_dispatch_request(&candidate.dispatch_request_id),
    );
}

#[test]
fn transaction_rejects_a_locally_consistent_contract_that_drifted_from_graph_control() {
    let (fixture, contract_id) = admitted_fixture();
    let mut candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    let (drifted_id, drifted_sha256, admission_head) =
        drift_stored_contract_profile(&fixture, &contract_id);
    candidate.contract_id = drifted_id;
    candidate.contract_sha256 = drifted_sha256;
    candidate.expected_last_event_sha256 = admission_head;
    recanonicalize(&mut candidate);

    assert_corrupt(
        &fixture
            .store
            .prepare_group_agent_node_dispatch_request(&candidate),
    );
    assert_v2_unchanged(&fixture);
}

fn drift_stored_contract_profile(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    contract_id: &str,
) -> (String, String, String) {
    let mut inspection = fixture
        .store
        .inspect_group_agent_node_execution_contract(contract_id)
        .expect("inspect contract before drift");
    inspection.contract.node.agent_profile = "locally-consistent-drift".into();
    let contract_sha256 = inspection
        .contract
        .expected_sha256()
        .expect("drifted contract digest");
    let drifted_id = format!("node-contract-{contract_sha256}");
    let stored_contract = &mut inspection.contract;
    stored_contract.contract_id.clone_from(&drifted_id);
    stored_contract.contract_sha256.clone_from(&contract_sha256);
    let contract_json = stored_contract.canonical_json().expect("contract JSON");
    let GroupAgentGraphRunEventKind::NodeExecutionContractAdmitted {
        contract_id,
        contract_sha256: event_contract_sha256,
        contract_bytes,
        ..
    } = &mut inspection.admission_event.kind
    else {
        panic!("admission event");
    };
    contract_id.clone_from(&drifted_id);
    event_contract_sha256.clone_from(&contract_sha256);
    *contract_bytes = contract_json.len();
    let event_json = inspection
        .admission_event
        .canonical_json()
        .expect("admission event JSON");
    let event_sha256 = inspection
        .admission_event
        .expected_sha256()
        .expect("admission event digest");
    persist_contract_drift(
        fixture,
        &inspection.record.contract_id,
        &drifted_id,
        &contract_sha256,
        contract_json.as_bytes(),
        event_json.as_bytes(),
        &event_sha256,
    );
    (drifted_id, contract_sha256, event_sha256)
}

fn persist_contract_drift(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    original_id: &str,
    drifted_id: &str,
    contract_sha256: &str,
    contract_json: &[u8],
    event_json: &[u8],
    event_sha256: &str,
) {
    let mut connection = fixture.connection();
    let transaction = connection.transaction().expect("drift transaction");
    transaction
        .execute(
            "UPDATE group_agent_graph_node_execution_contracts
             SET id=?1,contract_blob=?2,contract_bytes=?3,contract_sha256=?4
             WHERE id=?5",
            params![
                drifted_id,
                contract_json,
                i64::try_from(contract_json.len()).expect("contract length"),
                decode_hex(contract_sha256),
                original_id,
            ],
        )
        .expect("persist drifted contract");
    transaction
        .execute(
            "UPDATE group_agent_graph_run_events
             SET event_blob=?1,event_bytes=?2,event_sha256=?3
             WHERE graph_run_id='graph-run-1' AND seq=2",
            params![
                event_json,
                i64::try_from(event_json.len()).expect("event length"),
                decode_hex(event_sha256),
            ],
        )
        .expect("persist drifted admission event");
    transaction
        .execute(
            "UPDATE group_agent_graph_runs SET journal_bytes=(
               SELECT sum(event_bytes) FROM group_agent_graph_run_events
               WHERE graph_run_id='graph-run-1'
             ) WHERE id='graph-run-1'",
            [],
        )
        .expect("update drifted journal bytes");
    transaction.commit().expect("commit contract drift");
}

#[test]
fn request_event_metadata_drift_invalidates_every_v3_source_view() {
    let (fixture, contract_id) = admitted_fixture();
    let candidate = request(&fixture, &contract_id, "dispatch-key", 50);
    fixture
        .store
        .prepare_group_agent_node_dispatch_request(&candidate)
        .expect("seed request");
    fixture
        .connection()
        .execute_batch(
            "UPDATE group_agent_graph_node_dispatch_requests
             SET created_at_ms=created_at_ms+1",
        )
        .expect("drift request receipt time");

    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_graph_run(&candidate.graph_run_id),
    );
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_node_execution_contract(&candidate.contract_id),
    );
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_node_dispatch_request(&candidate.dispatch_request_id),
    );
}

#[test]
fn concurrent_same_key_creates_once_and_all_workers_observe_one_exact_request() {
    const WORKERS: usize = 6;
    let (fixture, contract_id) = admitted_fixture();
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            let candidate = request(
                &fixture,
                &contract_id,
                "shared-dispatch-key",
                60 + u64::try_from(index).expect("worker index fits"),
            );
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .prepare_group_agent_node_dispatch_request(&candidate)
                    .expect("concurrent preparation")
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("preparation worker"))
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                result.disposition == PrepareGroupAgentNodeDispatchRequestDisposition::Created
            })
            .count(),
        1
    );
    assert!(
        results
            .iter()
            .all(|result| result.inspection == results[0].inspection)
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        1
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 3);
}

fn assert_v2_unchanged(fixture: &sqlite_group_agent_graph_run_support::Fixture) {
    assert_eq!(
        fixture.row_count("group_agent_graph_node_dispatch_requests"),
        0
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 2);
    let row: (i64, String, i64, i64) = fixture
        .connection()
        .query_row(
            "SELECT run_version,status,dispatch_request_present,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read unchanged v2 row");
    assert_eq!(row, (2, "awaiting_core_dispatch".into(), 0, 2));
}

fn assert_conflict<T>(result: &Result<T, HubStoreError>) {
    assert!(matches!(
        result,
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAgentNodeDispatchRequest,
            ..
        })
    ));
}

fn assert_corrupt<T>(result: &Result<T, HubStoreError>) {
    assert!(matches!(result, Err(HubStoreError::Corrupt { .. })));
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
