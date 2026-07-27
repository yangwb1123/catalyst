use std::{
    fs,
    io::Write,
    process::{Command, Stdio},
};

use serde_json::Value;
use tempfile::TempDir;

fn run_json(arguments: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .output()
        .expect("run forge-runtime");
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI emits JSON")
}

fn run_failure(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .output()
        .expect("run forge-runtime");
    assert!(!output.status.success(), "command must fail");
    String::from_utf8(output.stderr).expect("diagnostics are UTF-8")
}

fn run_json_stdin(arguments: &[&str], input: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn forge-runtime");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input.as_bytes())
        .expect("write prompt");
    let output = child.wait_with_output().expect("wait for forge-runtime");
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("CLI emits JSON")
}

fn path_text(path: &std::path::Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

fn create_group(state: &TempDir) -> String {
    let group = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "group",
        "create",
        "identity rollout",
    ]);
    group["group"]["id"].as_str().expect("group id").to_owned()
}

fn link_projects(state: &TempDir, projects: &TempDir, group_id: &str, roles: &[&str]) {
    for role in roles {
        let path = projects.path().join(role);
        fs::create_dir(&path).expect("project fixture");
        run_json(&[
            "--state-dir",
            path_text(state.path()),
            "--json",
            "group",
            "add",
            group_id,
            path_text(&path),
            "--role",
            role,
        ]);
    }
}

fn create_group_discussion(state: &TempDir, group_id: &str) -> String {
    let session = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "--group",
        group_id,
        "session",
        "new",
        "--title",
        "SSO integration discussion",
    ]);
    session["session"]["id"]
        .as_str()
        .expect("group session")
        .to_owned()
}

#[test]
fn no_path_is_global_and_a_path_enters_a_project_hub() {
    let state = TempDir::new().expect("state directory");
    let project = TempDir::new().expect("project directory");
    let global = run_json(&["--state-dir", path_text(state.path()), "--json"]);
    assert_eq!(global["type"], "hub");
    assert_eq!(global["snapshot"]["scope"]["kind"], "global");
    assert_eq!(global["remote"], "not_configured");

    let scoped = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        path_text(project.path()),
    ]);
    assert_eq!(scoped["snapshot"]["scope"]["kind"], "project");
    assert_eq!(
        scoped["snapshot"]["projects"][0]["path"],
        path_text(&project.path().canonicalize().expect("canonical project"))
    );
}

#[test]
fn session_prompt_and_global_memory_persist_across_processes() {
    let state = TempDir::new().expect("state directory");
    let project = TempDir::new().expect("project directory");
    let created = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        path_text(project.path()),
        "session",
        "new",
        "--title",
        "Frontend",
    ]);
    let session_id = created["session"]["id"].as_str().expect("session id");

    let receipt = run_json_stdin(
        &[
            "--state-dir",
            path_text(state.path()),
            "--json",
            "prompt",
            "add",
            session_id,
            "-",
        ],
        "connect frontend to SSO",
    );
    assert!(receipt["prompt"].get("content").is_none());
    assert!(receipt["prompt"].get("idempotency_key").is_none());
    let prompts = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "prompt",
        "list",
    ]);
    assert_eq!(prompts["prompts"][0]["conversation_id"], session_id);
    assert_eq!(prompts["prompts"][0]["content"], "connect frontend to SSO");
}

#[test]
fn a_local_group_links_frontend_backend_and_sso_roles() {
    let state = TempDir::new().expect("state directory");
    let projects = TempDir::new().expect("projects directory");
    let roles = ["frontend", "backend", "sso"];
    let group_id = create_group(&state);
    link_projects(&state, &projects, &group_id, &roles);
    let session_id = create_group_discussion(&state, &group_id);
    run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "prompt",
        "add",
        &session_id,
        "compare the three integration contracts",
    ]);

    let hub = run_json(&["--state-dir", path_text(state.path()), "--json"]);
    let links = hub["snapshot"]["group_project_members"]
        .as_array()
        .expect("group links");
    assert_eq!(links.len(), 3);
    for role in roles {
        assert!(links.iter().any(|link| link["role"] == role));
    }
    let group_hub = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "--group",
        &group_id,
    ]);
    assert_eq!(group_hub["snapshot"]["conversations"][0]["id"], session_id);
}

#[test]
fn management_commands_never_silently_ignore_a_space_selector() {
    let error = run_failure(&["--group", "missing", "prompt", "list"]);
    assert!(error.contains("selectors are not valid"));
}

#[test]
fn a_reused_cli_key_makes_cross_process_retries_idempotent() {
    let state = TempDir::new().expect("state directory");
    let project = TempDir::new().expect("project directory");
    let args = [
        "--state-dir",
        path_text(state.path()),
        "--json",
        "--idempotency-key",
        "stable-session-key",
        path_text(project.path()),
        "session",
        "new",
        "--title",
        "Retry-safe session",
    ];
    let first = run_json(&args);
    let replay = run_json(&args);
    assert_eq!(first["session"]["id"], replay["session"]["id"]);

    let error = run_failure(&[
        "--state-dir",
        path_text(state.path()),
        "--idempotency-key",
        "stable-session-key",
        path_text(project.path()),
        "session",
        "new",
        "--title",
        "Changed payload",
    ]);
    assert!(error.contains("idempotency key"));
}

#[test]
fn a_failed_group_link_does_not_register_the_project() {
    let state = TempDir::new().expect("state directory");
    let project = TempDir::new().expect("project directory");
    let error = run_failure(&[
        "--state-dir",
        path_text(state.path()),
        "group",
        "add",
        "missing-group",
        path_text(project.path()),
    ]);
    assert!(error.contains("was not found"));

    let hub = run_json(&["--state-dir", path_text(state.path()), "--json"]);
    assert_eq!(
        hub["snapshot"]["projects"].as_array().map(Vec::len),
        Some(0)
    );
}
