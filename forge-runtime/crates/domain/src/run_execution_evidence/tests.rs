use super::*;
use crate::execution::runner_command::RunnerTerminalReceiptAuthority;
use crate::execution::runner_execution_intent::RunnerExecutionOwner;
use crate::execution::session_runner_receipt::{
    SessionRunnerReceiptAuthority, SessionRunnerReceiptObservation,
    SessionRunnerTerminalReceiptObservation,
};
use crate::run_observed::{RunObservedInput, observe_run};
use crate::{ConversationOwner, OwnedRunStatus, OwnedRunSummary};
use serde_json::{Value, json};

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/forge-run-execution-evidence-v1.json");
const COMMAND_SHA256: &str = "42ed02a535113450e6f2cc757fb9b4e2cce6143724274191bbae159e9ea8de7a";

#[test]
fn run_execution_evidence_matches_fixture_and_stays_content_free() {
    let fixture: RunExecutionEvidence =
        serde_json::from_str(FIXTURE).expect("strict execution evidence fixture");
    fixture
        .validate()
        .expect("canonical execution evidence fixture");
    assert_eq!(fixture.api_version, RUN_EXECUTION_EVIDENCE_SCHEMA_VERSION);
    assert_eq!(
        fixture.evaluation_mode,
        RUN_EXECUTION_EVIDENCE_EVALUATION_MODE
    );
    assert_eq!(fixture.authority, RunExecutionEvidenceAuthority::default());
    assert!(fixture.metadata_observed);
    assert!(!fixture.content_included);

    let observed = observe_run_execution_evidence(input()).expect("valid execution evidence");
    let expected: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    let actual = serde_json::to_value(observed).expect("evidence JSON");
    assert_eq!(actual, expected);
    assert_eq!(actual["content_included"], false);
    assert_eq!(actual["authority"]["execution_authorized"], false);
}

#[test]
fn run_execution_evidence_rejects_confused_or_authoritative_values() {
    let mut foreign_owner = input();
    foreign_owner.run.owner_ref = "a".repeat(64);
    assert_eq!(
        observe_run_execution_evidence(foreign_owner),
        Err(RunExecutionEvidenceError::OwnerMismatch)
    );

    let mut foreign_run = input();
    foreign_run.run.run_id = "run-foreign".into();
    assert_eq!(
        observe_run_execution_evidence(foreign_run),
        Err(RunExecutionEvidenceError::BindingMismatch)
    );

    let mut authoritative = input();
    authoritative.run.authority.execution_authorized = true;
    assert_eq!(
        observe_run_execution_evidence(authoritative),
        Err(RunExecutionEvidenceError::InvalidRun)
    );

    let mut authoritative_receipt = input();
    authoritative_receipt.receipt.authority.receipt_persisted = true;
    assert_eq!(
        observe_run_execution_evidence(authoritative_receipt),
        Err(RunExecutionEvidenceError::InvalidReceipt)
    );

    let mut unknown: Value = serde_json::from_str(FIXTURE).expect("fixture JSON");
    unknown["prompt"] = json!("raw prompt");
    assert!(serde_json::from_value::<RunExecutionEvidence>(unknown).is_err());
}

#[test]
fn run_execution_evidence_wire_validation_rejects_authority_content_and_binding_mutations() {
    let mut fixture: RunExecutionEvidence = serde_json::from_str(FIXTURE).unwrap();
    fixture.authority.dispatch_performed = true;
    assert_eq!(
        fixture.validate(),
        Err(RunExecutionEvidenceError::InvalidEvidence)
    );

    let mut fixture: RunExecutionEvidence = serde_json::from_str(FIXTURE).unwrap();
    fixture.authority.dispatch_performed = false;
    fixture.content_included = true;
    assert_eq!(
        fixture.validate(),
        Err(RunExecutionEvidenceError::InvalidEvidence)
    );

    let mut fixture: RunExecutionEvidence = serde_json::from_str(FIXTURE).unwrap();
    fixture.authority.dispatch_performed = false;
    fixture.disposition_kind = "uncertain".into();
    assert_eq!(
        fixture.validate(),
        Err(RunExecutionEvidenceError::InvalidEvidence)
    );
}

#[test]
fn uncertain_receipt_requires_manual_reconciliation() {
    let mut input = input();
    input.receipt.receipt_observation.disposition_kind = "uncertain".into();
    input.receipt.receipt_observation.uncertain = true;
    input.receipt.receipt_observation.reconciliation_required = true;
    input.receipt.receipt_observation.manual_review_required = true;
    input.receipt.receipt_observation.follow_up = "reconciliation_manual".into();
    let evidence = observe_run_execution_evidence(input).expect("uncertain evidence");
    assert!(evidence.uncertain);
    assert!(evidence.reconciliation_required);
}

#[allow(clippy::too_many_lines)]
fn input() -> RunExecutionEvidenceInput {
    let owner = ConversationOwner {
        issuer: "https://id.example".into(),
        subject: "user-1".into(),
        tenant_id: "tenant-1".into(),
    };
    let run = observe_run(RunObservedInput {
        owner,
        conversation_id: "conversation-001".into(),
        run: OwnedRunSummary {
            run_id: "run-001".into(),
            prompt_id: "prompt-001".into(),
            created_at_ms: 200,
            latest_sequence: 5,
            status: OwnedRunStatus::Nonterminal,
        },
    })
    .expect("Run observation");
    let receipt_owner = RunnerExecutionOwner {
        issuer: "https://id.example".into(),
        subject: "user-1".into(),
        tenant_id: "tenant-1".into(),
    };
    RunExecutionEvidenceInput {
        run,
        receipt: SessionRunnerReceiptObservation {
            schema_version: "forge.session-runner-receipt-observation/v1".into(),
            evaluation_mode: "pure_session_runner_receipt_binding_only".into(),
            owner: receipt_owner.clone(),
            conversation_id: "conversation-001".into(),
            prompt_id: "prompt-001".into(),
            run_id: "run-001".into(),
            receipt_observation: SessionRunnerTerminalReceiptObservation {
                schema_version: "forge.runner-command-terminal-receipt/v1".into(),
                evaluation_mode: "pure_runner_command_receipt_only".into(),
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
            prompt_run_binding_valid: true,
            receipt_binding_valid: true,
            preview_only: true,
            selected_target_id: None,
            authority: SessionRunnerReceiptAuthority::default(),
        },
    }
}
