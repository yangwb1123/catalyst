use std::{io::ErrorKind, process::Command};

use forge_runtime_application::MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES;
use serde_json::Value;
use tempfile::TempDir;

mod group_agent_graph_run_support;
mod group_agent_graph_support;
mod group_agent_node_dispatch_authorization_support;
use group_agent_graph_run_support::Fixture;
use group_agent_node_dispatch_authorization_support::*;

const RELEASE_EXPORT_ARGS: &[&str] = &[
    "group",
    "graph",
    "run",
    "dispatch",
    "release-control",
    "export",
    "run-1",
];
const AUTHORIZATION_VERIFY_ARGS: &[&str] = &[
    "group",
    "graph",
    "run",
    "dispatch",
    "authorization",
    "verify",
    "run-1",
    "--authorization",
    "-",
];

#[test]
fn real_core_authorization_round_trip_is_exact_redacted_and_effect_free() {
    let (listener, endpoint) = loopback_sentinel();
    let fixture = Fixture::new();
    let graph_run_id = prepare_run(&fixture);
    let scheduler_control = export_scheduler_control(&fixture, &graph_run_id);
    let contract = build_contract_with_real_core(&scheduler_control, &endpoint);
    admit_contract(&fixture, &graph_run_id, &contract);
    prepare_dispatch_request(&fixture, &graph_run_id);
    fixture.assert_workspace_unchanged();
    fixture.remove_member_workspaces();
    let database_before = database_bytes(&fixture);
    let state_before = state_file_bytes(&fixture);
    assert_no_sqlite_sidecars(fixture.state.path());

    let release_control = export_release_control(&fixture, &graph_run_id, true);
    assert_eq!(state_file_bytes(&fixture), state_before);
    let human_mode = export_release_control(&fixture, &graph_run_id, false);
    assert_eq!(release_control, human_mode);
    assert!(!release_control.ends_with(b"\n"));
    assert_release_control_is_explicitly_private(&release_control, &endpoint);
    assert_eq!(database_bytes(&fixture), database_before);

    let authorization = authorize_with_real_core(&release_control);
    assert!(!authorization.ends_with(b"\n"));
    let authorization_value: Value =
        serde_json::from_slice(&authorization).expect("authorization JSON");
    let verified = verify_stdin(&fixture, &graph_run_id, &authorization);
    assert_eq!(state_file_bytes(&fixture), state_before);
    assert_verified(&verified, &authorization_value, &endpoint);
    let verified_file = verify_file(&fixture, &graph_run_id, &authorization);
    assert_eq!(verified_file, verified);
    assert_eq!(database_bytes(&fixture), database_before);

    reject_noncanonical_without_writes(&fixture, &graph_run_id, &authorization, &database_before);
    assert_eq!(state_file_bytes(&fixture), state_before);
    assert_no_sqlite_sidecars(fixture.state.path());
    assert_run_still_waits_for_authority(&fixture, &graph_run_id);
    let error = listener.accept().expect_err("no provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}

#[test]
fn release_reads_on_empty_or_missing_state_create_no_database_or_sidecars() {
    let root = TempDir::new().expect("state root");
    let empty_state = root.path().join("empty");
    std::fs::create_dir(&empty_state).expect("empty state directory");
    make_private_directory(&empty_state);
    let missing_state = root.path().join("missing");
    let cwd = TempDir::new().expect("current directory");
    let commands = [
        (RELEASE_EXPORT_ARGS, b"".as_slice()),
        (AUTHORIZATION_VERIFY_ARGS, b"{}".as_slice()),
    ];
    for state in [&empty_state, &missing_state] {
        let before = state.exists().then(|| state_directory_file_bytes(state));
        for (args, input) in &commands {
            let output = invoke_raw(state, cwd.path(), args, input);
            assert_failure(&output);
            assert!(output.stdout.is_empty());
            assert_eq!(
                state.exists().then(|| state_directory_file_bytes(state)),
                before
            );
            assert_no_sqlite_sidecars(state);
            assert!(!state.join("hub.sqlite3").exists());
        }
    }
}

#[cfg(unix)]
fn make_private_directory(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .expect("private state fixture");
}

#[cfg(not(unix))]
fn make_private_directory(_path: &std::path::Path) {}

#[test]
fn invalid_and_oversize_authorization_fail_before_database_creation() {
    let state = TempDir::new().expect("state directory");
    let cwd = TempDir::new().expect("current directory");
    let args = [
        "group",
        "graph",
        "run",
        "dispatch",
        "authorization",
        "verify",
        "run-1",
        "--authorization",
        "-",
    ];
    let cases = [
        (
            vec![b'x'; MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES + 1],
            "exceeds its byte limit",
        ),
        (vec![0xff], "must be UTF-8"),
    ];
    for (input, expected_error) in cases {
        let output = invoke_raw(state.path(), cwd.path(), &args, &input);
        assert_failure(&output);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected_error), "unexpected error: {error}");
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn release_reads_reject_idempotency_keys_before_database_creation() {
    let state = TempDir::new().expect("state directory");
    let cwd = TempDir::new().expect("current directory");
    for args in [
        vec![
            "--idempotency-key",
            "forbidden",
            "group",
            "graph",
            "run",
            "dispatch",
            "release-control",
            "export",
            "run-1",
        ],
        vec![
            "--idempotency-key",
            "forbidden",
            "group",
            "graph",
            "run",
            "dispatch",
            "authorization",
            "verify",
            "run-1",
            "--authorization",
            "-",
        ],
    ] {
        let output = invoke_raw(state.path(), cwd.path(), &args, b"{}");
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("only valid for mutating"));
        assert!(!state.path().join("hub.sqlite3").exists());
    }
}

#[test]
fn help_warns_about_raw_disclosure_and_has_no_authority_mutation_verbs() {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .arg("help")
        .output()
        .expect("run help");
    assert_success(&output);
    let help = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(help.contains("dispatch release-control export GRAPH_RUN_ID"));
    assert!(help.contains("dispatch authorization verify GRAPH_RUN_ID"));
    assert!(help.contains("explicit export command is authorization to disclose"));
    assert!(help.contains("--json does not wrap the bytes"));
    for verb in [
        "admit", "show", "list", "send", "claim", "retry", "resume", "complete", "advance",
    ] {
        assert!(!help.contains(&format!("group graph run dispatch authorization {verb}")));
    }
    for verb in [
        "admit", "show", "list", "send", "claim", "retry", "resume", "complete", "advance",
    ] {
        assert!(!help.contains(&format!("group graph run dispatch release-control {verb}")));
    }
}
