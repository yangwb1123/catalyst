mod support;

use forge_runtime_domain::{
    BeginRun, HubStore, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits,
    RunOutcome, RunProvider, RunStore, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

use support::{
    RunFixture, assert_prompt_writeback, assert_success, fixture, fixture_store, invoke,
    invoke_without_openai_key, parse_jsonl, path_text, run_json, start_arguments, text_field,
};

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
    assert!(help.contains("run explain RUN_ID"));
    assert!(help.contains("run resume RUN_ID"));
}

#[test]
fn run_explain_does_not_create_a_missing_hub_database() {
    let state = TempDir::new().expect("state directory");
    let output = invoke(&[
        "--state-dir",
        path_text(state.path()),
        "--json",
        "run",
        "explain",
        "missing-run",
    ]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!state.path().join("hub.sqlite3").exists());
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
    let explanation = run_json(&["--state-dir", state, "--json", "run", "explain", run_id]);
    assert_eq!(explanation["type"], "run_explanation");
    assert_eq!(explanation["explanation"]["recovery"]["status"], "terminal");
    assert_eq!(
        explanation["explanation"]["continuation"]["command"],
        Value::Null
    );
    assert_eq!(
        explanation["explanation"]["authorization"]["workspace_read"]["status"],
        "declared_and_runtime_exposed"
    );
    assert_eq!(
        explanation["explanation"]["context"]["prior_conversation_history"]["status"],
        "open"
    );
    assert!(
        !explanation
            .to_string()
            .contains("inspect the durable workspace")
    );
    assert!(
        !explanation
            .to_string()
            .contains("Read README.md successfully")
    );
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
