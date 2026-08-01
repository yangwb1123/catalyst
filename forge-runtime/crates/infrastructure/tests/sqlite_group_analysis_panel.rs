use std::sync::{Arc, Barrier};

use forge_runtime_domain::{
    ClaimGroupModelAnalysisDispatchResult, CompleteGroupModelAnalysis,
    GROUP_ANALYSIS_PANEL_VERSION, GroupAnalysisPanelContribution, GroupAnalysisPanelInspection,
    GroupAnalysisPanelManifest, GroupAnalysisPanelStore, GroupModelAnalysisInspection,
    GroupModelAnalysisStore, HubEntity, HubStoreError, PrepareGroupAnalysisPanel,
    PrepareGroupAnalysisPanelDisposition,
};
use rusqlite::Connection;

#[path = "sqlite_group_model_analysis_support/mod.rs"]
#[allow(dead_code)]
mod analysis_support;

use analysis_support::{Fixture, claim_request, result_artifact};

#[test]
fn panel_prepare_is_atomic_ordered_and_exactly_replayable() {
    let fixture = Fixture::new();
    let first = completed(
        &fixture,
        "analysis-a",
        "key-a",
        "dispatch-a",
        "frontend view",
    );
    let second = completed(
        &fixture,
        "analysis-b",
        "key-b",
        "dispatch-b",
        "backend view",
    );
    let request = panel_request(&first, &second, "panel-1", "panel-key", 50);

    let created = fixture
        .store
        .prepare_group_analysis_panel(&request)
        .expect("prepare panel");
    assert_eq!(
        created.disposition,
        PrepareGroupAnalysisPanelDisposition::Created
    );
    assert_eq!(
        analysis_ids(&created.inspection),
        ["analysis-a", "analysis-b"]
    );
    assert_replay_and_reads(&fixture, &request, &created.inspection);
}

fn assert_replay_and_reads(
    fixture: &Fixture,
    request: &PrepareGroupAnalysisPanel,
    created: &GroupAnalysisPanelInspection,
) {
    let mut replay = request.clone();
    replay.panel_id = "ignored-candidate".into();
    replay.created_at_ms = 999;
    let replayed = fixture
        .store
        .prepare_group_analysis_panel(&replay)
        .expect("replay exact panel");
    assert_eq!(
        replayed.disposition,
        PrepareGroupAnalysisPanelDisposition::Replayed
    );
    assert_eq!(replayed.inspection, *created);
    assert_eq!(
        fixture
            .store
            .inspect_group_analysis_panel("panel-1")
            .expect("inspect panel"),
        *created
    );
    assert_eq!(
        fixture
            .store
            .list_group_analysis_panels(Some("group-run-1"), 10)
            .expect("list panel"),
        vec![created.panel.clone()]
    );
}

#[test]
fn order_changes_conflict_and_invalid_sources_leave_no_partial_panel() {
    let fixture = Fixture::new();
    let first = completed(&fixture, "analysis-a", "key-a", "dispatch-a", "first");
    let second = completed(&fixture, "analysis-b", "key-b", "dispatch-b", "second");
    let request = panel_request(&first, &second, "panel-1", "panel-key", 50);
    fixture
        .store
        .prepare_group_analysis_panel(&request)
        .expect("prepare panel");

    let mut reversed = request.clone();
    reversed.manifest.contributions.reverse();
    assert!(matches!(
        fixture.store.prepare_group_analysis_panel(&reversed),
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAnalysisPanel,
            ..
        })
    ));

    let mut absent = request;
    absent.panel_id = "panel-2".into();
    absent.idempotency_key = "panel-key-2".into();
    absent.manifest.contributions[1].analysis.analysis_id = "missing-analysis".into();
    absent.manifest.contributions[1].result.result.analysis_id = "missing-analysis".into();
    assert!(matches!(
        fixture.store.prepare_group_analysis_panel(&absent),
        Err(HubStoreError::Conflict {
            entity: HubEntity::GroupAnalysisPanel,
            ..
        })
    ));
    assert_eq!(panel_row_count(&fixture), 1);
}

#[test]
fn corrupt_candidate_source_is_not_downgraded_to_a_conflict() {
    let fixture = Fixture::new();
    let first = completed(&fixture, "analysis-a", "key-a", "dispatch-a", "first");
    let second = completed(&fixture, "analysis-b", "key-b", "dispatch-b", "second");
    Connection::open(&fixture.database)
        .expect("raw SQLite")
        .execute(
            "UPDATE group_model_analysis_results
             SET result_sha256=zeroblob(32) WHERE analysis_id='analysis-a'",
            [],
        )
        .expect("inject source corruption");

    let result = fixture.store.prepare_group_analysis_panel(&panel_request(
        &first,
        &second,
        "panel-1",
        "panel-key",
        50,
    ));
    assert!(matches!(result, Err(HubStoreError::Corrupt { .. })));
    assert_eq!(panel_row_count(&fixture), 0);
}

#[test]
fn concurrent_same_key_creates_one_panel_and_replays_one_identity() {
    const WORKERS: usize = 6;
    let fixture = Fixture::new();
    let first = completed(&fixture, "analysis-a", "key-a", "dispatch-a", "first");
    let second = completed(&fixture, "analysis-b", "key-b", "dispatch-b", "second");
    let base = panel_request(&first, &second, "panel-0", "shared-panel-key", 50);
    let barrier = Arc::new(Barrier::new(WORKERS));
    let mut workers = Vec::new();
    for index in 0..WORKERS {
        let store = fixture.store.clone();
        let barrier = Arc::clone(&barrier);
        let mut request = base.clone();
        request.panel_id = format!("panel-{index}");
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store
                .prepare_group_analysis_panel(&request)
                .expect("concurrent prepare")
        }));
    }
    let results = workers
        .into_iter()
        .map(|worker| worker.join().expect("panel worker"))
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| {
                result.disposition == PrepareGroupAnalysisPanelDisposition::Created
            })
            .count(),
        1
    );
    let identity = &results[0].inspection;
    assert!(results.iter().all(|result| result.inspection == *identity));
    assert_eq!(panel_row_count(&fixture), 1);
}

#[test]
fn full_inspection_rejects_manifest_member_and_source_corruption() {
    for sql in [
        "UPDATE group_analysis_panels SET manifest_blob=zeroblob(length(manifest_blob))",
        "UPDATE group_analysis_panels SET manifest_sha256=zeroblob(32)",
        "UPDATE group_analysis_panel_analyses SET result_sha256=zeroblob(32)",
        "DELETE FROM group_analysis_panel_analyses WHERE position=1",
        "UPDATE group_model_analysis_results SET result_sha256=zeroblob(32)
         WHERE analysis_id='analysis-a'",
        "UPDATE group_runs SET snapshot_sha256=zeroblob(32) WHERE id='group-run-1'",
    ] {
        let fixture = prepared_fixture();
        Connection::open(&fixture.database)
            .expect("raw SQLite")
            .execute_batch(sql)
            .expect("inject corruption");
        assert!(matches!(
            fixture.store.inspect_group_analysis_panel("panel-1"),
            Err(HubStoreError::Corrupt { .. })
        ));
    }
}

fn prepared_fixture() -> Fixture {
    let fixture = Fixture::new();
    let first = completed(&fixture, "analysis-a", "key-a", "dispatch-a", "first");
    let second = completed(&fixture, "analysis-b", "key-b", "dispatch-b", "second");
    fixture
        .store
        .prepare_group_analysis_panel(&panel_request(&first, &second, "panel-1", "panel-key", 50))
        .expect("prepare panel");
    fixture
}

fn completed(
    fixture: &Fixture,
    analysis_id: &str,
    key: &str,
    dispatch_id: &str,
    answer: &str,
) -> GroupModelAnalysisInspection {
    fixture.prepare(analysis_id, key);
    let claimed = fixture
        .store
        .claim_group_model_analysis_dispatch(&claim_request(analysis_id, dispatch_id, 30))
        .expect("claim analysis");
    assert!(matches!(
        claimed,
        ClaimGroupModelAnalysisDispatchResult::Claimed { .. }
    ));
    let dispatched = fixture
        .store
        .inspect_group_model_analysis(analysis_id)
        .expect("inspect dispatch");
    fixture
        .store
        .complete_group_model_analysis(&CompleteGroupModelAnalysis {
            v: 1,
            artifact: result_artifact(&dispatched, answer, 40),
        })
        .expect("complete analysis")
        .inspection
}

fn panel_request(
    first: &GroupModelAnalysisInspection,
    second: &GroupModelAnalysisInspection,
    panel_id: &str,
    key: &str,
    created_at_ms: u64,
) -> PrepareGroupAnalysisPanel {
    let source = first
        .prepared
        .as_ref()
        .expect("prepared receipt")
        .source
        .clone();
    PrepareGroupAnalysisPanel {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        panel_id: panel_id.into(),
        manifest: GroupAnalysisPanelManifest {
            v: GROUP_ANALYSIS_PANEL_VERSION,
            source,
            contributions: vec![contribution(first), contribution(second)],
        },
        idempotency_key: key.into(),
        created_at_ms,
    }
}

fn contribution(inspection: &GroupModelAnalysisInspection) -> GroupAnalysisPanelContribution {
    GroupAnalysisPanelContribution {
        analysis: inspection.analysis.clone(),
        result: inspection.result.clone().expect("completed result"),
    }
}

fn analysis_ids(inspection: &GroupAnalysisPanelInspection) -> Vec<&str> {
    inspection
        .manifest
        .contributions
        .iter()
        .map(|item| item.analysis.analysis_id.as_str())
        .collect()
}

fn panel_row_count(fixture: &Fixture) -> i64 {
    Connection::open(&fixture.database)
        .expect("raw SQLite")
        .query_row("SELECT COUNT(*) FROM group_analysis_panels", [], |row| {
            row.get(0)
        })
        .expect("panel count")
}
