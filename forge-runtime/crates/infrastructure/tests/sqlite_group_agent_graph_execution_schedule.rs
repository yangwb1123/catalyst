#[allow(dead_code)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[allow(dead_code)]
mod sqlite_group_agent_graph_run_support;
#[allow(dead_code)]
mod sqlite_group_agent_node_execution_contract_support;

use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    AdmitGroupAgentGraphExecutionScheduleDisposition, GroupAgentGraphExecutionScheduleStore,
    GroupAgentGraphRunStore, GroupAgentNodeExecutionContractStore, HubEntity, HubStoreError,
};

use sqlite_group_agent_graph_execution_schedule_support::{
    prepared_fixture, recanonicalize, request, request_for_run,
};

#[test]
fn admission_replays_and_preserves_the_graph_run_and_journal() {
    let fixture = prepared_fixture();
    let before = fixture
        .store
        .inspect_group_agent_graph_run("graph-run-1")
        .expect("inspect source before schedule");
    let candidate = request(&fixture, "schedule-key", 40);
    let created = fixture
        .store
        .admit_group_agent_graph_execution_schedule(&candidate)
        .expect("admit execution schedule");
    assert_eq!(
        created.disposition,
        AdmitGroupAgentGraphExecutionScheduleDisposition::Created
    );
    assert_eq!(created.inspection.schedule, candidate.schedule);

    let replay = request(&fixture, "schedule-key", 999);
    let replayed = fixture
        .store
        .admit_group_agent_graph_execution_schedule(&replay)
        .expect("replay ignores candidate admission time");
    assert_eq!(
        replayed.disposition,
        AdmitGroupAgentGraphExecutionScheduleDisposition::Replayed
    );
    assert_eq!(replayed.inspection, created.inspection);
    assert_reads(&fixture, &created.inspection);
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 1);
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_graph_run("graph-run-1")
            .expect("inspect unchanged source"),
        before
    );
    assert_passive_columns(&fixture);
}

#[test]
fn same_key_replay_rejects_an_advanced_head_while_show_remains_readable() {
    let fixture = prepared_fixture();
    let candidate = request(&fixture, "schedule-key", 40);
    let contract =
        sqlite_group_agent_node_execution_contract_support::request(&fixture, "contract-key", 41);
    let created = fixture
        .store
        .admit_group_agent_graph_execution_schedule(&candidate)
        .expect("seed execution schedule");
    fixture
        .store
        .admit_group_agent_node_execution_contract(&contract)
        .expect("advance Graph Run head");

    assert_schedule_conflict(
        &fixture
            .store
            .admit_group_agent_graph_execution_schedule(&candidate),
    );
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_graph_execution_schedule(&created.inspection.record.schedule_id)
            .expect("historical schedule remains readable"),
        created.inspection
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_execution_schedules"),
        1
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 2);
}

#[test]
fn reused_identity_or_key_with_different_input_conflicts_without_partial_rows() {
    let fixture = fixture_with_two_runs();
    let original = request(&fixture, "schedule-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&original)
        .expect("seed execution schedule");

    let second_key = request(&fixture, "second-key", 41);
    assert_schedule_conflict(
        &fixture
            .store
            .admit_group_agent_graph_execution_schedule(&second_key),
    );
    let divergent = request_for_run(&fixture, "graph-run-2", "schedule-key", 42);
    assert_schedule_conflict(
        &fixture
            .store
            .admit_group_agent_graph_execution_schedule(&divergent),
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_execution_schedules"),
        1
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 2);
}

#[test]
fn stored_or_source_corruption_wins_over_divergent_same_key_conflict() {
    let corruptions = [
        "UPDATE group_agent_graph_execution_schedules
         SET schedule_blob=zeroblob(length(schedule_blob))",
        "UPDATE group_agent_graph_execution_schedules SET schedule_sha256=zeroblob(32)",
        "UPDATE group_agent_graph_execution_schedules SET graph_run_id='missing-run'",
        "UPDATE group_agent_graph_run_events SET event_sha256=zeroblob(32)
         WHERE graph_run_id='graph-run-1' AND seq=1",
        "UPDATE group_agent_graphs SET manifest_blob=zeroblob(length(manifest_blob))",
    ];
    for sql in corruptions {
        let fixture = fixture_with_two_runs();
        let original = request(&fixture, "shared-key", 40);
        let divergent = request_for_run(&fixture, "graph-run-2", "shared-key", 41);
        fixture
            .store
            .admit_group_agent_graph_execution_schedule(&original)
            .expect("seed execution schedule");
        let connection = fixture.connection();
        connection
            .pragma_update(None, "foreign_keys", false)
            .expect("disable fixture foreign keys");
        connection
            .execute_batch(sql)
            .expect("inject stored corruption");
        assert_corrupt(
            &fixture
                .store
                .admit_group_agent_graph_execution_schedule(&divergent),
        );
    }
}

#[test]
fn metadata_list_never_decodes_schedule_content_but_validates_its_inputs() {
    let fixture = prepared_fixture();
    let candidate = request(&fixture, "schedule-key", 40);
    fixture
        .store
        .admit_group_agent_graph_execution_schedule(&candidate)
        .expect("seed execution schedule");
    fixture
        .connection()
        .execute_batch(
            "UPDATE group_agent_graph_execution_schedules
             SET schedule_blob=zeroblob(length(schedule_blob))",
        )
        .expect("corrupt schedule content only");

    let listed = fixture
        .store
        .list_group_agent_graph_execution_schedules(Some("graph-run-1"), 10)
        .expect("metadata list avoids schedule content");
    assert_eq!(listed.len(), 1);
    assert_corrupt(
        &fixture
            .store
            .inspect_group_agent_graph_execution_schedule(&candidate.schedule.schedule_id),
    );

    let empty = sqlite_group_agent_graph_run_support::Fixture::new();
    assert!(matches!(
        empty
            .store
            .list_group_agent_graph_execution_schedules(Some("missing"), 10),
        Err(HubStoreError::NotFound {
            entity: HubEntity::GroupAgentGraphRun,
            ..
        })
    ));
    for limit in [0, 101] {
        assert_schedule_conflict(
            &empty
                .store
                .list_group_agent_graph_execution_schedules(None, limit),
        );
    }
}

#[test]
fn stale_run_and_orphan_children_fail_closed() {
    let stale = prepared_fixture();
    let contract =
        sqlite_group_agent_node_execution_contract_support::request(&stale, "contract-key", 40);
    stale
        .store
        .admit_group_agent_node_execution_contract(&contract)
        .expect("advance Graph Run beyond v1");
    let candidate = request(&stale, "schedule-key", 50);
    assert_schedule_conflict(
        &stale
            .store
            .admit_group_agent_graph_execution_schedule(&candidate),
    );
    assert_eq!(stale.row_count("group_agent_graph_execution_schedules"), 0);

    let orphan = prepared_fixture();
    let candidate = request(&orphan, "schedule-key", 40);
    orphan
        .store
        .admit_group_agent_graph_execution_schedule(&candidate)
        .expect("seed schedule child");
    let connection = orphan.connection();
    connection
        .pragma_update(None, "foreign_keys", false)
        .expect("disable fixture foreign keys");
    connection
        .execute(
            "UPDATE group_agent_graph_execution_schedules
             SET graph_run_id='missing-run' WHERE id=?1",
            [&candidate.schedule.schedule_id],
        )
        .expect("orphan schedule child");
    assert_corrupt(&orphan.store.inspect_group_agent_graph_run("missing-run"));
    assert_corrupt(
        &orphan
            .store
            .inspect_group_agent_graph_execution_schedule(&candidate.schedule.schedule_id),
    );
}

#[test]
fn extra_stored_policy_columns_are_bound_to_the_exact_artifact() {
    for sql in [
        "UPDATE group_agent_graph_execution_schedules SET scheduler_protocol_version=2",
        "UPDATE group_agent_graph_execution_schedules SET initial_node='backend'",
        "UPDATE group_agent_graph_execution_schedules SET progress_observed=1",
        "UPDATE group_agent_graph_execution_schedules SET successor_advanced=1",
    ] {
        let fixture = prepared_fixture();
        let candidate = request(&fixture, "schedule-key", 40);
        fixture
            .store
            .admit_group_agent_graph_execution_schedule(&candidate)
            .expect("seed execution schedule");
        let connection = fixture.connection();
        connection
            .pragma_update(None, "ignore_check_constraints", true)
            .expect("allow corruption fixture");
        connection.execute_batch(sql).expect("corrupt extra column");
        assert_corrupt(
            &fixture
                .store
                .inspect_group_agent_graph_execution_schedule(&candidate.schedule.schedule_id),
        );
    }
}

#[test]
fn concurrent_same_key_creates_once_and_replays_one_exact_identity() {
    const WORKERS: usize = 6;
    let fixture = prepared_fixture();
    let barrier = Arc::new(Barrier::new(WORKERS));
    let workers = (0..WORKERS)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = Arc::clone(&barrier);
            let candidate = request(
                &fixture,
                "shared-schedule-key",
                50 + u64::try_from(index).expect("worker fits"),
            );
            std::thread::spawn(move || {
                barrier.wait();
                store
                    .admit_group_agent_graph_execution_schedule(&candidate)
                    .expect("concurrent schedule admission")
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("schedule worker"))
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                result.disposition == AdmitGroupAgentGraphExecutionScheduleDisposition::Created
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

#[test]
fn concurrent_divergent_same_key_has_one_winner_and_one_conflict() {
    let fixture = fixture_with_two_runs();
    let barrier = Arc::new(Barrier::new(2));
    let candidates = [
        request_for_run(&fixture, "graph-run-1", "racing-key", 50),
        request_for_run(&fixture, "graph-run-2", "racing-key", 51),
    ];
    let workers = candidates.map(|candidate| {
        let store = fixture.store.clone();
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            barrier.wait();
            store.admit_group_agent_graph_execution_schedule(&candidate)
        })
    });
    let results = workers.map(|worker| worker.join().expect("schedule race worker"));
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Ok(created) if created.disposition
                == AdmitGroupAgentGraphExecutionScheduleDisposition::Created))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(HubStoreError::Conflict { .. })))
            .count(),
        1
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_execution_schedules"),
        1
    );
}

#[test]
fn invalid_candidate_is_rejected_before_any_side_effect() {
    let fixture = prepared_fixture();
    let mut candidate = request(&fixture, "schedule-key", 40);
    candidate.schedule.initial_node = "backend".into();
    recanonicalize(&mut candidate);
    assert_schedule_conflict(
        &fixture
            .store
            .admit_group_agent_graph_execution_schedule(&candidate),
    );
    assert_eq!(
        fixture.row_count("group_agent_graph_execution_schedules"),
        0
    );
    assert_eq!(fixture.row_count("group_agent_graph_run_events"), 1);
}

fn fixture_with_two_runs() -> sqlite_group_agent_graph_run_support::Fixture {
    let fixture = prepared_fixture();
    fixture
        .store
        .begin_group_agent_graph_run(&fixture.request("graph-run-2", "run-key-2", 31))
        .expect("seed second Graph Run");
    fixture
}

fn assert_reads(
    fixture: &sqlite_group_agent_graph_run_support::Fixture,
    expected: &forge_runtime_domain::GroupAgentGraphExecutionScheduleInspection,
) {
    assert_eq!(
        fixture
            .store
            .inspect_group_agent_graph_execution_schedule(&expected.record.schedule_id)
            .expect("inspect execution schedule"),
        *expected
    );
    let listed = fixture
        .store
        .list_group_agent_graph_execution_schedules(Some("graph-run-1"), 10)
        .expect("list execution schedules");
    assert_eq!(listed.as_slice(), std::slice::from_ref(&expected.record));
}

fn assert_passive_columns(fixture: &sqlite_group_agent_graph_run_support::Fixture) {
    let row: (i64, i64, i64, i64) = fixture
        .connection()
        .query_row(
            "SELECT execution_contract_present,dispatch_authority_released,
                    progress_observed,successor_advanced
             FROM group_agent_graph_execution_schedules",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("passive schedule columns");
    assert_eq!(row, (0, 0, 0, 0));
}

fn assert_schedule_conflict<T>(result: &Result<T, HubStoreError>) {
    assert!(
        matches!(
            result,
            Err(HubStoreError::Conflict {
                entity: HubEntity::GroupAgentGraphExecutionSchedule,
                ..
            })
        ),
        "expected execution schedule conflict"
    );
}

fn assert_corrupt<T>(result: &Result<T, HubStoreError>) {
    assert!(
        matches!(result, Err(HubStoreError::Corrupt { .. })),
        "expected corruption"
    );
}
