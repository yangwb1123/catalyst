use std::{fs, io};

use forge_runtime_domain::{
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodePredecessorOutcome,
    GroupAgentScheduledNodePredecessorReceipt, GroupAgentScheduledNodeTerminalArtifactKind,
    GroupAgentScheduledNodeTerminalReceipt, MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES,
};
use tempfile::NamedTempFile;

use super::{read_predecessor_receipt, validate_supplied_receipts};

#[test]
fn read_predecessor_receipt_rejects_garbage_and_noncanonical_json() {
    let garbage = write_source(b"not-json");
    assert_receipt_error(&garbage, "invalid or noncanonical predecessor receipt");

    let canonical = terminal_receipt("frontend", GroupAgentNodeTerminalOutcome::Completed)
        .canonical_json()
        .expect("canonical receipt");
    let noncanonical = write_source(format!("{canonical}\n").as_bytes());
    assert_receipt_error(&noncanonical, "invalid or noncanonical predecessor receipt");
}

#[test]
fn read_predecessor_receipt_rejects_exact_bound_overflow() {
    let source = write_source(&vec![
        b' ';
        MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES + 1
    ]);
    assert_receipt_error(&source, "predecessor receipt exceeds its byte limit");
}

#[test]
fn read_predecessor_receipt_rejects_resigned_artifact_identity_mismatch() {
    let mut forged = terminal_receipt("frontend", GroupAgentNodeTerminalOutcome::Completed);
    forged.artifact_id = format!("scheduled-node-terminal-artifact-{}", "d".repeat(64));
    sign_receipt(&mut forged);
    let source = write_source(
        forged
            .canonical_json()
            .expect("canonical forged receipt")
            .as_bytes(),
    );

    assert_receipt_error(&source, "invalid or noncanonical predecessor receipt");
}

#[test]
fn supplied_completed_receipt_must_match_every_bound_identity() {
    let supplied = terminal_receipt("frontend", GroupAgentNodeTerminalOutcome::Completed);
    let expected = compact_receipt(
        &supplied,
        GroupAgentScheduledNodePredecessorOutcome::Completed,
    );
    assert!(
        validate_supplied_receipts("graph-run", &[expected.clone()], &[supplied.clone()]).is_ok()
    );

    let mut mismatched = expected;
    mismatched.terminal_receipt_sha256 = "f".repeat(64);
    assert!(validate_supplied_receipts("graph-run", &[mismatched], &[supplied]).is_err());
}

#[test]
fn supplied_receipts_can_arrive_in_any_order() {
    let frontend = terminal_receipt("frontend", GroupAgentNodeTerminalOutcome::Completed);
    let backend = terminal_receipt("backend", GroupAgentNodeTerminalOutcome::Completed);
    let expected = [
        compact_receipt(
            &frontend,
            GroupAgentScheduledNodePredecessorOutcome::Completed,
        ),
        compact_receipt(
            &backend,
            GroupAgentScheduledNodePredecessorOutcome::Completed,
        ),
    ];

    assert!(validate_supplied_receipts("graph-run", &expected, &[backend, frontend]).is_ok());
}

#[test]
fn duplicate_missing_and_unrelated_supplied_receipts_are_rejected() {
    let frontend = terminal_receipt("frontend", GroupAgentNodeTerminalOutcome::Completed);
    let backend = terminal_receipt("backend", GroupAgentNodeTerminalOutcome::Completed);
    let unrelated = terminal_receipt("unrelated", GroupAgentNodeTerminalOutcome::Completed);
    let expected = [
        compact_receipt(
            &frontend,
            GroupAgentScheduledNodePredecessorOutcome::Completed,
        ),
        compact_receipt(
            &backend,
            GroupAgentScheduledNodePredecessorOutcome::Completed,
        ),
    ];

    assert!(
        validate_supplied_receipts("graph-run", &expected, &[
            frontend.clone(),
            frontend.clone()
        ],)
        .is_err()
    );
    assert!(validate_supplied_receipts("graph-run", &expected, &[frontend.clone()]).is_err());
    assert!(validate_supplied_receipts("graph-run", &expected, &[frontend, unrelated]).is_err());
}

#[test]
fn same_wave_candidate_accepts_empty_receipt_sets() {
    assert!(validate_supplied_receipts("graph-run", &[], &[]).is_ok());
}

#[test]
fn failed_receipts_cannot_advance_a_successor() {
    let cases = [
        (
            GroupAgentNodeTerminalOutcome::Failed,
            GroupAgentScheduledNodePredecessorOutcome::Failed,
        ),
        (
            GroupAgentNodeTerminalOutcome::FailedUncertain,
            GroupAgentScheduledNodePredecessorOutcome::FailedUncertain,
        ),
    ];
    for (terminal_outcome, predecessor_outcome) in cases {
        let supplied = terminal_receipt("frontend", terminal_outcome);
        let expected = compact_receipt(&supplied, predecessor_outcome);
        assert!(
            validate_supplied_receipts("graph-run", &[expected], &[supplied]).is_err(),
            "{terminal_outcome:?} must not advance a successor"
        );
    }
}

fn compact_receipt(
    receipt: &GroupAgentScheduledNodeTerminalReceipt,
    node_outcome: GroupAgentScheduledNodePredecessorOutcome,
) -> GroupAgentScheduledNodePredecessorReceipt {
    GroupAgentScheduledNodePredecessorReceipt {
        predecessor_node_id: receipt.node_id.clone(),
        predecessor_attempt: receipt.attempt,
        terminal_event_seq: 0,
        terminal_event_sha256: String::new(),
        terminal_receipt_id: receipt.receipt_id.clone(),
        terminal_receipt_sha256: receipt.receipt_sha256.clone(),
        node_outcome,
        provider_request_id: receipt.provider_request_id.clone(),
        dispatch_id: receipt.dispatch_id.clone(),
    }
}

fn terminal_receipt(
    node_id: &str,
    node_outcome: GroupAgentNodeTerminalOutcome,
) -> GroupAgentScheduledNodeTerminalReceipt {
    let artifact_kind = if node_outcome == GroupAgentNodeTerminalOutcome::Completed {
        GroupAgentScheduledNodeTerminalArtifactKind::Result
    } else {
        GroupAgentScheduledNodeTerminalArtifactKind::Uncertainty
    };
    let mut receipt = GroupAgentScheduledNodeTerminalReceipt {
        v: 1,
        scheduler_protocol_version: 1,
        terminal_receipt_protocol_version: 1,
        terminal_control_sha256: "a".repeat(64),
        graph_run_id: "graph-run".into(),
        graph_id: "graph".into(),
        node_id: node_id.into(),
        attempt: 1,
        dispatch_id: format!("dispatch-{node_id}"),
        provider_request_id: format!("scheduled-node-provider-request-{node_id}"),
        project_lane_sha256: "b".repeat(64),
        artifact_kind,
        artifact_id: format!("scheduled-node-terminal-artifact-{}", "c".repeat(64)),
        artifact_sha256: "c".repeat(64),
        node_outcome,
        retry_authorized: false,
        lane_release_authorized: true,
        successor_advance_authorized: false,
        receipt_id: String::new(),
        receipt_sha256: String::new(),
    };
    sign_receipt(&mut receipt);
    receipt.validate().expect("valid receipt fixture");
    receipt
}

fn sign_receipt(receipt: &mut GroupAgentScheduledNodeTerminalReceipt) {
    let digest = receipt.expected_sha256().expect("receipt digest");
    receipt.receipt_id = format!("scheduled-node-terminal-receipt-{digest}");
    receipt.receipt_sha256 = digest;
}

fn write_source(bytes: &[u8]) -> NamedTempFile {
    let source = NamedTempFile::new().expect("temporary receipt source");
    fs::write(source.path(), bytes).expect("write receipt source");
    source
}

fn assert_receipt_error(source: &NamedTempFile, expected_message: &str) {
    let error = read_predecessor_receipt(source.path().to_str().expect("UTF-8 temp path"))
        .expect_err("receipt source must be rejected");
    let error = error.downcast::<io::Error>().expect("input error");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(error.to_string(), expected_message);
}
