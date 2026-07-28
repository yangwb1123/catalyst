use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

use forge_runtime_domain::{
    BeginRun, HubStore, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits,
    RunOutcome, RunProvider, RunStore, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

struct RunFixture {
    state: TempDir,
    project: TempDir,
    conversation_id: String,
    prompt_id: String,
}

fn invoke(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .output()
        .expect("run forge-runtime")
}

fn invoke_without_openai_key(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-runtime"))
        .args(arguments)
        .env_remove("OPENAI_API_KEY")
        .output()
        .expect("run forge-runtime without an OpenAI key")
}

fn run_json(arguments: &[&str]) -> Value {
    let output = invoke(arguments);
    assert_success(&output);
    serde_json::from_slice(&output.stdout).expect("CLI emits JSON")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "command failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn help_discloses_live_egress_journaling_and_no_tool_default() {
    let output = invoke(&["--help"]);
    assert_success(&output);
    let help = String::from_utf8_lossy(&output.stdout);

    assert!(help.contains("--allow-read RELATIVE_FILE"));
    assert!(help.contains("prompt, prior conversation history"));
    assert!(help.contains("off-machine to OpenAI"));
    assert!(help.contains("journaled locally in plaintext"));
    assert!(help.contains("exposes no tools"));
    assert!(help.contains("grants no WorkspaceRead capability"));
}

#[test]
fn allow_read_without_live_is_rejected_before_execution() {
    let output = invoke(&[
        "-C",
        ".",
        "run",
        "start",
        "conversation",
        "prompt",
        "--allow-read",
        ".env",
    ]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("only valid with --live"));
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

fn fixture() -> RunFixture {
    let state = TempDir::new().expect("state directory");
    let project = TempDir::new().expect("project directory");
    fs::write(project.path().join("README.md"), "# Durable run\n").expect("workspace fixture");
    let created = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "-C",
        path_text(project.path()),
        "session",
        "new",
        "--title",
        "Durable execution",
    ]);
    let conversation_id = text_field(&created, &["session", "id"]);
    let prompt = run_json(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "prompt",
        "add",
        &conversation_id,
        "inspect the durable workspace",
    ]);
    let prompt_id = text_field(&prompt, &["prompt", "id"]);
    RunFixture {
        state,
        project,
        conversation_id,
        prompt_id,
    }
}

fn text_field(value: &Value, path: &[&str]) -> String {
    let mut current = value;
    for segment in path {
        current = &current[*segment];
    }
    current.as_str().expect("string field").to_owned()
}

fn start_arguments<'a>(fixture: &'a RunFixture, key: &'a str) -> Vec<&'a str> {
    vec![
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        key,
        "-C",
        path_text(fixture.project.path()),
        "run",
        "start",
        &fixture.conversation_id,
        &fixture.prompt_id,
        "--read",
        "README.md",
    ]
}

fn parse_jsonl(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).expect("runtime event is JSON"))
        .collect()
}

#[test]
fn a_run_journal_survives_process_boundaries_and_terminal_replay_is_idempotent() {
    let fixture = fixture();
    let arguments = start_arguments(&fixture, "stable-run-key");
    let started = invoke(&arguments);
    assert_success(&started);
    let events = parse_jsonl(&started);
    assert!(!events.is_empty());
    assert_eq!(events[0]["type"], "run_started");
    assert_eq!(
        events.last().expect("terminal event")["type"],
        "run_finished"
    );
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["seq"], u64::try_from(index + 1).expect("sequence"));
    }
    let run_id = text_field(&events[0], &["run_id"]);
    let started_index = event_index(&events, "tool_started");
    let finished_index = event_index(&events, "tool_finished");
    assert!(started_index < finished_index);

    assert_run_queries(&fixture, &run_id, events.len());
    remove_assistant_writeback(&fixture);
    let replay = invoke(&arguments);
    assert_success(&replay);
    assert!(replay.stdout.is_empty(), "a replay emits no runtime event");
    assert_prompt_writeback(&fixture);
}

fn remove_assistant_writeback(fixture: &RunFixture) {
    let database = fixture.state.path().join("hub.sqlite3");
    let connection = Connection::open(database).expect("open Hub fixture");
    let deleted = connection
        .execute(
            "DELETE FROM prompts WHERE conversation_id = ?1 AND role = 'assistant'",
            [&fixture.conversation_id],
        )
        .expect("simulate terminal-before-writeback crash");
    assert_eq!(deleted, 1);
}

fn event_index(events: &[Value], event_type: &str) -> usize {
    events
        .iter()
        .position(|event| event["type"] == event_type)
        .unwrap_or_else(|| panic!("missing event {event_type}"))
}

fn assert_run_queries(fixture: &RunFixture, run_id: &str, event_count: usize) {
    let state = path_text(fixture.state.path());
    let inspection = run_json(&["--state-dir", state, "--json", "run", "show", run_id]);
    assert_eq!(inspection["inspection"]["recovery"]["status"], "terminal");
    assert_eq!(
        inspection["inspection"]["events"].as_array().map(Vec::len),
        Some(event_count)
    );
    assert!(!inspection.to_string().contains("stable-run-key"));
    let listed = run_json(&[
        "--state-dir",
        state,
        "--json",
        "run",
        "list",
        &fixture.conversation_id,
        "--limit",
        "5",
    ]);
    assert_eq!(listed["runs"][0]["run_id"], run_id);
}

fn assert_prompt_writeback(fixture: &RunFixture) {
    let prompts = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "prompt",
        "list",
        &fixture.conversation_id,
        "--limit",
        "10",
    ]);
    let assistant: Vec<_> = prompts["prompts"]
        .as_array()
        .expect("prompt array")
        .iter()
        .filter(|prompt| prompt["role"] == "assistant")
        .collect();
    assert_eq!(assistant.len(), 1);
    assert!(
        assistant[0]["content"]
            .as_str()
            .is_some_and(|answer| answer.contains("Read README.md successfully"))
    );
}

#[test]
fn a_mismatched_project_conversation_emits_no_event_and_creates_no_run() {
    let fixture = fixture();
    let other_project = TempDir::new().expect("other project");
    let output = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(other_project.path()),
        "run",
        "start",
        &fixture.conversation_id,
        &fixture.prompt_id,
    ]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("selected Project"));
    let listed = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "list",
    ]);
    assert_eq!(listed["runs"].as_array().map(Vec::len), Some(0));
}

#[test]
fn an_incomplete_replay_never_restarts_the_provider_or_tools() {
    let fixture = fixture();
    seed_incomplete_run(&fixture, "incomplete-key");
    let arguments = start_arguments(&fixture, "incomplete-key");

    let replay = invoke(&arguments);

    assert!(!replay.status.success());
    assert!(replay.stdout.is_empty());
    assert!(String::from_utf8_lossy(&replay.stderr).contains("incomplete"));
}

#[test]
fn live_preflight_without_a_key_creates_no_run_and_makes_no_request() {
    let fixture = fixture();
    let output = invoke_without_openai_key(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        "live-without-key",
        "-C",
        path_text(fixture.project.path()),
        "run",
        "start",
        &fixture.conversation_id,
        &fixture.prompt_id,
        "--live",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("OPENAI_API_KEY"));
    let listed = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "list",
    ]);
    assert_eq!(listed["runs"].as_array().map(Vec::len), Some(0));
}

#[test]
fn terminal_live_replay_does_not_require_an_api_key() {
    let fixture = fixture();
    seed_terminal_live_run(&fixture, "terminal-live-key");
    let output = invoke_without_openai_key(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        "terminal-live-key",
        "-C",
        path_text(fixture.project.path()),
        "run",
        "start",
        &fixture.conversation_id,
        &fixture.prompt_id,
        "--live",
    ]);

    assert_success(&output);
    assert!(output.stdout.is_empty());
    assert_prompt_writeback(&fixture);
}

#[test]
fn replay_key_cannot_change_the_persisted_execution_mode() {
    let fixture = fixture();
    seed_incomplete_run(&fixture, "bound-execution-key");
    let output = invoke_without_openai_key(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        "bound-execution-key",
        "-C",
        path_text(fixture.project.path()),
        "run",
        "start",
        &fixture.conversation_id,
        &fixture.prompt_id,
        "--live",
    ]);

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("different Run input"));
    assert!(!error.contains("OPENAI_API_KEY"));
}

fn seed_incomplete_run(fixture: &RunFixture, key: &str) {
    let database = fixture.state.path().join("hub.sqlite3");
    let store = SqliteHubStore::open(database).expect("open Hub fixture");
    let project_path = fixture
        .project
        .path()
        .canonicalize()
        .expect("canonical project");
    let project = store.open_project(&project_path).expect("existing Project");
    store
        .begin_run(&BeginRun {
            v: RUN_STORE_VERSION,
            run_id: "incomplete-run".into(),
            conversation_id: fixture.conversation_id.clone(),
            prompt_id: fixture.prompt_id.clone(),
            project_id: project.id,
            execution: RunExecution {
                provider: RunProvider::DeterministicRead {
                    path: "README.md".into(),
                },
                system_prompt: "Use only the available read-only tools to answer the user.".into(),
                allowed_read_paths: vec!["README.md".into()],
                limits: RunLimits {
                    max_turns: 4,
                    max_tool_calls: 4,
                    max_tool_output_bytes: 64 * 1024,
                    max_model_output_bytes: 64 * 1024,
                    max_model_events: 4_096,
                    max_output_tokens_per_turn: 4_096,
                },
            },
            idempotency_key: key.into(),
            created_at_ms: 1,
        })
        .expect("seed incomplete Run");
}

fn seed_terminal_live_run(fixture: &RunFixture, key: &str) {
    let store = fixture_store(fixture);
    let run = live_begin(fixture, &store, key);
    store.begin_run(&run).expect("seed live Run");
    for (seq, kind) in [
        RuntimeEventKind::RunStarted {
            prompt: "inspect the durable workspace".into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "inspect the durable workspace".into(),
            },
        },
        RuntimeEventKind::TurnStarted { turn: 1 },
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: "Read README.md successfully: cached".into(),
                tool_calls: Vec::new(),
            },
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: "Read README.md successfully: cached".into(),
            },
        },
    ]
    .into_iter()
    .enumerate()
    {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: fixture.conversation_id.clone(),
                run_id: run.run_id.clone(),
                seq: u64::try_from(seq + 1).expect("event sequence"),
                emitted_at_ms: u64::try_from(seq + 1).expect("event time"),
                kind,
            })
            .expect("seed live event");
    }
}

fn fixture_store(fixture: &RunFixture) -> SqliteHubStore {
    SqliteHubStore::open(fixture.state.path().join("hub.sqlite3")).expect("open Hub fixture")
}

fn live_begin(fixture: &RunFixture, store: &SqliteHubStore, key: &str) -> BeginRun {
    let project_path = fixture
        .project
        .path()
        .canonicalize()
        .expect("canonical project");
    let project = store.open_project(&project_path).expect("existing Project");
    BeginRun {
        v: RUN_STORE_VERSION,
        run_id: "terminal-live-run".into(),
        conversation_id: fixture.conversation_id.clone(),
        prompt_id: fixture.prompt_id.clone(),
        project_id: project.id,
        execution: RunExecution {
            provider: RunProvider::OpenAiResponses {
                endpoint: "https://api.openai.com/v1".into(),
                model: "gpt-5.6-sol".into(),
            },
            system_prompt: "Answer the user without tools. No workspace access is available."
                .into(),
            allowed_read_paths: Vec::new(),
            limits: RunLimits {
                max_turns: 4,
                max_tool_calls: 0,
                max_tool_output_bytes: 64 * 1024,
                max_model_output_bytes: 64 * 1024,
                max_model_events: 4_096,
                max_output_tokens_per_turn: 4_096,
            },
        },
        idempotency_key: key.into(),
        created_at_ms: 1,
    }
}
