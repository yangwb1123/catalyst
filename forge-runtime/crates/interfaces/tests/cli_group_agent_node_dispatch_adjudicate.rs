#![allow(dead_code)]

use std::{fs, process::Command};

use tempfile::TempDir;

mod group_agent_graph_support;
mod group_agent_node_dispatch_execution_cli_support;

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION, GroupAgentGraphRunEventKind, GroupAgentGraphRunStatus,
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeTerminalArtifactKind, GroupAgentNodeTerminalClassification,
    group_agent_node_dispatch_authorization_id,
};
use group_agent_graph_support::path_text;
use group_agent_node_dispatch_execution_cli_support::{CREDENTIAL_SECRET, Fixture, TASK_SECRET};

const FULL_FLAGS: [&str; 14] = [
    "group",
    "graph",
    "run",
    "dispatch",
    "adjudicate",
    "run-1",
    "--authorization",
    "a.json",
    "--pricing",
    "p.json",
    "--core-bin",
    "/bin/true",
    "--core-bin-sha256",
    "0000000000000000000000000000000000000000000000000000000000000000",
];

#[test]
fn help_and_missing_flags_expose_adjudicate_boundaries_without_state_access() {
    let help = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .arg("help")
        .output()
        .expect("run help");
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("UTF-8 help");
    assert!(help.contains(
        "group graph run dispatch adjudicate GRAPH_RUN_ID\n                --authorization FILE|- --pricing FILE|-"
    ));
    assert!(help.contains("--core-bin ABSOLUTE_FILE --core-bin-sha256 SHA256"));
    assert!(help.contains(
        "A\n  hard-crash-quarantined claim can be remedied with `group graph run dispatch\n  adjudicate`"
    ));
    assert!(help.contains("with `group graph run dispatch adjudicate`. Prepare a new analysis"));

    let state = TempDir::new().expect("state directory");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--state-dir", path_text(state.path()), "--json"])
        .args(["group", "graph", "run", "dispatch", "adjudicate", "run-1"])
        .output()
        .expect("run invalid adjudicate");
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("dispatch adjudicate requires --authorization FILE|-"));
    assert!(error.contains("usage:"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

// run_adjudicate spawns the CLI with the adjudicate verb and extra args.
fn run_adjudicate(state: &TempDir, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--state-dir", path_text(state.path()), "--json"])
        .args(["group", "graph", "run", "dispatch", "adjudicate", "run-1"])
        .args(extra)
        .output()
        .expect("run adjudicate")
}

#[test]
fn parse_level_rejections_cover_dual_stdin_idempotency_key_and_execute_only_flags() {
    let state = TempDir::new().expect("state directory");
    let dual_stdin = run_adjudicate(&state, &["--authorization", "-", "--pricing", "-"]);
    assert_eq!(dual_stdin.status.code(), Some(2));
    let error = String::from_utf8_lossy(&dual_stdin.stderr);
    assert!(
        error.contains("dispatch adjudicate accepts standard input for only one artifact"),
        "{error}"
    );

    let idempotency = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            path_text(state.path()),
            "--idempotency-key",
            "adjudicate-key",
            "--json",
        ])
        .args(FULL_FLAGS)
        .output()
        .expect("run idempotency-key adjudicate");
    assert_eq!(idempotency.status.code(), Some(2));
    let error = String::from_utf8_lossy(&idempotency.stderr);
    assert!(
        error.contains("GRAPH_RUN_ID owns the single dispatch claim"),
        "{error}"
    );

    let consent = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--state-dir", path_text(state.path()), "--json"])
        .args(FULL_FLAGS)
        .arg("--confirm-off-machine")
        .output()
        .expect("run consent adjudicate");
    assert_eq!(consent.status.code(), Some(2));
    let error = String::from_utf8_lossy(&consent.stderr);
    assert!(
        error.contains("dispatch adjudicate does not accept --confirm-off-machine"),
        "{error}"
    );
}

#[test]
fn adjudication_remedies_a_stranded_v4_claim_with_the_real_pinned_core() {
    let fixture = Fixture::new();
    fixture.strand_v4_with_local_provider();
    let journal_before = fixture.lifecycle_inspection().graph_run.run.journal_bytes;

    let output = fixture.adjudicate(None);

    assert!(
        output.status.success(),
        "adjudication failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(json["graph_status"], "failed_uncertain");
    assert_eq!(json["classification"], "hard_crash");
    assert_eq!(json["provider_poll_started"], false);
    assert_eq!(json["terminal_seen"], false);
    assert_eq!(json["stream_eof_seen"], false);
    assert_eq!(json["lane_active"], false);
    assert_eq!(json["retry_authorized"], false);
    assert_eq!(json["dispatch_performed_this_invocation"], false);
    assert_eq!(json["database_written_this_invocation"], true);
    assert!(json.get("result_text").is_none());

    // DB post-state: the single terminalize CAS committed a deterministic
    // v5 failed_uncertain terminal with the lane ownership row released.
    let inspection = fixture.lifecycle_inspection();
    assert_terminal_run(&inspection, journal_before);
    fixture.assert_workspace_unchanged();
}

/// Direct run-level + evidence post-state assertions (design A2 / §7.6): the
/// CAS result is locally self-evident, not only transitively verified.
fn assert_terminal_run(inspection: &GroupAgentNodeLifecycleInspection, journal_before: usize) {
    assert_eq!(
        inspection.graph_run.run.status,
        GroupAgentGraphRunStatus::FailedUncertain
    );
    assert_eq!(
        inspection.graph_run.run.v,
        GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION
    );
    assert_eq!(inspection.graph_run.run.last_event_seq, 5);
    assert_eq!(inspection.graph_run.events.len(), 5);
    assert_eq!(
        inspection.graph_run.run.journal_bytes,
        journal_before + inspection.graph_run.event_jsons[4].len()
    );
    assert!(inspection.active_lane.is_none());

    assert_terminal_evidence(inspection);
}

fn assert_terminal_evidence(inspection: &GroupAgentNodeLifecycleInspection) {
    let artifact = inspection.artifact.as_ref().expect("hard-crash artifact");
    assert_eq!(
        artifact.artifact_kind,
        GroupAgentNodeTerminalArtifactKind::Uncertainty
    );
    assert_eq!(
        artifact.classification,
        GroupAgentNodeTerminalClassification::HardCrash
    );
    assert!(
        !artifact.provider_poll_started && !artifact.terminal_seen && !artifact.stream_eof_seen
    );
    assert!(!artifact.usage_observed && !artifact.actual_cost_calculated);
    assert!(!artifact.retry_authorized);
    assert!(artifact.created_at_ms >= inspection.claim.released_at_ms);

    let receipt = inspection.terminal_receipt.as_ref().expect("Core receipt");
    assert_eq!(
        receipt.graph_status,
        GroupAgentGraphRunStatus::FailedUncertain
    );
    assert_eq!(receipt.expected_last_event_seq, 4);
    assert!(receipt.lane_release_authorized);
    assert!(!receipt.retry_authorized);

    let GroupAgentGraphRunEventKind::NodeLifecycleTerminalized {
        retry_authorized,
        lane_released,
        artifact_id,
        terminal_receipt_id,
        ..
    } = &inspection.graph_run.events[4].kind
    else {
        panic!("seq-5 event must be a terminalized lifecycle");
    };
    assert!(!retry_authorized);
    assert!(lane_released);
    assert_eq!(artifact_id, &artifact.artifact_id);
    assert_eq!(terminal_receipt_id, &receipt.receipt_id);
}

#[test]
fn adjudication_is_effect_free_with_an_absent_or_poisoned_credential() {
    let absent = {
        let fixture = Fixture::new();
        fixture.strand_v4_with_local_provider();
        let output = fixture.adjudicate(None);
        assert!(
            output.status.success(),
            "keyless adjudication failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let inspection = fixture.lifecycle_inspection();
        assert_eq!(
            inspection.graph_run.run.status,
            GroupAgentGraphRunStatus::FailedUncertain
        );
        output
    };

    let poisoned = {
        let fixture = Fixture::new();
        fixture.strand_v4_with_local_provider();
        let output = fixture.adjudicate(Some(CREDENTIAL_SECRET));
        assert!(
            output.status.success(),
            "poisoned-key adjudication failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fixture.lifecycle_inspection().graph_run.run.status,
            GroupAgentGraphRunStatus::FailedUncertain
        );
        output
    };

    // The key is never read: absent and poisoned runs produce identical output
    // modulo the per-fixture run identity (graph_run_id embeds pid/time), are
    // silent on stderr, and commit the same terminal state.
    let absent_json: serde_json::Value = serde_json::from_slice(&absent.stdout).expect("JSON");
    let poisoned_json: serde_json::Value = serde_json::from_slice(&poisoned.stdout).expect("JSON");
    assert_eq!(normalized(&absent_json), normalized(&poisoned_json));
    assert!(absent.stderr.is_empty());
    assert!(poisoned.stderr.is_empty());
    assert!(!String::from_utf8_lossy(&poisoned.stdout).contains(CREDENTIAL_SECRET));
    assert!(!String::from_utf8_lossy(&poisoned.stdout).contains(TASK_SECRET));
}

fn normalized(value: &serde_json::Value) -> serde_json::Value {
    let mut value = value.clone();
    value["graph_run_id"] = "graph-run-normalized".into();
    value
}

#[test]
fn readjudication_and_digest_mismatch_are_refused_with_zero_mutation() {
    let fixture = Fixture::new();
    fixture.strand_v4_with_local_provider();

    // Wrong-but-valid authorization body → DigestMismatch pre-bridge.
    let original = fs::read(&fixture.authorization_path).expect("original authorization");
    let different = different_valid_authorization(&original);
    fixture.replace_authorization(different.as_bytes());
    let state_before = fixture.state_bytes();
    let mismatch = fixture.adjudicate(None);
    assert!(!mismatch.status.success());
    assert!(mismatch.stdout.is_empty());
    let error = String::from_utf8_lossy(&mismatch.stderr);
    assert!(
        error
            .contains("adjudication refused: authorization body does not match the claimed digest"),
        "{error}"
    );
    assert!(!error.contains("Core refused"));
    assert!(!error.contains("quarantined; resend is forbidden"));
    assert!(!error.contains(CREDENTIAL_SECRET));
    assert_eq!(fixture.state_bytes(), state_before);
    fixture.replace_authorization(&original);

    // Correct bodies → the stranded claim is adjudicated.
    let adjudicated = fixture.adjudicate(None);
    assert!(
        adjudicated.status.success(),
        "adjudication failed: {}",
        String::from_utf8_lossy(&adjudicated.stderr)
    );

    // Re-adjudication of the now-terminal claim → NotStranded, zero mutation.
    let state_before = fixture.state_bytes();
    let reentry = fixture.adjudicate(None);
    assert!(!reentry.status.success());
    assert!(reentry.stdout.is_empty());
    let error = String::from_utf8_lossy(&reentry.stderr);
    assert!(
        error.contains(&format!(
            "adjudication refused: {} is not a stranded hard-crash claim (status=failed_uncertain",
            fixture.graph_run_id
        )),
        "{error}"
    );
    assert!(error.contains("artifact_present=true, receipt_present=true"));
    assert_eq!(fixture.state_bytes(), state_before);
    fixture.assert_workspace_unchanged();
}

#[test]
fn execute_reentry_after_adjudication_returns_the_stored_inspection() {
    let fixture = Fixture::new();
    fixture.strand_v4_with_local_provider();
    let adjudicated = fixture.adjudicate(None);
    assert!(
        adjudicated.status.success(),
        "adjudication failed: {}",
        String::from_utf8_lossy(&adjudicated.stderr)
    );
    let stored = fixture.lifecycle_inspection();
    assert_eq!(
        stored.graph_run.run.status,
        GroupAgentGraphRunStatus::FailedUncertain
    );

    // Re-entry with deliberately different bodies must return the stored
    // terminal inspection, never resend, and never even read the new bodies.
    fixture.replace_authorization(b"different-authorization-must-not-be-read");
    fixture.replace_pricing(b"different-pricing-must-not-be-read");
    let state_before = fixture.state_bytes();
    let reentry = fixture.execute(true, Some(CREDENTIAL_SECRET));
    assert!(
        reentry.status.success(),
        "execute re-entry failed: {}",
        String::from_utf8_lossy(&reentry.stderr)
    );
    assert_stored_reentry_output(&reentry.stdout);

    // The stored inspection — not the new inputs — is returned, with no new
    // events and unchanged claim/authorization/pricing digests.
    let after = fixture.lifecycle_inspection();
    assert_eq!(after, stored);
    assert_eq!(
        after.claim.authorization_sha256,
        stored.claim.authorization_sha256
    );
    assert_eq!(
        after.claim.pricing_snapshot_sha256,
        stored.claim.pricing_snapshot_sha256
    );
    assert_eq!(after.graph_run.events, stored.graph_run.events);
    assert_eq!(
        after.graph_run.run.journal_bytes,
        stored.graph_run.run.journal_bytes
    );
    assert_eq!(fixture.state_bytes(), state_before);
    fixture.assert_workspace_unchanged();
}

fn assert_stored_reentry_output(stdout: &[u8]) {
    let json: serde_json::Value = serde_json::from_slice(stdout).expect("JSON output");
    assert_eq!(json["graph_status"], "failed_uncertain");
    assert_eq!(json["classification"], "hard_crash");
    assert_eq!(json["lane_active"], false);
    assert_eq!(json["dispatch_performed_this_invocation"], false);
    assert_eq!(json["database_written_this_invocation"], false);
    assert_eq!(json["metadata_only"], true);
}

/// Re-signs the fixture authorization with a different budget so the canonical
/// payload digest differs while every self-validation still holds.
fn different_valid_authorization(canonical: &[u8]) -> String {
    let text = std::str::from_utf8(canonical).expect("UTF-8 authorization fixture");
    let mut authorization: GroupAgentNodeDispatchAuthorization =
        serde_json::from_str(text).expect("fixture authorization JSON");
    authorization.budgets.max_cost_usd_micros += 1;
    authorization.authorization_sha256 = authorization
        .expected_sha256()
        .expect("recomputed authorization digest");
    authorization.authorization_id =
        group_agent_node_dispatch_authorization_id(&authorization.authorization_sha256);
    let json = authorization.canonical_json().expect("canonical JSON");
    authorization.validate().expect("still self-valid");
    assert_ne!(json.as_bytes(), canonical);
    json
}
