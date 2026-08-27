#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};
use tempfile::{TempDir, tempdir};

#[allow(clippy::duplicate_mod, dead_code)]
mod group_agent_graph_run_support;
#[allow(clippy::duplicate_mod, dead_code)]
mod group_agent_graph_support;
#[allow(clippy::duplicate_mod, dead_code)]
mod scheduled_graph_controller_cli_support;
#[allow(clippy::duplicate_mod, dead_code)]
mod scheduled_graph_reconcile_cli_support;

use group_agent_graph_run_support::command;
use group_agent_graph_support::path_text;
use scheduled_graph_controller_cli_support::ControllerFixture;
use scheduled_graph_reconcile_cli_support::shared_core;

#[test]
fn every_bad_controller_core_handshake_precedes_any_hub_mutation() {
    let fixture = ControllerFixture::new(shared_core());
    let cases = [
        (["9", "2", "2", "1"], &["reconcile"][..]),
        (["1", "9", "2", "1"], &["reconcile", "ready"][..]),
        (
            ["1", "2", "9", "1"],
            &["reconcile", "ready", "materialize"][..],
        ),
        (
            ["1", "2", "2", "9"],
            &["reconcile", "ready", "materialize", "terminal"][..],
        ),
    ];

    for (versions, expected_calls) in cases {
        let core = HandshakeCore::new(versions);
        let hub_before = fixture.source.hub_state();
        let output = fixture.start(&core.path, &core.sha256);

        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("handshake failed"), "{stderr}");
        assert_eq!(core.calls(), expected_calls);
        assert_eq!(
            fixture.source.hub_state(),
            hub_before,
            "bad handshake changed Hub"
        );
        fixture.assert_private(&output, &[]);
    }

    fixture.assert_workspace_unchanged();
    fixture.assert_no_network();
}

#[test]
fn malformed_identifiers_fail_before_hub_or_core_access() {
    let state = tempdir().expect("empty state directory");
    let cwd = tempdir().expect("isolated working directory");
    let core = HandshakeCore::new(["1", "2", "2", "1"]);
    let invalid_graph = "graph\u{202e}run";
    let digest = "a".repeat(64);
    let authorization = "b".repeat(64);
    let cases = [
        vec!["group", "graph", "run", "controller", "show", invalid_graph],
        vec![
            "group",
            "graph",
            "run",
            "controller",
            "advance",
            invalid_graph,
            "--core-bin",
            path_text(&core.path),
            "--core-bin-sha256",
            &core.sha256,
        ],
        vec![
            "group",
            "graph",
            "run",
            "controller",
            "step",
            "valid-graph-run",
            "--expected-awaiting-event-sha256",
            &digest,
            "--expected-provider-request-id",
            "provider\u{2066}request",
            "--expected-authorization-sha256",
            &authorization,
            "--pricing",
            "missing-pricing.json",
            "--core-bin",
            path_text(&core.path),
            "--core-bin-sha256",
            &core.sha256,
            "--confirm-off-machine",
        ],
    ];

    for args in cases {
        assert_preflight_failure(state.path(), cwd.path(), &args);
    }
    assert!(core.calls().is_empty(), "invalid input invoked Core");
    assert_eq!(fs::read_dir(state.path()).unwrap().count(), 0);
}

fn assert_preflight_failure(state: &Path, cwd: &Path, args: &[&str]) {
    let output = command(state, cwd, args)
        .output()
        .expect("run malformed controller command");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("input is invalid"), "{stderr}");
    assert!(!stderr.contains("unavailable"), "{stderr}");
}

struct HandshakeCore {
    _directory: TempDir,
    path: PathBuf,
    sha256: String,
    log: PathBuf,
}

impl HandshakeCore {
    fn new(versions: [&str; 4]) -> Self {
        let directory = tempdir().expect("handshake Core directory");
        let path = directory.path().join("handshake-core");
        let log = directory.path().join("calls");
        let script = handshake_script(&log, versions);
        fs::write(&path, script).expect("write handshake Core");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("make handshake Core executable");
        let path = path.canonicalize().expect("canonical handshake Core");
        let sha256 = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        Self {
            _directory: directory,
            path,
            sha256,
            log,
        }
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn handshake_script(log: &Path, versions: [&str; 4]) -> String {
    let log = shell_quote(log);
    format!(
        "#!/bin/sh\ncase \"$1:$2\" in\n\
         graph-scheduled-reconcile:--protocol-version) \
         printf '%s\\n' reconcile >> {log}; printf '%s' '{}';;\n\
         graph-scheduled-ready-node-dispatch-authorize:--protocol-version) \
         printf '%s\\n' ready >> {log}; printf '%s' '{}';;\n\
         graph-scheduled-node-contract:--protocol-version) \
         printf '%s\\n' materialize >> {log}; printf '%s' '{}';;\n\
         graph-scheduled-node-terminal-receipt:--protocol-version) \
         printf '%s\\n' terminal >> {log}; printf '%s' '{}';;\n\
         *) exit 98;;\nesac\n",
        versions[0], versions[1], versions[2], versions[3]
    )
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}
