#[path = "cli_agent_e2e_support/mod.rs"]
#[allow(dead_code)]
mod e2e_support;
#[allow(dead_code)]
mod support;

use std::{fs, process::Command, sync::Arc};

use forge_runtime_application::{HubService, RunService};
use forge_runtime_domain::{
    CURRENT_AGENT_TOOLSET_VERSION, ConversationScope, RunOutcome, RunProvider, RunRecoveryState,
};
use forge_runtime_infrastructure::SqliteHubStore;
use serde_json::{Value, json};
use tempfile::TempDir;

use e2e_support::{
    LocalResponses, assert_agent_trust, assert_dev_warning, assert_prior_conversation_history,
    error_stream, final_stream, invoke_agent, invoke_agent_json, invoke_agent_stdin,
    invoke_keyed_agent, tool_stream,
};
use support::{assert_success, invoke, invoke_without_openai_key, path_text};

#[test]
fn help_presents_the_agent_as_a_finite_opt_in_dev_command() {
    let output = invoke(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("-C PATH agent"));
    assert!(help.contains("read-only by default"));
    assert!(help.contains("--dev"));
    assert!(help.contains("not an OS sandbox"));
}

#[test]
fn missing_provider_key_fails_before_creating_agent_state() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let output = invoke_without_openai_key(&[
        "--state-dir",
        path_text(state.path()),
        "-C",
        path_text(project.path()),
        "agent",
        "inspect the project",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("OPENAI_API_KEY"));
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[cfg(unix)]
#[test]
fn agent_reads_a_sensitive_prompt_from_bounded_stdin() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let prompt = "sensitive stdin request\nwith a second line";
    let server = LocalResponses::start(vec![final_stream("stdin received")]);

    let output = invoke_agent_stdin(
        server.endpoint(),
        state.path(),
        project.path(),
        prompt.as_bytes(),
    );
    let requests = server.finish();

    assert_success(&output);
    assert_eq!(requests.len(), 1);
    let user = requests[0]["input"]
        .as_array()
        .expect("provider input")
        .iter()
        .find(|item| item["type"] == "message" && item["role"] == "user")
        .expect("current user Prompt");
    assert_eq!(user["content"], prompt);
}

#[cfg(unix)]
#[test]
fn human_outer_error_does_not_repeat_provider_controlled_detail() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let secret = "provider-secret\u{1b}[2J\u{202e}hidden";
    let server = LocalResponses::start(vec![error_stream(secret)]);
    let options = ["--model", "offline-test-model"];

    let output = invoke_keyed_agent(
        server.endpoint(),
        state.path(),
        project.path(),
        "malicious-provider-error-key",
        &options,
        "trigger provider failure",
    );
    let replay = invoke_keyed_agent(
        server.endpoint(),
        state.path(),
        project.path(),
        "malicious-provider-error-key",
        &options,
        "trigger provider failure",
    );
    server.finish();

    for output in [output, replay] {
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        assert!(stderr.contains("provider_error"), "{stderr}");
        assert!(!stderr.contains("provider-secret"), "{stderr}");
        assert!(!stderr.contains('\u{1b}'), "{stderr:?}");
        assert!(!stderr.contains('\u{202e}'), "{stderr:?}");
    }
}

#[test]
fn agent_requires_an_explicit_project() {
    let output = invoke(&["agent", "inspect the project"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("agent requires -C"));
}

#[test]
fn invalid_idempotency_key_fails_before_agent_state_creation() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let output = Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            path_text(state.path()),
            "--idempotency-key",
            "   ",
            "-C",
            path_text(project.path()),
            "agent",
            "inspect the project",
        ])
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .output()
        .expect("invoke Agent with an invalid idempotency key");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .to_lowercase()
            .contains("idempotency")
    );
    assert!(!state.path().join("hub.sqlite3").exists());
}

#[cfg(unix)]
#[test]
fn dev_agent_edits_and_verifies_a_workspace_through_the_real_cli() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    fs::write(project.path().join("note.txt"), "broken\n").expect("workspace fixture");
    let server = LocalResponses::start(dev_responses());

    let output = invoke_agent(server.endpoint(), state.path(), project.path(), true);
    let requests = server.finish();

    assert_dev_warning(&output);
    assert_agent_succeeded(&output, project.path(), state.path());
    assert_dev_requests(&requests);
}

#[cfg(unix)]
#[test]
fn mutation_and_process_tools_are_exposed_only_by_dev_mode() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let server = LocalResponses::start(vec![final_stream("inspection complete")]);

    let output = invoke_agent(server.endpoint(), state.path(), project.path(), false);
    let requests = server.finish();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        tool_names(&requests[0]),
        vec!["list_files", "read_file", "search_text"]
    );
}

#[cfg(unix)]
#[test]
fn agent_reuses_one_project_session_and_json_is_machine_readable() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");

    let first_server = LocalResponses::start(vec![final_stream("first answer")]);
    let first = invoke_agent(first_server.endpoint(), state.path(), project.path(), false);
    first_server.finish();
    assert!(first.status.success());

    let second_server = LocalResponses::start(vec![final_stream("second answer")]);
    let second = invoke_agent_json(second_server.endpoint(), state.path(), project.path());
    let requests = second_server.finish();
    assert!(second.status.success());
    let events: Vec<Value> = String::from_utf8(second.stdout)
        .expect("JSON output is UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("each output line is JSON"))
        .collect();
    assert_eq!(events[0]["type"], "agent_started");
    assert_agent_trust(&events[0], false);
    assert_prior_conversation_history(&requests[0]);

    let (hub, project_scope) = persisted_hub(state.path(), project.path());
    let sessions = hub.list_sessions(&project_scope).expect("Project sessions");
    assert_eq!(sessions.len(), 1);
    let prompts = hub
        .list_prompts(Some(&sessions[0].id), 10)
        .expect("persisted prompts");
    assert_eq!(prompts.len(), 4);
}

fn dev_responses() -> Vec<String> {
    vec![
        tool_stream(1, "read_file", &json!({"path": "note.txt"})),
        tool_stream(
            2,
            "edit_file",
            &json!({"path": "note.txt", "old_text": "broken\n", "new_text": "fixed\n"}),
        ),
        tool_stream(
            3,
            "exec_command",
            &json!({
                "program": "/bin/sh",
                "argv": ["-c", "test \"$(cat note.txt)\" = fixed"],
                "timeout_ms": 5000
            }),
        ),
        final_stream("fixed and verified"),
    ]
}

fn assert_agent_succeeded(
    output: &std::process::Output,
    project: &std::path::Path,
    state: &std::path::Path,
) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(project.join("note.txt")).unwrap(),
        "fixed\n"
    );
    assert!(state.join("hub.sqlite3").is_file());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "[tool] read_file started",
        "[tool] read_file finished",
        "[tool] edit_file started",
        "[tool] edit_file finished",
        "[tool] exec_command started",
        "[tool] exec_command finished",
        "fixed and verified",
        "[run] completed",
    ] {
        assert!(
            stdout.contains(expected),
            "missing {expected:?} in:\n{stdout}"
        );
    }
    assert_persisted_dev_run(state, project);
}

fn assert_persisted_dev_run(state: &std::path::Path, project: &std::path::Path) {
    let store = Arc::new(
        SqliteHubStore::open(state.join("hub.sqlite3")).expect("open persisted Agent Hub"),
    );
    let runs = RunService::new(store)
        .list_runs(None, 10)
        .expect("persisted Agent Runs");
    assert_eq!(runs.len(), 1);
    assert!(matches!(
        &runs[0].execution.provider,
        RunProvider::OpenAiAgent {
            dev: true,
            toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: Some(_),
            ..
        }
    ));

    let (hub, scope) = persisted_hub(state, project);
    let sessions = hub.list_sessions(&scope).expect("Project sessions");
    assert_eq!(sessions.len(), 1);
    let prompts = hub
        .list_prompts(Some(&sessions[0].id), 10)
        .expect("Agent prompts");
    assert_eq!(prompts.len(), 2);
    assert_eq!(prompts[0].role, "assistant");
    assert_eq!(prompts[1].role, "user");

    let inspection = RunService::new(Arc::new(
        SqliteHubStore::open(state.join("hub.sqlite3")).expect("reopen Agent Hub"),
    ))
    .inspect_run(&runs[0].run_id)
    .expect("inspect Agent Run");
    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { .. }
        }
    ));
}

fn persisted_hub(
    state: &std::path::Path,
    project: &std::path::Path,
) -> (HubService, ConversationScope) {
    let store = Arc::new(SqliteHubStore::open(state.join("hub.sqlite3")).expect("open Agent Hub"));
    let hub = HubService::new(store);
    let canonical = project.canonicalize().expect("canonical Project");
    let project = hub.open_project(&canonical).expect("persisted Project");
    let scope = ConversationScope::Project(project.id);
    (hub, scope)
}

fn assert_dev_requests(requests: &[Value]) {
    assert_eq!(requests.len(), 4);
    for request in requests {
        assert_eq!(
            tool_names(request),
            vec![
                "edit_file",
                "exec_command",
                "list_files",
                "read_file",
                "search_text",
            ]
        );
    }
    assert_output(requests, 1, "call-1", "broken");
    assert_output(requests, 2, "call-2", "after_sha256");
    assert_output(requests, 3, "call-3", "exit_code: 0");
}

fn tool_names(request: &Value) -> Vec<&str> {
    request["tools"]
        .as_array()
        .expect("tool array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect()
}

fn assert_output(requests: &[Value], request_index: usize, call_id: &str, expected: &str) {
    let output = requests[request_index]["input"]
        .as_array()
        .expect("input array")
        .iter()
        .find(|item| item["type"] == "function_call_output" && item["call_id"] == call_id)
        .expect("function call output");
    assert!(output["output"].as_str().unwrap().contains(expected));
}
