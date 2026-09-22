use super::{
    MAX_RUNNER_COMMAND_OUTPUT_BYTES, RunnerCommand, RunnerCommandError, RunnerTerminalReceipt,
    observe_runner_terminal_receipt,
};
use crate::execution::lease::{LeaseGrant, TerminalDisposition};
use serde::Deserialize;

const FIXTURE: &str = include_str!(
    "../../../../../docs/contracts/fixtures/forge-runner-command-terminal-receipt-v1.json"
);

const TTL: u64 = 10_000;

fn grant() -> LeaseGrant {
    LeaseGrant::issue(
        "attempt-1".into(),
        "runner-1".into(),
        1,
        "fence-1".into(),
        100,
        TTL,
    )
    .expect("valid grant")
}

fn command() -> RunnerCommand {
    RunnerCommand {
        v: super::RUNNER_COMMAND_ABI_VERSION,
        command_id: "command-1".into(),
        lease_proof: grant().proof(),
        idempotency_key: "attempt-1-command-1".into(),
        workspace_ref: "workspace-1".into(),
        argv: vec!["forge-task".into(), "--check".into(), String::new()],
        timeout_ms: 5_000,
        max_output_bytes: 65_536,
    }
}

#[test]
fn command_is_direct_argv_and_has_stable_digest() {
    let command = command();
    command.validate().expect("bounded command");
    assert_eq!(
        command.command_sha256().expect("digest"),
        "aeb0076a9e539d6308ddf0ee55845a7601ecb23b3cc1c0bc5f1192517c2f6eb7"
    );
    let mut changed = command.clone();
    changed.argv[1] = "--format".into();
    assert_ne!(
        command.command_sha256().expect("digest"),
        changed.command_sha256().expect("changed digest")
    );
}

#[test]
fn terminal_receipt_binds_command_digest_and_current_lease() {
    let command = command();
    let receipt = RunnerTerminalReceipt::from_command(
        &command,
        TerminalDisposition::Completed {
            receipt_sha256: "a".repeat(64),
        },
        200,
    )
    .expect("receipt");
    receipt
        .validate_against(&command, &grant())
        .expect("current lease accepts receipt");

    let mut changed = receipt.clone();
    changed.command_sha256 = "b".repeat(64);
    assert_eq!(
        changed.validate_against(&command, &grant()),
        Err(RunnerCommandError::CommandDigestMismatch)
    );
}

#[test]
fn terminal_receipt_observation_is_preview_only_and_marks_uncertainty() {
    let command = command();
    let completed = RunnerTerminalReceipt::from_command(
        &command,
        TerminalDisposition::Completed {
            receipt_sha256: "a".repeat(64),
        },
        200,
    )
    .expect("completed receipt");
    let observation = observe_runner_terminal_receipt(&command, &grant(), &completed)
        .expect("completed observation");
    assert_eq!(
        observation.schema_version,
        "forge.runner-command-terminal-receipt/v1"
    );
    assert_eq!(observation.disposition_kind, "completed");
    assert!(observation.receipt_valid && observation.preview_only);
    assert!(!observation.uncertain && !observation.reconciliation_required);
    assert_eq!(observation.follow_up, "none");
    assert_eq!(observation.authority, Default::default());

    let uncertain = RunnerTerminalReceipt::from_command(
        &command,
        TerminalDisposition::Uncertain {
            reason: "transport ended after effect boundary".into(),
        },
        200,
    )
    .expect("uncertain receipt");
    let observation = observe_runner_terminal_receipt(&command, &grant(), &uncertain)
        .expect("uncertain observation");
    assert!(observation.uncertain);
    assert!(observation.reconciliation_required && observation.manual_review_required);
    assert!(!observation.automatic_retry);
    assert_eq!(observation.follow_up, "reconciliation_manual");
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractFixture {
    schema_version: String,
    evaluation_mode: String,
    authority: Authority,
    grant: LeaseGrant,
    command: RunnerCommand,
    receipt: RunnerTerminalReceipt,
    expected: Expected,
}

#[derive(Deserialize)]
#[allow(clippy::struct_excessive_bools)]
#[serde(deny_unknown_fields)]
struct Authority {
    device_identity_verified: bool,
    command_persisted: bool,
    reservation_created: bool,
    execution_authorized: bool,
    dispatch_performed: bool,
    audit_published: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    command_sha256: String,
    receipt_valid: bool,
}

#[test]
fn runner_command_contract_fixture() {
    let fixture: ContractFixture = serde_json::from_str(FIXTURE).expect("strict fixture");
    assert_eq!(
        fixture.schema_version,
        "forge.runner-command-terminal-receipt/v1"
    );
    assert_eq!(fixture.evaluation_mode, "pure_runner_command_receipt_only");
    assert!(!fixture.authority.device_identity_verified);
    assert!(!fixture.authority.command_persisted);
    assert!(!fixture.authority.reservation_created);
    assert!(!fixture.authority.execution_authorized);
    assert!(!fixture.authority.dispatch_performed);
    assert!(!fixture.authority.audit_published);
    fixture.grant.validate().expect("valid lease fixture");
    assert_eq!(
        fixture.command.command_sha256().expect("command digest"),
        fixture.expected.command_sha256
    );
    assert_eq!(
        fixture
            .receipt
            .validate_against(&fixture.command, &fixture.grant)
            .is_ok(),
        fixture.expected.receipt_valid
    );
}

#[test]
fn stale_or_foreign_proofs_fail_closed() {
    let command = command();
    let receipt = RunnerTerminalReceipt::from_command(
        &command,
        TerminalDisposition::Uncertain {
            reason: "transport ended after effect boundary".into(),
        },
        200,
    )
    .expect("receipt");
    let renewed = grant()
        .renew(1_000, "fence-2".into(), TTL)
        .expect("renewal");
    assert!(matches!(
        receipt.validate_against(&command, &renewed),
        Err(RunnerCommandError::Lease(_))
    ));

    let mut foreign = command.clone();
    foreign.lease_proof.target_id = "runner-2".into();
    assert!(foreign.validate().is_ok());
    assert!(matches!(
        receipt.validate_against(&foreign, &grant()),
        Err(RunnerCommandError::CommandDigestMismatch
            | RunnerCommandError::ProofMismatch
            | RunnerCommandError::Lease(_))
    ));
}

#[test]
fn command_bounds_reject_shell_controls_and_oversized_output() {
    let mut invalid_argument = command();
    invalid_argument.argv[1] = "--bad\narg".into();
    assert_eq!(
        invalid_argument.validate(),
        Err(RunnerCommandError::InvalidArgument)
    );
    let mut oversized_output = command();
    oversized_output.max_output_bytes = MAX_RUNNER_COMMAND_OUTPUT_BYTES + 1;
    assert_eq!(
        oversized_output.validate(),
        Err(RunnerCommandError::InvalidOutputLimit)
    );
}
