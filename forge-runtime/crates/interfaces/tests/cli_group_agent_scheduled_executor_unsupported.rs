#![cfg(not(target_os = "linux"))]

use std::{path::Path, process::Command};

use tempfile::TempDir;

#[test]
fn effectful_scheduled_execute_rejects_before_private_source_or_state_access() {
    let state = TempDir::new().expect("state directory");
    let missing_authorization = state.path().join("private-authorization-must-not-be-read");
    let missing_pricing = state.path().join("private-pricing-must-not-be-read");
    let missing_core = state.path().join("core-must-not-be-opened");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(["--state-dir", path_text(state.path()), "--json"])
        .args([
            "group",
            "graph",
            "run",
            "scheduled-contract",
            "provider-request",
            "execute",
            "request-1",
            "--authorization",
            path_text(&missing_authorization),
            "--pricing",
            path_text(&missing_pricing),
            "--core-bin",
            path_text(&missing_core),
            "--core-bin-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--confirm-off-machine",
        ])
        .output()
        .expect("run unsupported scheduled execute");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("execution and adjudication are Linux-only"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("UTF-8 temporary path")
}
