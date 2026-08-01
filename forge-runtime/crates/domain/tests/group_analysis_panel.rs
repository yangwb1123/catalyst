use forge_runtime_domain::{
    GROUP_ANALYSIS_PANEL_VERSION, GROUP_CONTEXT_VERSION, GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
    GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT, GROUP_MODEL_ANALYSIS_RESULT_VERSION,
    GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION, GROUP_MODEL_ANALYSIS_VERSION, GROUP_RUN_VERSION,
    GroupAnalysisPanelContribution, GroupAnalysisPanelInspection, GroupAnalysisPanelManifest,
    GroupAnalysisPanelRecord, GroupAnalysisPanelStatus, GroupModelAnalysisConfig,
    GroupModelAnalysisOutcome, GroupModelAnalysisProvider, GroupModelAnalysisRecord,
    GroupModelAnalysisResult, GroupModelAnalysisResultArtifact, GroupModelAnalysisSource,
    GroupModelAnalysisStatus, MAX_GROUP_ANALYSIS_PANEL_ANALYSES, Usage,
};

#[test]
fn ordered_completed_manifest_and_matching_inspection_are_valid() {
    let manifest = manifest(&["analysis-front", "analysis-api", "analysis-sso"]);

    manifest.validate().expect("valid ordered panel manifest");
    assert_eq!(
        contribution_ids(&manifest),
        ["analysis-front", "analysis-api", "analysis-sso"]
    );

    inspection(manifest)
        .validate()
        .expect("matching panel inspection");
}

#[test]
fn duplicate_and_out_of_bounds_contribution_counts_are_rejected() {
    let duplicate = GroupAnalysisPanelManifest {
        contributions: vec![contribution("analysis-1"), contribution("analysis-1")],
        ..manifest(&["analysis-1", "analysis-2"])
    };
    assert!(duplicate.validate().is_err());
    assert!(manifest(&["analysis-only"]).validate().is_err());

    let identifiers: Vec<String> = (0..=MAX_GROUP_ANALYSIS_PANEL_ANALYSES)
        .map(|index| format!("analysis-{index}"))
        .collect();
    let oversized = manifest_from_strings(&identifiers);
    assert!(oversized.validate().is_err());
}

#[test]
fn length_outcome_is_not_a_complete_panel_contribution() {
    let mut truncated = contribution("analysis-length");
    truncated.result.result.outcome = GroupModelAnalysisOutcome::Length;
    let panel = GroupAnalysisPanelManifest {
        contributions: vec![contribution("analysis-ok"), truncated],
        ..manifest(&["analysis-1", "analysis-2"])
    };

    assert!(panel.validate().is_err());
}

#[test]
fn cross_source_contributions_are_rejected() {
    let mut other_run = contribution("analysis-other-run");
    other_run.analysis.group_run_id = "group-run-2".into();
    let panel = GroupAnalysisPanelManifest {
        contributions: vec![contribution("analysis-ok"), other_run],
        ..manifest(&["analysis-1", "analysis-2"])
    };
    assert!(panel.validate().is_err());

    let mut other_snapshot = contribution("analysis-other-snapshot");
    other_snapshot.analysis.source_snapshot_sha256 = digest('9');
    let panel = GroupAnalysisPanelManifest {
        contributions: vec![contribution("analysis-ok"), other_snapshot],
        ..manifest(&["analysis-1", "analysis-2"])
    };
    assert!(panel.validate().is_err());
}

#[test]
fn panel_record_and_manifest_must_agree() {
    let valid = inspection(manifest(&["analysis-1", "analysis-2"]));

    assert_inspection_rejected(&valid, |candidate| {
        candidate.panel.group_run_id = "group-run-2".into();
    });
    assert_inspection_rejected(&valid, |candidate| {
        candidate.panel.source_snapshot_sha256 = digest('9');
    });
    assert_inspection_rejected(&valid, |candidate| {
        candidate.panel.analysis_count += 1;
    });
    assert_inspection_rejected(&valid, |candidate| {
        candidate.panel.analysis_count = 1;
    });
}

fn manifest(identifiers: &[&str]) -> GroupAnalysisPanelManifest {
    GroupAnalysisPanelManifest {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        source: source(),
        contributions: identifiers
            .iter()
            .map(|identifier| contribution(identifier))
            .collect(),
    }
}

fn manifest_from_strings(identifiers: &[String]) -> GroupAnalysisPanelManifest {
    let identifiers: Vec<&str> = identifiers.iter().map(String::as_str).collect();
    manifest(&identifiers)
}

fn contribution(identifier: &str) -> GroupAnalysisPanelContribution {
    let analysis = analysis_record(identifier);
    let result = result_artifact(identifier);
    GroupAnalysisPanelContribution { analysis, result }
}

fn inspection(manifest: GroupAnalysisPanelManifest) -> GroupAnalysisPanelInspection {
    let analysis_count = manifest.contributions.len();
    let group_run_id = manifest.source.group_run_id.clone();
    let source_snapshot_sha256 = manifest.source.snapshot_sha256.clone();
    GroupAnalysisPanelInspection {
        v: GROUP_ANALYSIS_PANEL_VERSION,
        panel: GroupAnalysisPanelRecord {
            v: GROUP_ANALYSIS_PANEL_VERSION,
            panel_id: "panel-1".into(),
            group_run_id,
            status: GroupAnalysisPanelStatus::Prepared,
            source_snapshot_sha256,
            manifest_sha256: digest('a'),
            manifest_bytes: 1_024,
            analysis_count,
            created_at_ms: 40,
        },
        manifest,
    }
}

fn contribution_ids(manifest: &GroupAnalysisPanelManifest) -> Vec<&str> {
    manifest
        .contributions
        .iter()
        .map(|value| value.analysis.analysis_id.as_str())
        .collect()
}

fn assert_inspection_rejected(
    valid: &GroupAnalysisPanelInspection,
    mutate: impl FnOnce(&mut GroupAnalysisPanelInspection),
) {
    let mut candidate = valid.clone();
    mutate(&mut candidate);
    assert!(candidate.validate().is_err());
}

fn analysis_record(identifier: &str) -> GroupModelAnalysisRecord {
    GroupModelAnalysisRecord {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: identifier.into(),
        group_run_id: "group-run-1".into(),
        status: GroupModelAnalysisStatus::Completed,
        source_snapshot_sha256: digest('3'),
        config: analysis_config(),
        config_sha256: digest('4'),
        request_sha256: digest('5'),
        request_bytes: 32,
        protocol_version: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        created_at_ms: 10,
    }
}

fn result_artifact(identifier: &str) -> GroupModelAnalysisResultArtifact {
    GroupModelAnalysisResultArtifact {
        result: GroupModelAnalysisResult {
            v: GROUP_MODEL_ANALYSIS_RESULT_VERSION,
            analysis_id: identifier.into(),
            dispatch_id: format!("dispatch-{identifier}"),
            request_sha256: digest('5'),
            outcome: GroupModelAnalysisOutcome::Completed,
            answer: format!("Completed analysis for {identifier}."),
            usage: Usage {
                input_tokens: 100,
                output_tokens: 12,
            },
        },
        result_sha256: digest('6'),
        result_bytes: 256,
        created_at_ms: 30,
    }
}

fn source() -> GroupModelAnalysisSource {
    GroupModelAnalysisSource {
        group_run_version: GROUP_RUN_VERSION,
        group_run_id: "group-run-1".into(),
        group_id: "group-1".into(),
        context_version: GROUP_CONTEXT_VERSION,
        context_slice_sha256: digest('2'),
        snapshot_sha256: digest('3'),
        snapshot_bytes: 128,
    }
}

fn analysis_config() -> GroupModelAnalysisConfig {
    GroupModelAnalysisConfig {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        provider: GroupModelAnalysisProvider::OpenAiResponses,
        endpoint: GROUP_MODEL_ANALYSIS_PROVIDER_ENDPOINT.into(),
        model: "gpt-5".into(),
        system_prompt_version: GROUP_MODEL_ANALYSIS_SYSTEM_PROMPT_VERSION,
        system_prompt_sha256: digest('1'),
        max_output_tokens: 1_024,
        max_model_output_bytes: 4_096,
        max_model_events: 128,
    }
}

fn digest(character: char) -> String {
    character.to_string().repeat(64)
}
