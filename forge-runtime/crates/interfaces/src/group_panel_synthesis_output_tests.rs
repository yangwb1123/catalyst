use super::{GroupPanelSynthesisInspectionView, GroupPanelSynthesisListItemView, write_synthesis};
use crate::runtime_domain::{
    GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION, GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
    GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupPanelSynthesisConfig, GroupPanelSynthesisInspection, GroupPanelSynthesisOutcome,
    GroupPanelSynthesisOutputTarget, GroupPanelSynthesisPreparedReceipt,
    GroupPanelSynthesisProvider, GroupPanelSynthesisRecord, GroupPanelSynthesisRecovery,
    GroupPanelSynthesisResult, GroupPanelSynthesisResultArtifact, GroupPanelSynthesisSource,
    GroupPanelSynthesisStatus, GroupPanelSynthesisWritebackTarget, Usage,
};

#[test]
fn default_view_is_redacted_and_honest() {
    let view = GroupPanelSynthesisInspectionView::from_inspection(fixture(), false);
    let value = serde_json::to_value(view).expect("view JSON");
    let encoded = value.to_string();

    assert_eq!(value["single_model"], true);
    assert_eq!(value["synthesis_performed"], true);
    for field in [
        "discussion_performed",
        "consensus_reached",
        "factual_verification_performed",
        "tools_used",
        "workspace_accessed",
        "writeback_performed",
        "prompt_included",
        "input_included",
        "request_included",
        "panel_results_included",
        "result_included",
    ] {
        assert_eq!(value[field], false, "{field}");
    }
    for secret in [
        "secret synthesis",
        "panel answer",
        "system prompt",
        "request body",
    ] {
        assert!(!encoded.contains(secret), "view leaked {secret}");
    }
}

#[test]
fn included_result_is_terminal_escaped_for_human_output() {
    let view = GroupPanelSynthesisInspectionView::from_inspection(fixture(), true);
    let mut output = Vec::new();
    write_synthesis(&view, &mut output).expect("human output");
    let text = String::from_utf8(output).expect("UTF-8");

    assert!(text.contains("secret synthesis\\n\\x1b[2J"));
    assert!(!text.contains('\u{1b}'));
    assert!(text.contains("single model only: no discussion, consensus"));
}

#[test]
fn metadata_list_never_claims_unvalidated_synthesis_was_performed() {
    let terminal = serde_json::to_value(GroupPanelSynthesisListItemView::from_record(record()))
        .expect("terminal list item");
    let mut awaiting = record();
    awaiting.status = GroupPanelSynthesisStatus::AwaitingConsent;
    let awaiting = serde_json::to_value(GroupPanelSynthesisListItemView::from_record(awaiting))
        .expect("awaiting list item");

    assert_eq!(terminal["synthesis_performed"], false);
    assert_eq!(terminal["result_included"], false);
    assert_eq!(awaiting["synthesis_performed"], false);
    assert_eq!(awaiting["single_model"], true);
}

fn fixture() -> GroupPanelSynthesisInspection {
    GroupPanelSynthesisInspection {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis: record(),
        events: Vec::new(),
        recovery: GroupPanelSynthesisRecovery::Terminal {
            outcome: GroupPanelSynthesisOutcome::Completed,
        },
        prepared: Some(GroupPanelSynthesisPreparedReceipt {
            v: GROUP_PANEL_SYNTHESIS_VERSION,
            synthesis_id: "synthesis-1".into(),
            source: source(),
            config_sha256: "33".repeat(32),
            request_sha256: "44".repeat(32),
            request_bytes: 100,
        }),
        dispatch: None,
        completion: None,
        result: Some(result()),
    }
}

fn record() -> GroupPanelSynthesisRecord {
    GroupPanelSynthesisRecord {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: "synthesis-1".into(),
        panel_id: "panel-1".into(),
        group_run_id: "group-run-1".into(),
        status: GroupPanelSynthesisStatus::Completed,
        source_snapshot_sha256: "11".repeat(32),
        panel_manifest_sha256: "22".repeat(32),
        config: config(),
        config_sha256: "33".repeat(32),
        request_sha256: "44".repeat(32),
        request_bytes: 100,
        protocol_version: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        created_at_ms: 10,
    }
}

fn config() -> GroupPanelSynthesisConfig {
    GroupPanelSynthesisConfig {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        provider: GroupPanelSynthesisProvider::OpenAiResponses,
        endpoint: "https://api.openai.com/v1/responses".into(),
        model: "test-model".into(),
        system_prompt_version: GROUP_PANEL_SYNTHESIS_SYSTEM_PROMPT_VERSION,
        system_prompt_sha256: "55".repeat(32),
        max_output_tokens: 4_096,
        max_model_output_bytes: 65_536,
        max_model_events: 4_096,
        output_target: GroupPanelSynthesisOutputTarget::LocalArtifact,
        writeback_target: GroupPanelSynthesisWritebackTarget::None,
    }
}

fn source() -> GroupPanelSynthesisSource {
    GroupPanelSynthesisSource {
        panel_version: 1,
        panel_id: "panel-1".into(),
        group_run_id: "group-run-1".into(),
        group_id: "group-1".into(),
        source_snapshot_sha256: "11".repeat(32),
        panel_manifest_sha256: "22".repeat(32),
        panel_manifest_bytes: 200,
        analysis_count: 2,
    }
}

fn result() -> GroupPanelSynthesisResultArtifact {
    GroupPanelSynthesisResultArtifact {
        result: GroupPanelSynthesisResult {
            v: GROUP_PANEL_SYNTHESIS_RESULT_VERSION,
            synthesis_id: "synthesis-1".into(),
            dispatch_id: "dispatch-1".into(),
            request_sha256: "44".repeat(32),
            outcome: GroupPanelSynthesisOutcome::Completed,
            answer: "secret synthesis\n\u{1b}[2J".into(),
            usage: Usage::default(),
        },
        result_sha256: "66".repeat(32),
        result_bytes: 200,
        created_at_ms: 12,
    }
}
