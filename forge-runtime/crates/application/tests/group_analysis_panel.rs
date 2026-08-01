#[path = "group_analysis_panel_support/mod.rs"]
mod support;

use forge_runtime_application::GroupAnalysisPanelServiceError;
use forge_runtime_domain::{
    GroupAnalysisPanelRecord, GroupModelAnalysisOutcome, MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT,
};

use support::{
    GROUP_RUN_ID, awaiting_analysis, bad_result_digest, completed_analysis,
    contradictory_prepared_source, harness as make_harness, other_snapshot, prepare_input,
};

#[test]
fn prepare_freezes_completed_analyses_in_caller_order() {
    let harness = make_harness();
    let result = harness
        .service
        .prepare(&prepare_input(&["analysis-b", "analysis-a"]))
        .expect("prepare ordered panel");

    let ids = result
        .inspection
        .manifest
        .contributions
        .iter()
        .map(|value| value.analysis.analysis_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["analysis-b", "analysis-a"]);
    assert_eq!(
        harness.panels.last_request().manifest,
        result.inspection.manifest
    );
}

#[test]
fn prepare_rejects_duplicate_and_out_of_bounds_counts() {
    let harness = make_harness();
    for ids in [
        vec!["analysis-a"],
        vec!["analysis-a", "analysis-a"],
        vec![
            "analysis-a",
            "analysis-b",
            "analysis-c",
            "analysis-d",
            "analysis-e",
            "analysis-f",
            "analysis-g",
            "analysis-h",
            "analysis-i",
        ],
    ] {
        assert!(matches!(
            harness.service.prepare(&prepare_input(&ids)),
            Err(GroupAnalysisPanelServiceError::InvalidInput)
        ));
    }
}

#[test]
fn prepare_rejects_length_nonterminal_and_cross_source_analyses() {
    let cases = [
        completed_analysis(
            "analysis-b",
            &make_harness().snapshot,
            GroupModelAnalysisOutcome::Length,
            "truncated",
        ),
        awaiting_analysis("analysis-b", &make_harness().snapshot),
        completed_analysis(
            "analysis-b",
            &other_snapshot(),
            GroupModelAnalysisOutcome::Completed,
            "other source",
        ),
    ];
    for candidate in cases {
        let harness = make_harness();
        harness.analyses.respond_with("analysis-b", candidate);
        assert!(matches!(
            harness
                .service
                .prepare(&prepare_input(&["analysis-a", "analysis-b"])),
            Err(GroupAnalysisPanelServiceError::InvalidInput
                | GroupAnalysisPanelServiceError::InconsistentStoreResult)
        ));
    }
}

#[test]
fn prepare_fails_closed_on_analysis_and_panel_store_inconsistency() {
    let wrong_harness = make_harness();
    let wrong = completed_analysis(
        "analysis-b",
        &wrong_harness.snapshot,
        GroupModelAnalysisOutcome::Completed,
        "wrong identity",
    );
    wrong_harness.analyses.respond_with("analysis-a", wrong);
    assert_inconsistent_prepare(&wrong_harness);

    let harness = make_harness();
    let valid = completed_analysis(
        "analysis-a",
        &harness.snapshot,
        GroupModelAnalysisOutcome::Completed,
        "bad digest",
    );
    harness
        .analyses
        .respond_with("analysis-a", bad_result_digest(valid));
    assert_inconsistent_prepare(&harness);

    let harness = make_harness();
    let contradictory = completed_analysis(
        "analysis-a",
        &harness.snapshot,
        GroupModelAnalysisOutcome::Completed,
        "contradictory source",
    );
    harness
        .analyses
        .respond_with("analysis-a", contradictory_prepared_source(contradictory));
    assert_inconsistent_prepare(&harness);

    let harness = make_harness();
    harness.panels.corrupt_next_prepare();
    assert_inconsistent_prepare(&harness);
}

#[test]
fn inspect_revalidates_exact_source_artifacts() {
    let harness = make_harness();
    harness
        .service
        .prepare(&prepare_input(&["analysis-a", "analysis-b"]))
        .expect("prepare panel");
    let changed = completed_analysis(
        "analysis-a",
        &harness.snapshot,
        GroupModelAnalysisOutcome::Completed,
        "changed after freeze",
    );
    harness.analyses.respond_with("analysis-a", changed);

    assert!(matches!(
        harness.service.inspect("panel-1"),
        Err(GroupAnalysisPanelServiceError::InconsistentStoreResult)
    ));
}

#[test]
fn list_enforces_metadata_filter_uniqueness_and_bounds() {
    let harness = make_harness();
    let prepared = harness
        .service
        .prepare(&prepare_input(&["analysis-a", "analysis-b"]))
        .expect("prepare panel");
    assert_eq!(
        harness
            .service
            .list(Some(GROUP_RUN_ID), 1)
            .expect("bounded list"),
        vec![prepared.inspection.panel.clone()]
    );
    assert_invalid_list_limit(&harness);
    assert_duplicate_list_is_inconsistent(&harness, prepared.inspection.panel.clone());
    assert_wrong_filter_is_inconsistent(&harness, prepared.inspection.panel);
}

fn assert_inconsistent_prepare(harness: &support::Harness) {
    assert!(matches!(
        harness
            .service
            .prepare(&prepare_input(&["analysis-a", "analysis-b"])),
        Err(GroupAnalysisPanelServiceError::InconsistentStoreResult)
    ));
}

fn assert_invalid_list_limit(harness: &support::Harness) {
    for limit in [0, MAX_GROUP_ANALYSIS_PANEL_LIST_LIMIT + 1] {
        assert!(matches!(
            harness.service.list(Some(GROUP_RUN_ID), limit),
            Err(GroupAnalysisPanelServiceError::InvalidInput)
        ));
    }
}

fn assert_duplicate_list_is_inconsistent(
    harness: &support::Harness,
    record: GroupAnalysisPanelRecord,
) {
    harness
        .panels
        .set_list_override(vec![record.clone(), record]);
    assert!(matches!(
        harness.service.list(Some(GROUP_RUN_ID), 2),
        Err(GroupAnalysisPanelServiceError::InconsistentStoreResult)
    ));
}

fn assert_wrong_filter_is_inconsistent(
    harness: &support::Harness,
    mut record: GroupAnalysisPanelRecord,
) {
    record.group_run_id = "group-run-2".into();
    harness.panels.set_list_override(vec![record]);
    assert!(matches!(
        harness.service.list(Some(GROUP_RUN_ID), 1),
        Err(GroupAnalysisPanelServiceError::InconsistentStoreResult)
    ));
}
