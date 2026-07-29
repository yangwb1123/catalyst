use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatchResult, CompleteGroupModelAnalysis,
    CompleteGroupModelAnalysisDisposition, GROUP_RUN_VERSION, GroupContextPolicy,
    GroupModelAnalysisRecovery, GroupModelAnalysisStore, GroupRunStore, HubEntity, HubStoreError,
    PrepareGroupModelAnalysisDisposition, PrepareGroupRun,
};
use rusqlite::Connection;

mod sqlite_group_model_analysis_support;

use sqlite_group_model_analysis_support::{
    Fixture, claim_request, core_table_counts, result_artifact,
};

#[test]
fn prepare_is_exact_atomic_and_semantically_idempotent() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate("analysis-1", "analysis-key", 20);
    let first = fixture
        .store
        .prepare_group_model_analysis(&candidate)
        .expect("prepare analysis");
    assert_eq!(
        first.disposition,
        PrepareGroupModelAnalysisDisposition::Created
    );
    assert_eq!(
        first.inspection.recovery,
        GroupModelAnalysisRecovery::AwaitingConsent
    );

    let replay = fixture
        .store
        .prepare_group_model_analysis(&fixture.candidate("ignored-id", "analysis-key", 999))
        .expect("exact semantic replay");
    assert_eq!(
        replay.disposition,
        PrepareGroupModelAnalysisDisposition::Replayed
    );
    assert_eq!(replay.inspection, first.inspection);
    assert_eq!(
        fixture
            .store
            .inspect_group_model_analysis("analysis-1")
            .expect("inspect prepared analysis"),
        first.inspection
    );
    let listed = fixture
        .store
        .list_group_model_analyses(Some("group-run-1"), 10)
        .expect("list prepared analysis");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0], first.inspection.analysis);
    assert_persisted_prepare(&fixture, &candidate.request_body);
}

#[test]
fn prepare_conflicts_are_fail_closed() {
    let fixture = Fixture::new();
    fixture.prepare("analysis-1", "analysis-key");
    let other_source = fixture
        .store
        .prepare_group_run(&PrepareGroupRun {
            v: GROUP_RUN_VERSION,
            run_id: "group-run-2".into(),
            group_id: fixture.snapshot.run.group_id.clone(),
            policy: GroupContextPolicy::default(),
            idempotency_key: "group-run-key-2".into(),
            created_at_ms: 11,
        })
        .expect("prepare divergent source")
        .snapshot;
    assert_eq!(other_source.context_json, fixture.snapshot.context_json);
    let mut divergent = fixture.candidate("ignored", "analysis-key", 21);
    divergent.source.group_run_version = other_source.run.v;
    divergent.source.group_run_id = other_source.run.run_id;
    divergent.source.group_id = other_source.run.group_id;
    divergent.source.context_version = other_source.run.context_version;
    divergent.source.context_slice_sha256 = other_source.run.context_slice_sha256;
    divergent.source.snapshot_sha256 = other_source.run.snapshot_sha256;
    divergent.source.snapshot_bytes = other_source.run.snapshot_bytes;
    assert_analysis_conflict(
        &fixture.store.prepare_group_model_analysis(&divergent),
        "valid divergent source replay",
    );
    assert_analysis_conflict(
        &fixture
            .store
            .prepare_group_model_analysis(&fixture.candidate("analysis-1", "new-key", 22)),
        "reused analysis ID",
    );
    assert_analysis_conflict(
        &fixture.store.list_group_model_analyses(None, 0),
        "zero list limit",
    );
    assert!(matches!(
        fixture.store.inspect_group_model_analysis("missing"),
        Err(HubStoreError::NotFound {
            entity: HubEntity::GroupModelAnalysis,
            ..
        })
    ));
}

#[test]
fn claim_releases_exact_bytes_only_once() {
    let fixture = Fixture::new();
    let candidate = fixture.candidate("analysis-1", "analysis-key", 20);
    let expected = candidate.request_body.clone();
    fixture
        .store
        .prepare_group_model_analysis(&candidate)
        .expect("prepare");
    let first = fixture
        .store
        .claim_group_model_analysis_dispatch(&claim_request("analysis-1", "dispatch-1", 30))
        .expect("claim");
    let body = match first {
        ClaimGroupModelAnalysisDispatchResult::Claimed { authority } => authority.into_parts().1,
        other @ ClaimGroupModelAnalysisDispatchResult::AlreadyClaimed { .. } => {
            panic!("expected exclusive claim, got {other:?}")
        }
    };
    assert_eq!(body, expected);

    let second = fixture
        .store
        .claim_group_model_analysis_dispatch(&claim_request("analysis-1", "dispatch-2", 31))
        .expect("inspect already claimed");
    assert!(matches!(
        second,
        ClaimGroupModelAnalysisDispatchResult::AlreadyClaimed { .. }
    ));
    assert_dispatch_state(&fixture);
}

#[test]
fn concurrent_claim_has_exactly_one_authority() {
    const WORKERS: usize = 8;
    let fixture = Fixture::new();
    let candidate = fixture.candidate("analysis-1", "analysis-key", 20);
    let request_body = candidate.request_body.clone();
    fixture
        .store
        .prepare_group_model_analysis(&candidate)
        .expect("prepare concurrent analysis");
    let barrier = Arc::new(Barrier::new(WORKERS));
    let mut workers = Vec::new();
    for index in 0..WORKERS {
        let store = fixture.store.clone();
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store
                .claim_group_model_analysis_dispatch(&claim_request(
                    "analysis-1",
                    &format!("dispatch-{index}"),
                    30 + index as u64,
                ))
                .expect("concurrent claim")
        }));
    }
    let outcomes = workers
        .into_iter()
        .map(|worker| worker.join().expect("claim worker"))
        .collect::<Vec<_>>();
    assert_concurrent_claim_outcomes(&outcomes, &request_body);
    assert_dispatch_state(&fixture);
}

fn assert_concurrent_claim_outcomes(
    outcomes: &[ClaimGroupModelAnalysisDispatchResult],
    request_body: &[u8],
) {
    let claimed = outcomes
        .iter()
        .filter(|outcome| {
            matches!(
                outcome,
                ClaimGroupModelAnalysisDispatchResult::Claimed { .. }
            )
        })
        .count();
    let losers = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            ClaimGroupModelAnalysisDispatchResult::AlreadyClaimed { inspection } => {
                Some(inspection)
            }
            ClaimGroupModelAnalysisDispatchResult::Claimed { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(claimed, 1);
    assert_eq!(losers.len(), outcomes.len() - 1);
    assert!(losers.iter().all(|inspection| {
        let encoded = serde_json::to_vec(inspection).expect("loser inspection JSON");
        inspection.events.len() == 2
            && inspection.dispatch.is_some()
            && inspection.result.is_none()
            && !encoded
                .windows(request_body.len())
                .any(|window| window == request_body)
    }));
}

#[test]
fn completion_is_atomic_and_exactly_replayable() {
    let fixture = Fixture::new();
    let core_counts = core_table_counts(&fixture);
    let dispatched = dispatch(&fixture);
    let artifact = result_artifact(&dispatched, "Cross-project risks are bounded.", 40);
    let request = CompleteGroupModelAnalysis {
        v: 1,
        artifact: artifact.clone(),
    };
    let first = fixture
        .store
        .complete_group_model_analysis(&request)
        .expect("complete analysis");
    assert_eq!(
        first.disposition,
        CompleteGroupModelAnalysisDisposition::Created
    );
    assert_eq!(first.inspection.result, Some(artifact.clone()));
    let replay = fixture
        .store
        .complete_group_model_analysis(&request)
        .expect("exact completion replay");
    assert_eq!(
        replay.disposition,
        CompleteGroupModelAnalysisDisposition::Replayed
    );

    let divergent = CompleteGroupModelAnalysis {
        v: 1,
        artifact: result_artifact(&dispatched, "A different valid answer.", 40),
    };
    assert_analysis_conflict(
        &fixture.store.complete_group_model_analysis(&divergent),
        "valid divergent completion",
    );
    assert_completed_rows(&fixture);
    assert_eq!(
        core_table_counts(&fixture),
        core_counts,
        "analysis workflow mutated a non-analysis core table"
    );
}

#[test]
fn prepare_and_complete_roll_back_on_late_event_failure() {
    let fixture = Fixture::new();
    install_event_abort(&fixture, 1);
    assert!(
        fixture
            .store
            .prepare_group_model_analysis(&fixture.candidate("analysis-fail", "fail-key", 20))
            .is_err()
    );
    assert_row_count(&fixture, "group_model_analyses", "analysis-fail", 0);
    assert_row_count(&fixture, "group_model_analysis_events", "analysis-fail", 0);
    remove_event_abort(&fixture);

    let dispatched = dispatch(&fixture);
    install_event_abort(&fixture, 3);
    let artifact = result_artifact(&dispatched, "will roll back", 40);
    assert!(
        fixture
            .store
            .complete_group_model_analysis(&CompleteGroupModelAnalysis { v: 1, artifact })
            .is_err()
    );
    let inspection = fixture
        .store
        .inspect_group_model_analysis("analysis-1")
        .expect("dispatch state remains valid");
    assert!(matches!(
        inspection.recovery,
        GroupModelAnalysisRecovery::DispatchUnknown { .. }
    ));
    assert_row_count(&fixture, "group_model_analysis_events", "analysis-1", 2);
    assert_row_count(&fixture, "group_model_analysis_results", "analysis-1", 0);
}

#[test]
fn full_inspection_rejects_corrupt_configuration_request_and_journal() {
    for sql in [
        "UPDATE group_model_analyses SET config_json='{}' WHERE id='analysis-1'",
        "UPDATE group_model_analyses SET request_body=zeroblob(length(request_body))
         WHERE id='analysis-1'",
        "UPDATE group_model_analysis_events SET event_sha256=zeroblob(32)
         WHERE analysis_id='analysis-1' AND seq=1",
        "UPDATE group_model_analysis_events SET event_json='{}'
         WHERE analysis_id='analysis-1' AND seq=1",
        "UPDATE group_model_analyses SET cursor_json='{}' WHERE id='analysis-1'",
        "UPDATE group_model_analyses SET journal_bytes=journal_bytes+1
         WHERE id='analysis-1'",
        "DELETE FROM group_model_analysis_events WHERE analysis_id='analysis-1'",
        "UPDATE group_model_analyses SET status='dispatch_unknown' WHERE id='analysis-1'",
        "UPDATE group_runs SET snapshot_sha256=zeroblob(32) WHERE id='group-run-1'",
    ] {
        assert_prepared_corruption(sql);
    }
}

#[test]
fn full_inspection_rejects_missing_or_corrupt_result() {
    for sql in [
        "DELETE FROM group_model_analysis_results WHERE analysis_id='analysis-1'",
        "UPDATE group_model_analysis_results SET result_blob=zeroblob(length(result_blob))
         WHERE analysis_id='analysis-1'",
        "UPDATE group_model_analysis_results SET result_sha256=zeroblob(32)
         WHERE analysis_id='analysis-1'",
        "UPDATE group_model_analysis_results SET result_bytes=result_bytes-1
         WHERE analysis_id='analysis-1'",
    ] {
        assert_completed_corruption(sql);
    }
}

#[test]
fn list_is_metadata_only_even_when_private_bytes_are_corrupt() {
    let fixture = Fixture::new();
    fixture.prepare("analysis-1", "analysis-key");
    execute_raw(
        &fixture,
        "UPDATE group_model_analyses
         SET config_json='{}',request_body=zeroblob(length(request_body))
         WHERE id='analysis-1'",
    );
    let listed = fixture
        .store
        .list_group_model_analyses(Some("group-run-1"), 10)
        .expect("metadata-only list");
    assert_eq!(listed.len(), 1);
    assert!(matches!(
        fixture.store.inspect_group_model_analysis("analysis-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

fn dispatch(fixture: &Fixture) -> forge_runtime_domain::GroupModelAnalysisInspection {
    fixture.prepare("analysis-1", "analysis-key");
    let claimed = fixture
        .store
        .claim_group_model_analysis_dispatch(&claim_request("analysis-1", "dispatch-1", 30))
        .expect("claim dispatch");
    assert!(matches!(
        claimed,
        ClaimGroupModelAnalysisDispatchResult::Claimed { .. }
    ));
    fixture
        .store
        .inspect_group_model_analysis("analysis-1")
        .expect("inspect dispatched")
}

fn assert_persisted_prepare(fixture: &Fixture, expected_body: &[u8]) {
    let connection = Connection::open(&fixture.database).expect("raw SQLite");
    let row: (Vec<u8>, i64, i64, String) = connection
        .query_row(
            "SELECT request_body,request_bytes,journal_bytes,status
             FROM group_model_analyses WHERE id='analysis-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("prepared row");
    assert_eq!(row.0, expected_body);
    assert_eq!(
        usize::try_from(row.1).expect("stored request byte count"),
        expected_body.len()
    );
    assert!(row.2 > 0);
    assert_eq!(row.3, "awaiting_consent");
    assert_row_count(fixture, "group_model_analysis_events", "analysis-1", 1);
}

fn assert_dispatch_state(fixture: &Fixture) {
    let inspection = fixture
        .store
        .inspect_group_model_analysis("analysis-1")
        .expect("inspect dispatch");
    assert!(matches!(
        inspection.recovery,
        GroupModelAnalysisRecovery::DispatchUnknown { .. }
    ));
    assert_eq!(inspection.events.len(), 2);
}

fn assert_completed_rows(fixture: &Fixture) {
    let inspection = fixture
        .store
        .inspect_group_model_analysis("analysis-1")
        .expect("inspect completed");
    assert!(matches!(
        inspection.recovery,
        GroupModelAnalysisRecovery::Terminal { .. }
    ));
    assert_eq!(inspection.events.len(), 3);
    assert_eq!(
        inspection.analysis.status,
        forge_runtime_domain::GroupModelAnalysisStatus::Completed
    );
    assert_row_count(fixture, "group_model_analysis_events", "analysis-1", 3);
    assert_row_count(fixture, "group_model_analysis_results", "analysis-1", 1);
}

fn assert_prepared_corruption(sql: &str) {
    let fixture = Fixture::new();
    fixture.prepare("analysis-1", "analysis-key");
    execute_raw(&fixture, sql);
    assert!(matches!(
        fixture.store.inspect_group_model_analysis("analysis-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

fn assert_completed_corruption(sql: &str) {
    let fixture = Fixture::new();
    let dispatched = dispatch(&fixture);
    let artifact = result_artifact(&dispatched, "validated answer", 40);
    fixture
        .store
        .complete_group_model_analysis(&CompleteGroupModelAnalysis { v: 1, artifact })
        .expect("complete");
    execute_raw(&fixture, sql);
    assert!(matches!(
        fixture.store.inspect_group_model_analysis("analysis-1"),
        Err(HubStoreError::Corrupt { .. })
    ));
}

fn install_event_abort(fixture: &Fixture, sequence: u64) {
    execute_raw(
        fixture,
        &format!(
            "CREATE TRIGGER fail_analysis_event BEFORE INSERT ON group_model_analysis_events
             WHEN NEW.seq={sequence} BEGIN SELECT RAISE(ABORT,'injected failure'); END"
        ),
    );
}

fn remove_event_abort(fixture: &Fixture) {
    execute_raw(fixture, "DROP TRIGGER fail_analysis_event");
}

fn execute_raw(fixture: &Fixture, sql: &str) {
    let connection = Connection::open(&fixture.database).expect("raw SQLite");
    connection
        .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")
        .expect("enable corruption fixture");
    connection.execute_batch(sql).expect("install corruption");
}

fn assert_row_count(fixture: &Fixture, table: &str, id: &str, expected: i64) {
    let connection = Connection::open(&fixture.database).expect("raw SQLite");
    let column = if table == "group_model_analyses" {
        "id"
    } else {
        "analysis_id"
    };
    let count: i64 = connection
        .query_row(
            &format!("SELECT COUNT(*) FROM {table} WHERE {column}=?1"),
            [id],
            |row| row.get(0),
        )
        .expect("row count");
    assert_eq!(count, expected);
}

fn assert_analysis_conflict<T>(result: &Result<T, HubStoreError>, subject: &str) {
    assert!(
        matches!(
            result,
            Err(HubStoreError::Conflict {
                entity: HubEntity::GroupModelAnalysis,
                ..
            })
        ),
        "{subject} must conflict"
    );
}
