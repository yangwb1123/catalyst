#![allow(dead_code)]

use std::{fs, process::Command};

use tempfile::TempDir;

mod group_agent_graph_support;
mod group_agent_node_dispatch_execution_cli_support;

use group_agent_graph_support::path_text;
use group_agent_node_dispatch_execution_cli_support::{CREDENTIAL_SECRET, Fixture, TASK_SECRET};

#[test]
fn help_and_argument_failures_expose_required_boundaries_without_state_access() {
    let help = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .arg("help")
        .output()
        .expect("run help");
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("UTF-8 help");
    for boundary in [
        "group graph run dispatch execute GRAPH_RUN_ID",
        "--authorization FILE|- --pricing FILE|-",
        "--core-bin ABSOLUTE_FILE --core-bin-sha256 SHA256",
        "--confirm-off-machine [--include-result]",
    ] {
        assert!(help.contains(boundary), "help omitted {boundary}");
    }
    assert!(help.contains("Result text is\n  hidden unless --include-result is explicit"));

    let state = TempDir::new().expect("state directory");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--state-dir", path_text(state.path()), "--json"])
        .args(["group", "graph", "run", "dispatch", "execute", "run-1"])
        .output()
        .expect("run invalid execute");
    assert_eq!(output.status.code(), Some(2));
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("dispatch execute requires --authorization FILE|-"));
    assert!(error.contains("usage:"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn wrong_core_pin_fails_before_database_credential_or_result_disclosure() {
    let state = TempDir::new().expect("state directory");
    let cwd = TempDir::new().expect("current directory");
    let authorization = cwd.path().join("authorization.json");
    let pricing = cwd.path().join("pricing.json");
    let artifact_fixture = "private-artifact-content-must-not-leak";
    fs::write(&authorization, artifact_fixture).unwrap();
    fs::write(&pricing, artifact_fixture).unwrap();
    let core = std::env::current_exe()
        .expect("test executable")
        .canonicalize()
        .expect("canonical test executable");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .current_dir(cwd.path())
        .env("OPENAI_API_KEY", CREDENTIAL_SECRET)
        .args(["--state-dir", path_text(state.path()), "--json"])
        .args([
            "group",
            "graph",
            "run",
            "dispatch",
            "execute",
            "run-1",
            "--authorization",
            path_text(&authorization),
            "--pricing",
            path_text(&pricing),
            "--core-bin",
            path_text(&core),
            "--core-bin-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--confirm-off-machine",
            "--include-result",
        ])
        .output()
        .expect("run bad Core pin");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("Core executable identity disagrees"));
    assert!(!error.contains(CREDENTIAL_SECRET));
    assert!(!error.contains(artifact_fixture));
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[test]
fn valid_cli_state_rejects_missing_consent_then_missing_credential_without_writes() {
    let fixture = Fixture::new();
    let state_before = fixture.state_bytes();

    let no_consent = fixture.execute(false, Some(CREDENTIAL_SECRET));
    assert_safe_failure(
        &no_consent,
        "fresh --confirm-off-machine consent is required",
    );
    assert_eq!(fixture.state_bytes(), state_before);
    fixture.assert_workspace_unchanged();

    let no_credential = fixture.execute(true, None);
    assert_safe_failure(
        &no_credential,
        "Group Agent Node Dispatch credential is unavailable",
    );
    assert_eq!(fixture.state_bytes(), state_before);
    fixture.assert_workspace_unchanged();
}

#[test]
fn v11_consent_authorization_and_pricing_failures_are_byte_exact() {
    let fixture = Fixture::new_v11();
    assert_eq!(fixture.schema_version(), 11);
    let state_before = fixture.state_bytes();

    let no_consent = fixture.execute(false, Some(CREDENTIAL_SECRET));
    assert_safe_failure(
        &no_consent,
        "fresh --confirm-off-machine consent is required",
    );
    assert_eq!(fixture.state_bytes(), state_before);

    let no_credential = fixture.execute(true, None);
    assert_safe_failure(
        &no_credential,
        "Group Agent Node Dispatch credential is unavailable",
    );
    assert_eq!(fixture.state_bytes(), state_before);

    let unsafe_credential = fixture.execute(true, Some(CREDENTIAL_SECRET));
    assert_safe_failure(
        &unsafe_credential,
        "registered Group Agent Node provider is unavailable",
    );
    assert_eq!(fixture.state_bytes(), state_before);

    fixture.replace_pricing(b"{}");
    let bad_pricing = fixture.execute(true, Some(CREDENTIAL_SECRET));
    assert_safe_failure(&bad_pricing, "pricing snapshot input is invalid JSON");
    assert_eq!(fixture.state_bytes(), state_before);

    fixture.replace_authorization(b"{}");
    let bad_authorization = fixture.execute(true, Some(CREDENTIAL_SECRET));
    assert_safe_failure(&bad_authorization, "authorization JSON is malformed");
    assert_eq!(fixture.schema_version(), 11);
    assert_eq!(fixture.state_bytes(), state_before);
    fixture.assert_workspace_unchanged();
}

#[test]
fn multi_node_topology_is_rejected_before_database_or_credential_mutation() {
    let fixture = Fixture::new_multi_node();
    assert_eq!(fixture.schema_version(), 11);
    let state_before = fixture.state_bytes();

    for consent in [false, true] {
        let output = fixture.execute(consent, Some(CREDENTIAL_SECRET));

        assert_safe_failure(
            &output,
            "Group Agent Node Dispatch supports only one-node Graphs in protocol v1",
        );
        assert_eq!(fixture.schema_version(), 11);
        assert_eq!(fixture.state_bytes(), state_before);
        fixture.assert_workspace_unchanged();
    }
}

#[test]
fn v4_reinvocation_reports_quarantine_without_inputs_core_consent_or_credential() {
    let fixture = Fixture::new();
    fixture.strand_v4_with_local_provider();
    let state_before = fixture.state_bytes();
    fixture.remove_execution_preflight_files();

    let output = fixture.execute_include_result(false, None);

    assert_quarantine_output(&output);
    assert_eq!(fixture.state_bytes(), state_before);
    fixture.assert_workspace_unchanged();
}

#[test]
fn hot_wal_v4_reinvocation_reads_the_claim_without_logical_writes_or_resend() {
    let fixture = Fixture::new();
    let database_before = fixture.database_bytes();
    let _keeper = fixture.hold_wal_open();
    fixture.strand_v4_with_local_provider();
    assert_eq!(fixture.database_bytes(), database_before);
    assert!(fixture.shm_exists());
    let wal_before = fixture.wal_bytes();
    fixture.remove_execution_preflight_files();

    let output = fixture.execute_include_result(false, None);

    assert_quarantine_output(&output);
    assert_eq!(fixture.database_bytes(), database_before);
    assert_eq!(fixture.wal_bytes(), wal_before);
    fixture.assert_workspace_unchanged();
}

fn assert_quarantine_output(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "quarantine inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(json["graph_status"], "dispatch_unknown");
    assert_eq!(json["lane_active"], true);
    assert_eq!(json["retry_authorized"], false);
    assert_eq!(json["dispatch_performed_this_invocation"], false);
    assert_eq!(json["database_written_this_invocation"], false);
    assert_eq!(json["metadata_only"], true);
    assert!(json.get("result_text").is_none());
}

fn assert_safe_failure(output: &std::process::Output, expected: &str) {
    assert!(!output.status.success(), "execute unexpectedly succeeded");
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains(expected), "unexpected failure: {error}");
    assert!(!error.contains(CREDENTIAL_SECRET));
    assert!(!error.contains(TASK_SECRET));
    assert!(!error.contains("private-dispatch-execution-model"));
}
