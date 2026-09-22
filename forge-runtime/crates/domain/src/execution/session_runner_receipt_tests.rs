use super::*;
use crate::execution::runner_command::RunnerTerminalReceiptAuthority;
use crate::execution::runner_execution_intent::RunnerExecutionIntentAuthority;
use serde_json::{Value, json};

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-observation-v1.json"
);
const COMMAND_SHA256: &str = "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a";

fn request() -> SessionRunnerReceiptObservationRequest {
    let owner = RunnerExecutionOwner {
        issuer: "https://id.example".into(),
        subject: "user-1".into(),
        tenant_id: "tenant-1".into(),
    };
    SessionRunnerReceiptObservationRequest {
        owner: owner.clone(),
        conversation_id: "conversation-001".into(),
        prompt_id: "prompt-001".into(),
        run_id: "run-001".into(),
        runner_execution_intent: RunnerExecutionIntentObservation {
            schema_version: RUNNER_EXECUTION_INTENT_SCHEMA_VERSION,
            evaluation_mode: RUNNER_EXECUTION_INTENT_EVALUATION_MODE,
            owner,
            conversation_id: "conversation-001".into(),
            prompt_id: "prompt-001".into(),
            run_id: "run-001".into(),
            attempt_id: "attempt-001".into(),
            command_id: "command-001".into(),
            target_id: "runner-1".into(),
            command_sha256: COMMAND_SHA256.into(),
            idempotency_key: "run-001:attempt-001:command-001".into(),
            prompt_run_binding_valid: true,
            runner_command_binding_valid: true,
            preview_only: true,
            selected_target_id: None,
            authority: RunnerExecutionIntentAuthority::default(),
        },
        receipt_observation: RunnerTerminalReceiptObservation {
            schema_version: RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION,
            evaluation_mode: RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE,
            command_id: "command-001".into(),
            command_sha256: COMMAND_SHA256.into(),
            attempt_id: "attempt-001".into(),
            target_id: "runner-1".into(),
            disposition_kind: "completed".into(),
            observed_at_ms: 300,
            receipt_valid: true,
            preview_only: true,
            uncertain: false,
            reconciliation_required: false,
            manual_review_required: false,
            automatic_retry: false,
            follow_up: "none".into(),
            authority: RunnerTerminalReceiptAuthority::default(),
        },
    }
}

#[test]
fn session_runner_receipt_fixture_is_canonical_and_preview_only() {
    let observation = observe_session_runner_receipt(request()).expect("valid session receipt");
    observation.validate().expect("valid canonical observation");
    let expected: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let actual = serde_json::to_value(observation).expect("observation JSON");
    assert_eq!(actual, expected);
    assert!(actual["selected_target_id"].is_null());
    assert_eq!(actual["authority"]["execution_authorized"], false);
}

#[test]
fn session_runner_receipt_rejects_confused_or_authoritative_values() {
    let mut foreign_prompt = request();
    foreign_prompt.prompt_id = "prompt-foreign".into();
    assert_eq!(
        observe_session_runner_receipt(foreign_prompt),
        Err(SessionRunnerReceiptError::InvalidBinding)
    );

    let mut foreign_target = request();
    foreign_target.receipt_observation.target_id = "runner-foreign".into();
    assert_eq!(
        observe_session_runner_receipt(foreign_target),
        Err(SessionRunnerReceiptError::InvalidBinding)
    );

    let mut selected = request();
    selected.runner_execution_intent.selected_target_id = Some("runner-1".into());
    assert_eq!(
        observe_session_runner_receipt(selected),
        Err(SessionRunnerReceiptError::InvalidBinding)
    );

    let mut authoritative = request();
    authoritative
        .receipt_observation
        .authority
        .dispatch_performed = true;
    assert_eq!(
        observe_session_runner_receipt(authoritative),
        Err(SessionRunnerReceiptError::InvalidBinding)
    );

    let mut unknown_disposition = request();
    unknown_disposition.receipt_observation.disposition_kind = "unknown".into();
    assert_eq!(
        observe_session_runner_receipt(unknown_disposition),
        Err(SessionRunnerReceiptError::InvalidBinding)
    );

    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["unexpected"] = json!(true);
    assert!(serde_json::from_value::<SessionRunnerReceiptObservation>(unknown).is_err());
}

#[test]
fn uncertain_session_runner_receipt_requires_manual_reconciliation() {
    let mut input = request();
    input.receipt_observation.disposition_kind = "uncertain".into();
    input.receipt_observation.uncertain = true;
    input.receipt_observation.reconciliation_required = true;
    input.receipt_observation.manual_review_required = true;
    input.receipt_observation.follow_up = "reconciliation_manual".into();
    let observation = observe_session_runner_receipt(input).expect("uncertain receipt");
    assert!(observation.receipt_observation.uncertain);
    assert!(observation.receipt_observation.reconciliation_required);
    assert!(observation.receipt_observation.manual_review_required);
    assert!(!observation.receipt_observation.automatic_retry);
    assert_eq!(
        observation.receipt_observation.follow_up,
        "reconciliation_manual"
    );
}
