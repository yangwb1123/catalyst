use super::{
    lease::LeaseGrant,
    runner_command::{
        RunnerCommand, RunnerTerminalReceipt, RunnerTerminalReceiptAuthority,
        observe_runner_terminal_receipt,
    },
};
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-runner-terminal-receipt-vectors-v1.json"
);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptVectorsFixture {
    schema_version: String,
    evaluation_mode: String,
    authority: RunnerTerminalReceiptAuthority,
    vectors: Vec<ReceiptVector>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptVector {
    name: String,
    grant: LeaseGrant,
    command: RunnerCommand,
    receipt: RunnerTerminalReceipt,
    expected: ReceiptExpectation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReceiptExpectation {
    command_sha256: String,
    disposition_kind: String,
    receipt_valid: bool,
    uncertain: bool,
    reconciliation_required: bool,
    manual_review_required: bool,
    automatic_retry: bool,
    follow_up: String,
}

#[test]
fn runner_terminal_receipt_vectors_match_all_outcomes() {
    let fixture: ReceiptVectorsFixture = serde_json::from_str(FIXTURE).expect("strict fixture");
    assert_eq!(
        fixture.schema_version,
        "forge.runner-terminal-receipt-vectors/v1"
    );
    assert_eq!(
        fixture.evaluation_mode,
        "pure_runner_terminal_receipt_vectors_only"
    );
    assert_eq!(fixture.authority, RunnerTerminalReceiptAuthority::default());
    assert_eq!(fixture.vectors.len(), 3);

    for vector in fixture.vectors {
        assert!(!vector.name.is_empty());
        assert_eq!(vector.grant.validate(), Ok(()), "grant {}", vector.name);
        assert_eq!(
            vector.command.command_sha256().expect("command digest"),
            vector.expected.command_sha256,
            "digest {}",
            vector.name
        );
        assert_eq!(
            vector.receipt.command_sha256,
            vector.expected.command_sha256
        );
        let observation =
            observe_runner_terminal_receipt(&vector.command, &vector.grant, &vector.receipt)
                .expect("terminal observation");
        assert_eq!(
            observation.disposition_kind,
            vector.expected.disposition_kind
        );
        assert_eq!(observation.receipt_valid, vector.expected.receipt_valid);
        assert_eq!(observation.uncertain, vector.expected.uncertain);
        assert_eq!(
            observation.reconciliation_required,
            vector.expected.reconciliation_required
        );
        assert_eq!(
            observation.manual_review_required,
            vector.expected.manual_review_required
        );
        assert_eq!(observation.automatic_retry, vector.expected.automatic_retry);
        assert_eq!(observation.follow_up, vector.expected.follow_up);
        assert!(observation.preview_only);
        assert_eq!(
            observation.authority,
            RunnerTerminalReceiptAuthority::default()
        );
    }
}

#[test]
fn runner_terminal_receipt_vectors_reject_duplicate_unknown_and_trailing_values() {
    let duplicate = FIXTURE.replace(
        "\"schema_version\": \"forge.runner-terminal-receipt-vectors/v1\",",
        "\"schema_version\": \"forge.runner-terminal-receipt-vectors/v1\", \"schema_version\": \"forge.runner-terminal-receipt-vectors/v1\",",
    );
    assert!(serde_json::from_str::<ReceiptVectorsFixture>(&duplicate).is_err());

    let unknown = FIXTURE.replace(
        "\"evaluation_mode\": \"pure_runner_terminal_receipt_vectors_only\",",
        "\"evaluation_mode\": \"pure_runner_terminal_receipt_vectors_only\", \"unexpected\": true,",
    );
    assert!(serde_json::from_str::<ReceiptVectorsFixture>(&unknown).is_err());
    assert!(serde_json::from_str::<ReceiptVectorsFixture>(&format!("{FIXTURE} {{}}")).is_err());
}
