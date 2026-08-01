#[path = "group_panel_synthesis_support/mod.rs"]
mod support;

use std::fs;

use serde_json::Value;
use support::{
    FIRST_ANSWER, FROZEN_PROMPT, Fixture, SECOND_ANSWER, SYNTHESIS_KEY, WORKSPACE_SECRET, run_json,
};

#[test]
fn local_prepare_show_list_and_replay_are_exact_redacted_and_side_effect_free() {
    let fixture = Fixture::new();
    let before = fixture.side_effect_counts();
    let created = fixture.prepare_synthesis();
    assert_prepared(&created, "created", &fixture.panel_id);
    assert_private(&created);
    let synthesis_id = synthesis_id(&created);
    assert_request_contract(&fixture, synthesis_id);

    let shown = fixture.show(synthesis_id);
    assert_eq!(shown["type"], "group_panel_synthesis");
    assert_honesty(&shown["inspection"], false);
    assert_eq!(shown["inspection"], created["inspection"]);
    assert_private(&shown);

    let listed = run_json(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "synthesis",
            "list",
            &fixture.panel_id,
            "--limit",
            "5",
        ],
    );
    assert_eq!(listed["metadata_only"], true);
    assert_eq!(listed["source_and_journal_validated"], false);
    assert_eq!(listed["inspect_with"], "group synthesis show SYNTHESIS_ID");
    assert_eq!(listed["syntheses"][0]["synthesis_id"], synthesis_id);
    assert_honesty(&listed["syntheses"][0], false);
    assert_private(&listed);

    fs::remove_dir_all(fixture.project.path()).expect("remove source workspace");
    let replay = fixture.prepare_synthesis();
    assert_prepared(&replay, "replayed", &fixture.panel_id);
    assert_eq!(replay["inspection"], created["inspection"]);
    assert_eq!(fixture.side_effect_counts(), before);
}

#[test]
fn consent_and_credentials_fail_before_claim_and_claimed_work_is_not_resent() {
    let fixture = Fixture::new();
    let prepared = fixture.prepare_synthesis();
    let synthesis_id = synthesis_id(&prepared);
    let before = fixture.side_effect_counts();

    assert_consent_required(&fixture, synthesis_id);
    assert_missing_key_is_preclaim(&fixture, synthesis_id);
    fixture.claim_synthesis(synthesis_id);
    let recovered = fixture.invoke(&["group", "synthesis", "send", synthesis_id], None);
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    let recovered: Value = serde_json::from_slice(&recovered.stdout).expect("recovery JSON");
    assert_eq!(recovered["disposition"], "already_claimed");
    assert_eq!(
        recovered["inspection"]["recovery"]["status"],
        "dispatch_unknown"
    );
    assert_honesty(&recovered["inspection"], false);
    assert_private(&recovered);
    assert_eq!(fixture.side_effect_counts(), before);
}

fn assert_consent_required(fixture: &Fixture, synthesis_id: &str) {
    let output = fixture.invoke(&["group", "synthesis", "send", synthesis_id], None);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    for expected in [
        "--confirm-off-machine",
        "text of all 2 ordered copied panel results",
        "panel/source metadata",
        "does not separately attach Group dossier or excerpt fields",
        "Copied result text may itself quote or reproduce source content",
        "Prior Group analysis consent does not authorize this disclosure",
    ] {
        assert!(error.contains(expected), "{error}");
    }
    assert_awaiting(fixture, synthesis_id);
}

fn assert_missing_key_is_preclaim(fixture: &Fixture, synthesis_id: &str) {
    let output = fixture.invoke(
        &[
            "group",
            "synthesis",
            "send",
            synthesis_id,
            "--confirm-off-machine",
        ],
        None,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("OPENAI_API_KEY"));
    assert_awaiting(fixture, synthesis_id);
    assert_eq!(fixture.synthesis_event_count(synthesis_id), 1);
}

fn assert_prepared(output: &Value, disposition: &str, panel_id: &str) {
    assert_eq!(output["type"], "group_panel_synthesis_prepared");
    assert_eq!(output["disposition"], disposition);
    assert_eq!(output["inspection"]["synthesis"]["panel_id"], panel_id);
    assert_eq!(
        output["inspection"]["synthesis"]["status"],
        "awaiting_consent"
    );
    assert_eq!(
        output["inspection"]["recovery"]["status"],
        "awaiting_consent"
    );
    assert_honesty(&output["inspection"], false);
}

fn assert_honesty(inspection: &Value, synthesis_performed: bool) {
    assert_eq!(inspection["single_model"], true);
    assert_eq!(inspection["synthesis_performed"], synthesis_performed);
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
        assert_eq!(inspection[field], false, "{field}");
    }
    assert!(inspection.get("result").is_none());
}

fn assert_request_contract(fixture: &Fixture, synthesis_id: &str) {
    let (body, config) = fixture.request_and_config(synthesis_id);
    let body: Value = serde_json::from_slice(&body).expect("request JSON");
    assert_eq!(body["model"], "test-model");
    assert_eq!(body["tools"], serde_json::json!([]));
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], true);
    let input = body["input"][0]["content"]
        .as_str()
        .expect("manifest user input");
    assert!(input.contains(FIRST_ANSWER));
    assert!(input.contains(SECOND_ANSWER));
    assert!(!input.contains(FROZEN_PROMPT));
    assert!(!input.contains(WORKSPACE_SECRET));
    assert!(config.contains("untrusted data"));
    assert!(config.contains("\"output_target\":\"local_artifact\""));
    assert!(config.contains("\"writeback_target\":\"none\""));
}

fn assert_awaiting(fixture: &Fixture, synthesis_id: &str) {
    let shown = fixture.show(synthesis_id);
    assert_eq!(
        shown["inspection"]["synthesis"]["status"],
        "awaiting_consent"
    );
    assert_eq!(shown["inspection"]["dispatch"], Value::Null);
}

fn assert_private(output: &Value) {
    let encoded = output.to_string();
    for forbidden in [
        FIRST_ANSWER,
        SECOND_ANSWER,
        FROZEN_PROMPT,
        WORKSPACE_SECRET,
        SYNTHESIS_KEY,
        "analysis-key-a",
        "analysis-key-b",
        "panel-key",
        "Return only the synthesis",
        "\"request_body\"",
        "\"config_json\"",
        "\"events\"",
        "\"result\"",
    ] {
        assert!(!encoded.contains(forbidden), "output leaked {forbidden}");
    }
}

fn synthesis_id(output: &Value) -> &str {
    output["inspection"]["synthesis"]["synthesis_id"]
        .as_str()
        .expect("synthesis ID")
}
