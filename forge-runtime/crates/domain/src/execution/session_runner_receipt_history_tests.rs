use super::*;
use serde_json::Value;

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
);

#[test]
fn session_runner_receipt_history_fixture_reduces_to_latest_manual_boundary() {
    let observation: SessionRunnerReceiptHistoryObservation =
        serde_json::from_str(FIXTURE).expect("strict session receipt history fixture");
    observation.validate().expect("valid history observation");
    let reduced = observe_session_runner_receipt_history(request(&observation))
        .expect("reduce session receipt history");
    let expected: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let actual = serde_json::to_value(reduced).expect("history JSON");
    assert_eq!(actual, expected);
    assert_eq!(observation.attempt_count, 2);
    assert_eq!(observation.latest_disposition_kind, "uncertain");
    assert!(observation.reconciliation_required);
    assert!(observation.manual_review_required);
    assert!(!observation.automatic_retry);
    assert_eq!(observation.follow_up, "reconciliation_manual");
}

#[test]
fn session_runner_receipt_history_rejects_wire_and_lifecycle_drift() {
    let duplicate = FIXTURE.replace(
        "  \"schema_version\": \"forge.session-runner-receipt-history/v1\",\n",
        "  \"schema_version\": \"forge.session-runner-receipt-history/v1\",\n  \"schema_version\": \"forge.session-runner-receipt-history/v1\",\n",
    );
    assert!(serde_json::from_str::<SessionRunnerReceiptHistoryObservation>(&duplicate).is_err());

    let unknown = FIXTURE.replace(
        "  \"evaluation_mode\": \"pure_session_runner_receipt_history_only\",\n",
        "  \"evaluation_mode\": \"pure_session_runner_receipt_history_only\",\n  \"unexpected\": true,\n",
    );
    assert!(serde_json::from_str::<SessionRunnerReceiptHistoryObservation>(&unknown).is_err());

    let trailing = format!("{FIXTURE} {{}}");
    assert!(serde_json::from_str::<SessionRunnerReceiptHistoryObservation>(&trailing).is_err());

    let base: SessionRunnerReceiptHistoryObservation =
        serde_json::from_str(FIXTURE).expect("fixture");
    let mut duplicate_attempt = base.clone();
    duplicate_attempt.receipts[1].receipt_observation.attempt_id = duplicate_attempt.receipts[0]
        .receipt_observation
        .attempt_id
        .clone();
    assert!(duplicate_attempt.validate().is_err());

    let mut out_of_order = base.clone();
    out_of_order.receipts[1].receipt_observation.observed_at_ms = 50;
    assert!(out_of_order.validate().is_err());

    let mut retry_after_uncertain = base.clone();
    let first = &mut retry_after_uncertain.receipts[0].receipt_observation;
    first.disposition_kind = "uncertain".into();
    first.uncertain = true;
    first.reconciliation_required = true;
    first.manual_review_required = true;
    first.follow_up = "reconciliation_manual".into();
    assert!(retry_after_uncertain.validate().is_err());

    let mut summary_drift = base.clone();
    summary_drift.attempt_count += 1;
    assert!(summary_drift.validate().is_err());

    let mut selected = base.clone();
    selected.selected_target_id = Some("runner-2".into());
    assert!(selected.validate().is_err());

    let mut authoritative = base;
    authoritative.authority.receipt_persisted = true;
    assert!(authoritative.validate().is_err());
}

fn request(
    observation: &SessionRunnerReceiptHistoryObservation,
) -> SessionRunnerReceiptHistoryRequest {
    SessionRunnerReceiptHistoryRequest {
        owner: observation.owner.clone(),
        conversation_id: observation.conversation_id.clone(),
        prompt_id: observation.prompt_id.clone(),
        run_id: observation.run_id.clone(),
        receipts: observation.receipts.clone(),
    }
}
