#[path = "cli_agent_e2e_support/mod.rs"]
#[allow(dead_code)]
mod e2e_support;

use std::{
    process::Command,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use forge_runtime_application::{HubService, RunService};
use forge_runtime_domain::{
    BeginRun, BeginRunDisposition, BeginRunWithPrompt, CURRENT_AGENT_TOOLSET_VERSION,
    ConversationScope, Message, RUN_STORE_VERSION, RunExecution, RunLimits, RunProvider, RunStore,
    RuntimeEvent, RuntimeEventKind, ToolCall, WorkspaceIdentity,
};
use forge_runtime_infrastructure::{CapStdAgentWorkspace, SqliteHubStore};
use tempfile::TempDir;

use e2e_support::{LocalResponses, error_stream, final_stream, spawn_resume};

const PROMPT: &str = "recover the committed atomic seed";
const RUN_KEY: &str = "agent-crash-retry-key";
const MODEL: &str = "offline-test-model";
const READ_ONLY_SYSTEM_PROMPT: &str = "You are Forge, a read-only coding assistant working in the selected local workspace. Work only on the user's current task; do not invent or continue a Sprint, Roadmap, or unrelated backlog unless the user explicitly asks. Use list_files and search_text to discover relevant project instructions and code, inspect the needed files, and give concrete guidance, but do not claim to have changed files or run commands.";

#[cfg(unix)]
#[test]
fn committed_agent_seed_requires_explicit_resume_without_duplicate_provider_effect() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let server = LocalResponses::start(vec![final_stream("recovered answer")]);
    let endpoint = server.endpoint().to_owned();
    let (store, run_id, conversation_id) =
        seed_crash_prefix(state.path(), project.path(), &endpoint);

    let seeded = RunService::new(store.clone())
        .inspect_run(&run_id)
        .expect("inspect committed seed");
    assert_eq!(seeded.events.len(), 2);

    let keyed_retry = invoke_agent(&endpoint, state.path(), project.path());
    assert!(!keyed_retry.status.success());
    assert!(
        String::from_utf8_lossy(&keyed_retry.stderr).contains("explicit run resume is required")
    );
    assert_eq!(
        RunService::new(store.clone())
            .inspect_run(&run_id)
            .expect("keyed retry is read-only"),
        seeded
    );

    let resumed = invoke_resume(state.path(), project.path(), &run_id);
    assert_success(&resumed);
    let requests = server.finish();
    assert_eq!(
        requests.len(),
        1,
        "keyed replay must not add a provider request before explicit resume"
    );

    assert_terminal_retry_is_read_only(
        store,
        &endpoint,
        state.path(),
        project.path(),
        &run_id,
        &conversation_id,
    );
}

#[cfg(unix)]
fn assert_terminal_retry_is_read_only(
    store: Arc<SqliteHubStore>,
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
    run_id: &str,
    conversation_id: &str,
) {
    let after_resume = RunService::new(store.clone())
        .inspect_run(run_id)
        .expect("inspect resumed Run");
    assert_eq!(event_count(&after_resume, is_run_started), 1);
    assert_eq!(event_count(&after_resume, is_current_user), 1);
    assert_eq!(event_count(&after_resume, is_turn_started), 1);
    assert_eq!(event_count(&after_resume, is_tool_effect), 0);
    let prompts = HubService::new(store.clone())
        .list_prompts(Some(conversation_id), 10)
        .expect("resumed prompts");
    assert_eq!(prompts.len(), 2);

    assert_success(&invoke_agent(endpoint, state, project));
    assert_eq!(
        RunService::new(store.clone())
            .inspect_run(run_id)
            .expect("inspect exact retry"),
        after_resume
    );
    assert_eq!(
        HubService::new(store)
            .list_prompts(Some(conversation_id), 10)
            .expect("exact retry prompts"),
        prompts
    );
}

#[cfg(unix)]
#[test]
fn concurrent_same_key_agents_allow_only_one_provider_request() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let (server, gate) = LocalResponses::start_gated(final_stream("single winner"));
    let endpoint = server.endpoint().to_owned();

    let first = spawn_agent(&endpoint, state.path(), project.path());
    gate.wait_for_request();
    let second = spawn_agent(&endpoint, state.path(), project.path());
    let second = second.wait_with_output().expect("second Agent exits");
    gate.release();
    let first = first.wait_with_output().expect("first Agent exits");
    let requests = server.finish();

    assert_eq!(requests.len(), 1, "only the Created seed owner may send");
    assert_success(&first);
    assert!(
        !second.status.success(),
        "active seed must require explicit resume"
    );
    assert!(String::from_utf8_lossy(&second.stderr).contains("explicit run resume is required"));
    let store = Arc::new(
        SqliteHubStore::open(state.path().join("hub.sqlite3")).expect("open shared Agent Hub"),
    );
    let runs = RunService::new(store)
        .list_runs(None, 10)
        .expect("list contender Runs");
    assert_eq!(runs.len(), 1);
}

#[cfg(unix)]
#[test]
fn concurrent_resumes_allow_only_one_provider_request() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let (server, gate) = LocalResponses::start_gated(final_stream("single resume winner"));
    let (_, run_id, _) = seed_crash_prefix(state.path(), project.path(), server.endpoint());

    let first = spawn_resume(state.path(), project.path(), &run_id);
    gate.wait_for_request();
    let second = spawn_resume(state.path(), project.path(), &run_id);
    let second = second.wait_with_output().expect("second resume exits");
    gate.release();
    let first = first.wait_with_output().expect("first resume exits");

    assert_eq!(
        usize::from(first.status.success()) + usize::from(second.status.success()),
        1,
        "exactly one resume contender must execute"
    );
    let loser = if first.status.success() {
        &second
    } else {
        &first
    };
    assert!(
        String::from_utf8_lossy(&loser.stderr)
            .contains("another local Run execution is active for this Hub")
    );
    assert_eq!(server.finish().len(), 1);
}

#[cfg(unix)]
#[test]
fn explicit_resume_does_not_print_provider_control_text() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let secret = "provider-secret\n\u{1b}[2J\u{202e}spoof";
    let server = LocalResponses::start(vec![error_stream(secret)]);
    let (store, run_id, _) = seed_crash_prefix(state.path(), project.path(), server.endpoint());
    append_finished_tool_prefix(store.as_ref(), &run_id);

    let resumed = invoke_resume(state.path(), project.path(), &run_id);

    assert!(!resumed.status.success());
    let stderr = String::from_utf8_lossy(&resumed.stderr);
    assert!(
        stderr.contains("Run resume failed: provider_error"),
        "{stderr}"
    );
    assert!(!stderr.contains("provider-secret"), "{stderr}");
    assert!(!stderr.contains('\u{1b}'), "{stderr}");
    assert!(!stderr.contains('\u{202e}'), "{stderr}");
    let stdout = String::from_utf8_lossy(&resumed.stdout);
    assert!(!stdout.contains("private-tool-output"), "{stdout}");
    assert!(!stdout.contains("README.md"), "{stdout}");
    assert_eq!(server.finish().len(), 1);
}

#[cfg(unix)]
#[test]
fn keyed_replay_reports_a_pending_tool_without_suggesting_automatic_resume() {
    let project = TempDir::new().expect("project");
    let state = TempDir::new().expect("state");
    let endpoint = "http://127.0.0.1:9/v1";
    let (store, run_id, _) = seed_crash_prefix(state.path(), project.path(), endpoint);
    append_pending_tool_prefix(store.as_ref(), &run_id);
    let before = RunService::new(store.clone())
        .inspect_run(&run_id)
        .expect("inspect pending prefix");

    let retry = invoke_agent(endpoint, state.path(), project.path());

    assert!(!retry.status.success());
    let stderr = String::from_utf8_lossy(&retry.stderr);
    assert!(stderr.contains("pending_tool_effect"), "{stderr}");
    assert!(
        !stderr.contains("explicit run resume is required"),
        "{stderr}"
    );
    assert_eq!(
        RunService::new(store)
            .inspect_run(&run_id)
            .expect("keyed retry is read-only"),
        before
    );
}

fn spawn_agent(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            path_text(state),
            "--idempotency-key",
            RUN_KEY,
            "-C",
            path_text(project),
            "agent",
            "--model",
            MODEL,
            PROMPT,
        ])
        .env("OPENAI_BASE_URL", endpoint)
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn same-key Agent contender")
}

#[cfg(unix)]
fn append_pending_tool_prefix(store: &SqliteHubStore, run_id: &str) {
    let conversation_id = store
        .inspect_run(run_id)
        .expect("inspect seed")
        .run
        .conversation_id;
    let call = ToolCall {
        id: "pending-call".into(),
        name: "read_file".into(),
        arguments: serde_json::json!({"path": "README.md"}),
    };
    for (seq, kind) in [
        (3, RuntimeEventKind::TurnStarted { turn: 1 }),
        (
            4,
            RuntimeEventKind::MessageCommitted {
                message: Message::Assistant {
                    text: String::new(),
                    tool_calls: vec![call.clone()],
                },
            },
        ),
        (5, RuntimeEventKind::ToolStarted { call }),
    ] {
        store
            .append_event(&RuntimeEvent {
                v: forge_runtime_domain::PROTOCOL_VERSION,
                session_id: conversation_id.clone(),
                run_id: run_id.into(),
                seq,
                emitted_at_ms: now_ms(),
                kind,
            })
            .expect("append pending tool prefix");
    }
}

#[cfg(unix)]
fn append_finished_tool_prefix(store: &SqliteHubStore, run_id: &str) {
    append_pending_tool_prefix(store, run_id);
    let conversation_id = store
        .inspect_run(run_id)
        .expect("inspect pending prefix")
        .run
        .conversation_id;
    store
        .append_event(&RuntimeEvent {
            v: forge_runtime_domain::PROTOCOL_VERSION,
            session_id: conversation_id,
            run_id: run_id.into(),
            seq: 6,
            emitted_at_ms: now_ms(),
            kind: RuntimeEventKind::ToolFinished {
                call_id: "pending-call".into(),
                name: "read_file".into(),
                output: "private-tool-output".into(),
                is_error: false,
                truncated: false,
            },
        })
        .expect("append finished tool prefix");
}

#[cfg(unix)]
fn seed_crash_prefix(
    state: &std::path::Path,
    project_path: &std::path::Path,
    endpoint: &str,
) -> (Arc<SqliteHubStore>, String, String) {
    let canonical = project_path.canonicalize().expect("canonical project");
    let store = Arc::new(SqliteHubStore::open(state.join("hub.sqlite3")).expect("open Agent Hub"));
    let hub = HubService::new(store.clone());
    let project = hub.open_project(&canonical).expect("open Project");
    let conversation = hub
        .create_session(
            &ConversationScope::Project(project.id.clone()),
            "Forge Dev Agent",
            &format!("forge-dev-agent-session:{}", project.id),
        )
        .expect("create Agent session");
    let identity = CapStdAgentWorkspace::open(&canonical)
        .expect("open anchored workspace")
        .workspace_identity()
        .clone();
    let request = BeginRunWithPrompt {
        run: BeginRun {
            v: RUN_STORE_VERSION,
            run_id: "run-before-agent-crash".into(),
            conversation_id: conversation.id.clone(),
            prompt_id: "prompt-before-agent-crash".into(),
            project_id: project.id,
            execution: seed_execution(endpoint, identity),
            idempotency_key: RUN_KEY.into(),
            created_at_ms: now_ms(),
        },
        prompt_content: PROMPT.into(),
        prompt_idempotency_key: RUN_KEY.into(),
    };
    let seeded = RunService::new(store.clone())
        .begin_run_with_prompt(&request)
        .expect("commit atomic Prompt, Run, and executable prefix");
    assert_eq!(seeded.disposition, BeginRunDisposition::Created);
    (store, seeded.run.run_id, conversation.id)
}

fn seed_execution(endpoint: &str, identity: WorkspaceIdentity) -> RunExecution {
    RunExecution {
        provider: RunProvider::OpenAiAgent {
            endpoint: endpoint.into(),
            model: MODEL.into(),
            dev: false,
            toolset_version: CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: Some(identity),
        },
        system_prompt: READ_ONLY_SYSTEM_PROMPT.into(),
        allowed_read_paths: Vec::new(),
        limits: RunLimits {
            max_turns: 24,
            max_tool_calls: 64,
            max_tool_output_bytes: 43_690,
            max_model_output_bytes: 256 * 1024,
            max_model_events: 2_048,
            max_output_tokens_per_turn: 4_096,
        },
    }
}

fn invoke_agent(
    endpoint: &str,
    state: &std::path::Path,
    project: &std::path::Path,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            path_text(state),
            "--idempotency-key",
            RUN_KEY,
            "-C",
            path_text(project),
            "agent",
            "--model",
            MODEL,
            PROMPT,
        ])
        .env("OPENAI_BASE_URL", endpoint)
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .output()
        .expect("invoke seeded Agent")
}

fn invoke_resume(
    state: &std::path::Path,
    project: &std::path::Path,
    run_id: &str,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args([
            "--state-dir",
            path_text(state),
            "-C",
            path_text(project),
            "run",
            "resume",
            run_id,
        ])
        .env("OPENAI_API_KEY", "dummy-offline-key")
        .output()
        .expect("explicitly resume seeded Agent")
}

fn event_count(
    inspection: &forge_runtime_domain::RunInspection,
    predicate: fn(&RuntimeEventKind) -> bool,
) -> usize {
    inspection
        .events
        .iter()
        .filter(|event| predicate(&event.kind))
        .count()
}

fn is_run_started(kind: &RuntimeEventKind) -> bool {
    matches!(kind, RuntimeEventKind::RunStarted { .. })
}

fn is_current_user(kind: &RuntimeEventKind) -> bool {
    matches!(
        kind,
        RuntimeEventKind::MessageCommitted {
            message: Message::User { text }
        } if text == PROMPT
    )
}

fn is_turn_started(kind: &RuntimeEventKind) -> bool {
    matches!(kind, RuntimeEventKind::TurnStarted { .. })
}

fn is_tool_effect(kind: &RuntimeEventKind) -> bool {
    matches!(
        kind,
        RuntimeEventKind::ToolStarted { .. }
            | RuntimeEventKind::ToolFinished { .. }
            | RuntimeEventKind::ToolRejected { .. }
    )
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn path_text(path: &std::path::Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

fn now_ms() -> u64 {
    u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_millis(),
    )
    .expect("time fits u64")
}
