#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
mod sqlite_group_agent_node_execution_contract_support;

use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    AdmitGroupAgentNodeExecutionContractDisposition, GroupAgentGraphRunStatus,
    GroupAgentGraphRunStore, GroupAgentNodeExecutionContractStore, HubEntity, HubStoreError,
};

use sqlite_group_agent_node_execution_contract_support::{
    prepared_fixture, recanonicalize, request,
};

#[test]
fn admission_replays_original_and_reads_full_and_metadata_views() {
    let fixture = prepared_fixture();
    let candidate = request(&fixture, "contract-key", 40);
    let created = fixture
        .store
        .admit_group_agent_node_execution_contract(&candidate)
        .expect("admit contract");
    assert_eq!(
        created.disposition,
        AdmitGroupAgentNodeExecutionContractDisposition::Created
    );
    assert_eq!(created.inspection.contract, candidate.contract);
    assert_eq!(created.inspection.admission_event, candidate.event);

    let replay = request(&fixture, "contract-key", 999);
    let replayed = fixture
        .store
        .admit_group_agent_node_execution_contract(&replay)
        .expect("replay ignores candidate time");
    assert_eq!(
        replayed.disposition,
        AdmitGroupAgentNodeExecutionContractDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_reads(&fixture, &created.inspection);
    assert_transitioned_row(&fixture, &candidate);
}

#[test]
fn divergent_key_identity_snapshot_and_run_inputs_leave_no_partial_rows() {
    let fixture = prepared_fixture();
    let original = request(&fixture, "contract-key", 40);
    fixture
        .store
        .admit_group_agent_node_execution_contract(&original)
        .expect("seed contract");

    let mut divergent = request(&fixture, "contract-key", 41);
    divergent.contract.provider.model = "other-model".into();
    recanonicalize(&mut divergent);
    assert_contract_conflict(
        &fixture
            .store
            .admit_group_agent_node_execution_contract(&divergent),
    );

    let second_key = request(&fixture, "second-key", 42);
    assert_contract_conflict(
        &fixture
            .store
            .admit_group_agent_node_execution_contract(&second_key),
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_node_execution_contracts"),
        1
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 2);
}

#[test]
fn stale_but_self_consistent_control_snapshot_is_rejected_without_transition() {
    let fixture = prepared_fixture();
    let mut candidate = request(&fixture, "contract-key", 40);
    candidate.control_snapshot.last_event_sha256 = "a".repeat(64);
    recanonicalize(&mut candidate);
    assert_contract_conflict(
        &fixture
            .store
            .admit_group_agent_node_execution_contract(&candidate),
    );
    assert_base_row(&fixture);
}

#[test]
fn stored_corruption_wins_over_divergent_same_key_conflict() {
    for sql in [
        "UPDATE group_agent_graph_node_execution_contracts
         SET contract_blob=zeroblob(length(contract_blob))",
        "UPDATE group_agent_graph_node_execution_contracts SET contract_sha256=zeroblob(32)",
        "UPDATE group_agent_graph_run_events SET event_sha256=zeroblob(32) WHERE seq=2",
        "UPDATE group_agent_graphs SET manifest_blob=zeroblob(length(manifest_blob))",
        "UPDATE group_runs SET snapshot_sha256=zeroblob(32) WHERE id='group-run-1'",
    ] {
        let fixture = prepared_fixture();
        let original = request(&fixture, "contract-key", 40);
        fixture
            .store
            .admit_group_agent_node_execution_contract(&original)
            .expect("seed contract");
        fixture
            .connection()
            .execute_batch(sql)
            .expect("inject stored corruption");
        let mut divergent = original.clone();
        divergent.contract.provider.model = "other-model".into();
        recanonicalize(&mut divergent);
        assert_corrupt(
            &fixture
                .store
                .admit_group_agent_node_execution_contract(&divergent),
        );
    }
}

#[test]
fn metadata_list_is_content_free_but_rejects_bad_metadata_and_filters() {
    let fixture = prepared_fixture();
    let candidate = request(&fixture, "contract-key", 40);
    fixture
        .store
        .admit_group_agent_node_execution_contract(&candidate)
        .expect("seed contract");
    fixture
        .connection()
        .execute_batch(
            "UPDATE group_agent_graph_node_execution_contracts
             SET contract_blob=zeroblob(length(contract_blob))",
        )
        .expect("corrupt only contract content");
    let listed = fixture
        .store
        .list_group_agent_node_execution_contracts(Some("graph-run-1"), 10)
        .expect("metadata list avoids contract content");
    assert_eq!(listed.len(), 1);
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_node_execution_contract(&candidate.contract.contract_id),
    );

    let empty = sqlite_group_agent_graph_run_support::Fixture::new();
    assert!(matches!(
        empty
            .store
            .list_group_agent_node_execution_contracts(Some("missing"), 10),
        Err(HubStoreError::NotFound {
            entity: HubEntity::GroupAgentGraphRun,
            ..
        })
    ));
    for limit in [0, 101] {
        assert_contract_conflict(
            &empty
                .store
                .list_group_agent_node_execution_contracts(None, limit),
        );
    }
}

#[test]
fn post_transition_reread_failure_rolls_back_contract_event_and_run() {
    let fixture = prepared_fixture();
    let candidate = request(&fixture, "contract-key", 40);
    fixture
        .connection()
        .execute_batch(
            "CREATE TRIGGER mutate_run_after_contract_event
             AFTER INSERT ON group_agent_graph_run_events WHEN NEW.seq=2
             BEGIN
               UPDATE group_agent_graph_runs SET node_count=3 WHERE id=NEW.graph_run_id;
             END;",
        )
        .expect("install late corruption trigger");
    assert_corrupt(
        &fixture
            .store
            .admit_group_agent_node_execution_contract(&candidate),
    );
    assert_base_row(&fixture);
}

#[test]
fn concurrent_same_key_creates_once_and_every_result_has_one_identity() {
    const WORKERS: usize = 6;
    let fixture = prepared_fixture();
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            let candidate = request(
                &fixture,
                "shared-contract-key",
                50 + u64::try_from(index).expect("worker fits"),
            );
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .admit_group_agent_node_execution_contract(&candidate)
                    .expect("concurrent admission")
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("admission worker"))
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                result.disposition == AdmitGroupAgentNodeExecutionContractDisposition::Created
            })
            .count(),
        1
    );
    assert!(
        results
            .iter()
            .all(|result| result.inspection == results[0].inspection)
    );
}

fn assert_reads(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    expected: &forge_runtime_domain::GroupAgentNodeExecutionContractInspection,
) {
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_node_execution_contract(&expected.record.contract_id)
            .expect("inspect contract"),
        *expected
    );
    let listed = fixture
        .store
        .list_group_agent_node_execution_contracts(Some("graph-run-1"), 10)
        .expect("list contracts");
    assert_eq!(listed.as_slice(), std::slice::from_ref(&expected.record));
}

fn assert_transitioned_row(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    request: &forge_runtime_domain::AdmitGroupAgentNodeExecutionContract,
) {
    let run = fixture
        .store
        .inspect_group_agent_graph_run("graph-run-1")
        .expect("inspect transitioned run");
    assert_eq!(run.run.v, 2);
    assert_eq!(
        run.run.status,
        GroupAgentGraphRunStatus::AwaitingCoreDispatch
    );
    assert!(run.run.execution_contract_present);
    assert!(!run.run.dispatch_authority_released);
    assert_eq!(run.run.last_event_seq, 2);
    assert_eq!(run.events[1], request.event);
}

fn assert_base_row(fixture: &sqlite_group_agent_graph_run_support::Fixture) {
    assert_eq!(
        fixture.row_count("group_agent_graph_node_execution_contracts"),
        0
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 1);
    let row: (i64, String, i64, i64) = fixture
        .connection()
        .query_row(
            "SELECT run_version,status,execution_contract_present,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("base Graph Run row");
    assert_eq!(row, (1, "awaiting_execution_contract".into(), 0, 1));
}

fn assert_contract_conflict<T>(result: &Result<T, HubStoreError>) {
    assert!(
        matches!(
            result,
            Err(HubStoreError::Conflict {
                entity: HubEntity::GroupAgentNodeExecutionContract,
                ..
            })
        ),
        "expected Node Execution Contract conflict"
    );
}

fn assert_corrupt<T>(result: &Result<T, HubStoreError>) {
    assert!(
        matches!(result, Err(HubStoreError::Corrupt { .. })),
        "expected corruption"
    );
}
