#[allow(dead_code)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;
#[allow(dead_code)]
mod sqlite_group_agent_scheduled_node_contract_support;

use forge_runtime_domain::{
    AdmitGroupAgentScheduledNodeContractDisposition, GroupAgentGraphRunStore,
    GroupAgentNodeExecutionContractStore, GroupAgentScheduledNodeContractStore, HubStoreError,
};
use std::sync::{Arc, Barrier};

use sqlite_group_agent_scheduled_node_contract_support::prepared_fixture;

#[test]
fn admission_replays_exactly_and_preserves_run_and_journal() {
    let (fixture, request) = prepared_fixture();
    let before = fixture
        .store
        .inspect_group_agent_graph_run("graph-run-1")
        .expect("inspect candidate source");
    let created = fixture
        .store
        .admit_group_agent_scheduled_node_contract(&request)
        .expect("admit scheduled-node candidate");
    assert_eq!(
        created.disposition,
        AdmitGroupAgentScheduledNodeContractDisposition::Created
    );
    assert_eq!(created.inspection.candidate, request.candidate);

    let mut replay = request.clone();
    replay.admitted_at_ms = 999;
    let replayed = fixture
        .store
        .admit_group_agent_scheduled_node_contract(&replay)
        .expect("exact replay ignores admission time");
    assert_eq!(
        replayed.disposition,
        AdmitGroupAgentScheduledNodeContractDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_scheduled_node_contract(&request.candidate.contract_id)
            .expect("show admitted candidate"),
        created.inspection
    );
    assert_eq!(
        fixture
            .store
            .list_group_agent_scheduled_node_contracts(Some("graph-run-1"), 10)
            .expect("list candidate metadata"),
        vec![created.inspection.record]
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 1);
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_graph_run("graph-run-1")
            .expect("inspect unchanged candidate source"),
        before
    );
}

#[test]
fn all_candidate_identity_lookups_are_valid_and_a_different_key_conflicts() {
    let (fixture, request) = prepared_fixture();
    fixture
        .store
        .admit_group_agent_scheduled_node_contract(&request)
        .expect("seed scheduled-node candidate");
    let mut divergent_key = request;
    divergent_key.idempotency_key = "other-scheduled-contract-key".into();

    assert_conflict(
        &fixture
            .store
            .admit_group_agent_scheduled_node_contract(&divergent_key),
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_scheduled_node_contract_candidates"),
        1
    );
}

#[test]
fn same_key_with_a_different_valid_candidate_conflicts() {
    let (fixture, request) = prepared_fixture();
    fixture
        .store
        .admit_group_agent_scheduled_node_contract(&request)
        .expect("seed scheduled-node candidate");
    let mut divergent = request;
    divergent.candidate.provider.model = "different-private-model".into();
    let digest = divergent
        .candidate
        .expected_sha256()
        .expect("divergent candidate digest");
    divergent.candidate.contract_id = format!("scheduled-node-contract-{digest}");
    divergent.candidate.contract_sha256 = digest;
    divergent.candidate_json = divergent
        .candidate
        .canonical_json()
        .expect("divergent canonical candidate");
    divergent
        .validate()
        .expect("divergent candidate remains valid");

    assert_conflict(
        &fixture
            .store
            .admit_group_agent_scheduled_node_contract(&divergent),
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_scheduled_node_contract_candidates"),
        1
    );
}

#[test]
fn concurrent_same_v2_candidate_creates_once_and_replays_one_exact_row() {
    let (fixture, request) = prepared_fixture();
    let barrier = Arc::new(Barrier::new(2));
    let first_store = fixture.store.clone();
    let first_barrier = Arc::clone(&barrier);
    let first_request = request.clone();
    let first = std::thread::spawn(move || {
        first_barrier.wait();
        first_store.admit_group_agent_scheduled_node_contract(&first_request)
    });
    let second_store = fixture.store.clone();
    let second = std::thread::spawn(move || {
        barrier.wait();
        second_store.admit_group_agent_scheduled_node_contract(&request)
    });

    let results = [
        first
            .join()
            .expect("first candidate worker")
            .expect("first admission"),
        second
            .join()
            .expect("second candidate worker")
            .expect("second admission"),
    ];
    let created = results
        .iter()
        .filter(|result| {
            result.disposition == AdmitGroupAgentScheduledNodeContractDisposition::Created
        })
        .count();
    let replayed = results
        .iter()
        .filter(|result| {
            result.disposition == AdmitGroupAgentScheduledNodeContractDisposition::Replayed
        })
        .count();
    assert_eq!((created, replayed), (1, 1));
    assert_eq!(results[0].inspection, results[1].inspection);
    assert_eq!(
        fixture.row_count("group_agent_graph_scheduled_node_contract_candidates"),
        1
    );
}

#[test]
fn scheduled_v2_and_legacy_v1_contract_families_are_mutually_exclusive() {
    let (candidate_first, candidate) = prepared_fixture();
    candidate_first
        .store
        .admit_group_agent_scheduled_node_contract(&candidate)
        .expect("seed scheduled v2 family");
    let legacy = sqlite_group_agent_node_execution_contract_support::request(
        &candidate_first,
        "legacy-after-v2",
        60,
    );
    assert_conflict(
        &candidate_first
            .store
            .admit_group_agent_node_execution_contract(&legacy),
    );

    let (legacy_first, candidate) = prepared_fixture();
    let legacy = sqlite_group_agent_node_execution_contract_support::request(
        &legacy_first,
        "legacy-before-v2",
        60,
    );
    legacy_first
        .store
        .admit_group_agent_node_execution_contract(&legacy)
        .expect("seed legacy v1 family");
    assert_conflict(
        &legacy_first
            .store
            .admit_group_agent_scheduled_node_contract(&candidate),
    );
    assert_eq!(
        legacy_first.row_count("group_agent_graph_scheduled_node_contract_candidates"),
        0
    );
}

#[test]
fn pristine_head_candidate_rejects_after_the_legacy_lifecycle_advances() {
    let (fixture, candidate) = prepared_fixture();
    assert_eq!(candidate.candidate.expected_last_event_seq, 1);
    let legacy = sqlite_group_agent_node_execution_contract_support::request(
        &fixture,
        "advance-before-stale-v2",
        60,
    );
    fixture
        .store
        .admit_group_agent_node_execution_contract(&legacy)
        .expect("advance legacy lifecycle head");
    let advanced = fixture
        .store
        .inspect_group_agent_graph_run("graph-run-1")
        .expect("inspect advanced Graph Run");
    assert_eq!(advanced.run.last_event_seq, 2);

    assert_conflict(
        &fixture
            .store
            .admit_group_agent_scheduled_node_contract(&candidate),
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_scheduled_node_contract_candidates"),
        0
    );
}

#[test]
fn concurrent_v1_and_v2_admission_has_exactly_one_contract_family_winner() {
    let (fixture, candidate) = prepared_fixture();
    let legacy = sqlite_group_agent_node_execution_contract_support::request(
        &fixture,
        "racing-legacy-key",
        60,
    );
    let barrier = Arc::new(Barrier::new(2));
    let candidate_store = fixture.store.clone();
    let candidate_barrier = Arc::clone(&barrier);
    let candidate_worker = std::thread::spawn(move || {
        candidate_barrier.wait();
        candidate_store.admit_group_agent_scheduled_node_contract(&candidate)
    });
    let legacy_store = fixture.store.clone();
    let legacy_worker = std::thread::spawn(move || {
        barrier.wait();
        legacy_store.admit_group_agent_node_execution_contract(&legacy)
    });

    let candidate_result = candidate_worker.join().expect("candidate worker");
    let legacy_result = legacy_worker.join().expect("legacy worker");
    assert_eq!(
        usize::from(candidate_result.is_ok()) + usize::from(legacy_result.is_ok()),
        1
    );
    if let Err(error) = candidate_result {
        assert!(matches!(error, HubStoreError::Conflict { .. }));
    }
    if let Err(error) = legacy_result {
        assert!(matches!(error, HubStoreError::Conflict { .. }));
    }
    assert_eq!(
        fixture.row_count("group_agent_graph_scheduled_node_contract_candidates")
            + fixture.row_count("group_agent_graph_node_execution_contracts"),
        1
    );
}

#[test]
fn stored_corruption_wins_while_metadata_list_remains_content_free() {
    let (fixture, request) = prepared_fixture();
    let legacy = sqlite_group_agent_node_execution_contract_support::request(
        &fixture,
        "legacy-corruption-probe",
        60,
    );
    fixture
        .store
        .admit_group_agent_scheduled_node_contract(&request)
        .expect("seed scheduled-node candidate");
    fixture
        .connection()
        .execute_batch(
            "UPDATE group_agent_graph_scheduled_node_contract_candidates
             SET contract_blob=zeroblob(length(contract_blob))",
        )
        .expect("corrupt candidate content");

    let records = fixture
        .store
        .list_group_agent_scheduled_node_contracts(Some("graph-run-1"), 10)
        .expect("metadata list does not decode Prompt content");
    assert_eq!(records.len(), 1);
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_scheduled_node_contract(&request.candidate.contract_id),
    );
    assert_corrupt(
        &fixture
            .store
            .admit_group_agent_scheduled_node_contract(&request),
    );
    assert_corrupt(
        &fixture
            .store
            .admit_group_agent_node_execution_contract(&legacy),
    );
}

fn assert_conflict<T>(result: &Result<T, HubStoreError>) {
    assert!(matches!(result, Err(HubStoreError::Conflict { .. })));
}

fn assert_corrupt<T>(result: &Result<T, HubStoreError>) {
    assert!(matches!(result, Err(HubStoreError::Corrupt { .. })));
}
