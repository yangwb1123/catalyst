use std::{fs, path::PathBuf};

use serde_json::{Value, json};

use super::{execute, write_output};
use crate::args::DeviceCommand;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json")
}

fn fixture_text() -> String {
    fs::read_to_string(fixture_path()).expect("session Runner receipt history fixture")
}

fn run_with_bytes(
    bytes: impl AsRef<[u8]>,
) -> Result<super::SessionRunnerReceiptHistoryObservation, Box<dyn std::error::Error>> {
    let file = tempfile::NamedTempFile::new().expect("temporary history input");
    fs::write(file.path(), bytes).expect("write temporary history input");
    execute(&DeviceCommand::SessionRunnerReceiptHistoryPreview {
        input: file.path().display().to_string(),
    })
}

#[test]
fn history_preview_reduces_failed_attempt_to_uncertain_manual_boundary() {
    let fixture = fixture_text();
    let output = run_with_bytes(&fixture).expect("valid receipt history");
    assert_eq!(
        serde_json::to_value(&output).expect("history JSON"),
        serde_json::from_str::<Value>(&fixture).expect("fixture JSON")
    );
    assert_eq!(
        output.schema_version,
        "forge.session-runner-receipt-history/v1"
    );
    assert_eq!(output.receipts.len(), 2);
    assert_eq!(
        output.receipts[0].receipt_observation.disposition_kind,
        "failed"
    );
    assert_eq!(
        output.receipts[1].receipt_observation.disposition_kind,
        "uncertain"
    );
    assert_eq!(output.attempt_count, 2);
    assert_eq!(output.latest_attempt_id, "attempt-002");
    assert_eq!(output.latest_command_id, "command-002");
    assert_eq!(output.latest_target_id, "runner-2");
    assert!(output.reconciliation_required);
    assert!(output.manual_review_required);
    assert!(!output.automatic_retry);
    assert_eq!(output.follow_up, "reconciliation_manual");
    assert!(output.selected_target_id.is_none());
    assert!(!output.authority.receipt_persisted);
    assert!(!output.authority.execution_authorized);

    let mut rendered = Vec::new();
    write_output(&output, false, &mut rendered).expect("human history output");
    let rendered = String::from_utf8(rendered).expect("UTF-8 history output");
    assert!(rendered.contains(
        "offline session Runner receipt history [forge.session-runner-receipt-history/v1]"
    ));
    assert!(rendered.contains(
        "attempt[1]: command=command-001 attempt=attempt-001 target=runner-1 disposition=failed"
    ));
    assert!(rendered.contains(
        "attempt[2]: command=command-002 attempt=attempt-002 target=runner-2 disposition=uncertain"
    ));
    assert!(rendered.contains("follow_up=reconciliation_manual"));
    assert!(rendered.contains("automatic_retry=false"));
    assert!(!rendered.contains("forge-task"));
}

#[test]
fn history_preview_rejects_unknown_duplicate_trailing_and_summary_drift() {
    let fixture = fixture_text();

    let unknown = fixture.replacen(
        "  \"evaluation_mode\": \"pure_session_runner_receipt_history_only\",\n",
        "  \"evaluation_mode\": \"pure_session_runner_receipt_history_only\",\n  \"unexpected\": true,\n",
        1,
    );
    assert!(run_with_bytes(unknown).is_err());

    let duplicate = fixture.replacen(
        "  \"schema_version\": \"forge.session-runner-receipt-history/v1\",\n",
        "  \"schema_version\": \"forge.session-runner-receipt-history/v1\",\n  \"schema_version\": \"forge.session-runner-receipt-history/v1\",\n",
        1,
    );
    assert!(run_with_bytes(duplicate).is_err());

    assert!(run_with_bytes(format!("{fixture} {{}}")).is_err());

    let mut summary_drift: Value = serde_json::from_str(&fixture).expect("fixture JSON");
    summary_drift["attempt_count"] = json!(3);
    assert!(run_with_bytes(serde_json::to_vec(&summary_drift).unwrap()).is_err());
}

#[test]
fn history_preview_rejects_retry_after_uncertain_and_selected_target() {
    let mut fixture: Value = serde_json::from_str(&fixture_text()).expect("fixture JSON");
    let first = &mut fixture["receipts"][0]["receipt_observation"];
    first["disposition_kind"] = json!("uncertain");
    first["uncertain"] = json!(true);
    first["reconciliation_required"] = json!(true);
    first["manual_review_required"] = json!(true);
    first["follow_up"] = json!("reconciliation_manual");
    assert!(run_with_bytes(serde_json::to_vec(&fixture).unwrap()).is_err());

    let mut selected: Value = serde_json::from_str(&fixture_text()).expect("fixture JSON");
    selected["selected_target_id"] = json!("runner-2");
    assert!(run_with_bytes(serde_json::to_vec(&selected).unwrap()).is_err());
}

#[test]
fn history_preview_uses_attempt_id_as_equal_timestamp_tie_breaker() {
    let mut equal_time: Value = serde_json::from_str(&fixture_text()).expect("fixture JSON");
    equal_time["receipts"][0]["receipt_observation"]["observed_at_ms"] = json!(100);
    equal_time["receipts"][1]["receipt_observation"]["observed_at_ms"] = json!(100);
    equal_time["latest_observed_at_ms"] = json!(100);
    let output = run_with_bytes(serde_json::to_vec(&equal_time).unwrap())
        .expect("ascending Attempt IDs are a valid equal-time history");
    assert_eq!(
        output.receipts[0].receipt_observation.attempt_id,
        "attempt-001"
    );
    assert_eq!(
        output.receipts[1].receipt_observation.attempt_id,
        "attempt-002"
    );
    assert_eq!(output.latest_observed_at_ms, 100);

    let mut descending: Value = serde_json::from_str(&fixture_text()).expect("fixture JSON");
    descending["receipts"][0]["receipt_observation"]["observed_at_ms"] = json!(100);
    descending["receipts"][1]["receipt_observation"]["observed_at_ms"] = json!(100);
    descending["receipts"][0]["receipt_observation"]["attempt_id"] = json!("attempt-z");
    descending["receipts"][1]["receipt_observation"]["attempt_id"] = json!("attempt-a");
    descending["latest_attempt_id"] = json!("attempt-a");
    descending["latest_observed_at_ms"] = json!(100);
    assert!(run_with_bytes(serde_json::to_vec(&descending).unwrap()).is_err());
}
