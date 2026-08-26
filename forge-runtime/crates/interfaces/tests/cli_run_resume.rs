mod support;

use forge_runtime_domain::{
    BeginRun, HubStore, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits,
    RunProvider, RunStore, RuntimeEvent, RuntimeEventKind, ToolCall,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use support::{
    RunFixture, assert_prompt_writeback, assert_success, fixture, fixture_store, invoke,
    invoke_without_openai_key, parse_jsonl, path_text, run_json, start_arguments,
};

#[test]
fn an_incomplete_replay_never_restarts_the_provider_or_tools() {
    let fixture = fixture();
    seed_incomplete_run(&fixture, "incomplete-key");
    let arguments = start_arguments(&fixture, "incomplete-key");
    let replay = invoke(&arguments);

    assert!(!replay.status.success());
    assert!(replay.stdout.is_empty());
    assert!(String::from_utf8_lossy(&replay.stderr).contains("incomplete"));
    assert_empty_prefix_explanation(&fixture);
}

#[test]
fn explicit_resume_repairs_a_committed_tool_effect_without_reexecuting_it() {
    let fixture = fixture();
    seed_tool_prefix(&fixture, "resume-after-tool", true);
    assert_finished_tool_explanation(&fixture);

    let resumed = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(fixture.project.path()),
        "run",
        "resume",
        "incomplete-run",
    ]);

    assert_success(&resumed);
    let new_events = parse_jsonl(&resumed);
    assert_eq!(new_events[0]["seq"], 7);
    assert_eq!(new_events[0]["type"], "message_committed");
    assert_eq!(new_events[0]["message"]["role"], "tool");
    assert_eq!(
        new_events
            .iter()
            .filter(|event| event["type"] == "tool_started")
            .count(),
        0,
        "a committed tool effect must not be replayed"
    );
    assert_eq!(
        new_events.last().expect("terminal event")["type"],
        "run_finished"
    );
    assert_prompt_writeback(&fixture);
    assert_terminal_event_count(&fixture, 6 + new_events.len());
}

#[test]
fn explicit_resume_refuses_a_pending_tool_before_provider_or_tool_setup() {
    let fixture = fixture();
    seed_tool_prefix(&fixture, "resume-pending-tool", false);
    assert_pending_tool_explanation(&fixture);

    let resumed = invoke_without_openai_key(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(fixture.project.path()),
        "run",
        "resume",
        "incomplete-run",
    ]);

    assert!(!resumed.status.success());
    assert!(resumed.stdout.is_empty());
    let error = String::from_utf8_lossy(&resumed.stderr);
    assert!(error.contains("pending tool effect"));
    assert!(!error.contains("OPENAI_API_KEY"));
    assert_pending_event_count(&fixture, 5);
}

#[test]
fn explicit_resume_requires_the_bound_project() {
    let fixture = fixture();
    seed_tool_prefix(&fixture, "resume-project-binding", true);
    let other_project = TempDir::new().expect("other project");
    let resumed = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(other_project.path()),
        "run",
        "resume",
        "incomplete-run",
    ]);

    assert!(!resumed.status.success());
    assert!(resumed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&resumed.stderr).contains("does not match"));
}

#[test]
fn explicit_resume_preserves_a_cancelled_rejection_batch() {
    let fixture = fixture();
    seed_cancelled_rejection_prefix(&fixture, "resume-rejected-batch");

    let resumed = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(fixture.project.path()),
        "run",
        "resume",
        "incomplete-run",
    ]);

    assert_eq!(resumed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&resumed.stderr).contains("Cancelled"));
    let events = parse_jsonl(&resumed);
    assert_eq!(events.len(), 3);
    assert_eq!(events[0]["type"], "tool_rejected");
    assert_eq!(events[0]["call"]["id"], "fixture-call-2");
    assert_eq!(events[1]["type"], "message_committed");
    assert_eq!(events[2]["type"], "run_finished");
    assert_eq!(events[2]["outcome"]["status"], "cancelled");
    assert!(events.iter().all(|event| event["type"] != "tool_started"));
}

#[test]
fn explicit_resume_bounds_a_repaired_limit_message_then_finishes() {
    let fixture = fixture();
    seed_limit_rejection_prefix(&fixture, "resume-limit-rejection", 8);

    let resumed = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(fixture.project.path()),
        "run",
        "resume",
        "incomplete-run",
    ]);

    assert_eq!(resumed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&resumed.stderr).contains("LimitExceeded"));
    let events = parse_jsonl(&resumed);
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["type"], "message_committed");
    assert_eq!(events[0]["message"]["output"], "tool_cal");
    assert_eq!(events[0]["message"]["truncated"], true);
    assert_eq!(events[1]["type"], "run_finished");
    assert_eq!(events[1]["outcome"]["status"], "limit_exceeded");
    assert_eq!(events[1]["outcome"]["kind"], "tool_calls");
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

fn explain_json(fixture: &RunFixture) -> Value {
    run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "explain",
        "incomplete-run",
    ])
}

fn assert_empty_prefix_explanation(fixture: &RunFixture) {
    let explanation = explain_json(fixture);
    assert_eq!(
        explanation["explanation"]["recovery"]["status"],
        "incomplete"
    );
    assert_eq!(explanation["explanation"]["continuation"]["safe"], false);
    assert_eq!(
        explanation["explanation"]["continuation"]["command"],
        Value::Null
    );
}

fn assert_finished_tool_explanation(fixture: &RunFixture) {
    let explanation = explain_json(fixture);
    assert_eq!(explanation["explanation"]["continuation"]["safe"], true);
    assert_eq!(
        explanation["explanation"]["context"]["observed_tool_calls"][0]["outcome"],
        "finished"
    );
}

fn assert_pending_tool_explanation(fixture: &RunFixture) {
    let explanation = explain_json(fixture);
    assert_eq!(
        explanation["explanation"]["recovery"]["status"],
        "pending_tool"
    );
    assert_eq!(explanation["explanation"]["continuation"]["safe"], false);
    assert_eq!(
        explanation["explanation"]["context"]["observed_tool_calls"][0]["outcome"],
        "pending_or_unresolved"
    );
}

fn assert_terminal_event_count(fixture: &RunFixture, expected: usize) {
    let inspection = run_inspection(fixture);
    assert_eq!(inspection["inspection"]["recovery"]["status"], "terminal");
    assert_eq!(
        inspection["inspection"]["events"].as_array().map(Vec::len),
        Some(expected)
    );
}

fn assert_pending_event_count(fixture: &RunFixture, expected: usize) {
    let inspection = run_inspection(fixture);
    assert_eq!(
        inspection["inspection"]["recovery"]["status"],
        "pending_tool"
    );
    assert_eq!(
        inspection["inspection"]["events"].as_array().map(Vec::len),
        Some(expected)
    );
}

fn run_inspection(fixture: &RunFixture) -> Value {
    run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "show",
        "incomplete-run",
    ])
}

fn seed_incomplete_run(fixture: &RunFixture, key: &str) {
    seed_incomplete_run_with_output_limit(fixture, key, 64 * 1024);
}

fn seed_incomplete_run_with_output_limit(
    fixture: &RunFixture,
    key: &str,
    max_tool_output_bytes: usize,
) {
    let store = fixture_store(fixture);
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
                    max_tool_output_bytes,
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

fn seed_cancelled_rejection_prefix(fixture: &RunFixture, key: &str) {
    seed_incomplete_run(fixture, key);
    let first = tool_call();
    let second = ToolCall {
        id: "fixture-call-2".into(),
        ..tool_call()
    };
    append_prefix(
        fixture,
        vec![
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
                    text: String::new(),
                    tool_calls: vec![first.clone(), second],
                },
            },
            rejected(&first, "cancelled"),
            rejected_message(first, "cancelled: tool call was not executed"),
        ],
    );
}

fn seed_limit_rejection_prefix(fixture: &RunFixture, key: &str, output_limit: usize) {
    seed_incomplete_run_with_output_limit(fixture, key, output_limit);
    let call = tool_call();
    append_prefix(
        fixture,
        vec![
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
                    text: String::new(),
                    tool_calls: vec![call.clone()],
                },
            },
            rejected(&call, "tool_call_limit"),
        ],
    );
}

fn rejected(call: &ToolCall, code: &str) -> RuntimeEventKind {
    RuntimeEventKind::ToolRejected {
        call: call.clone(),
        code: code.into(),
        message: "tool call was not executed".into(),
    }
}

fn rejected_message(call: ToolCall, output: &str) -> RuntimeEventKind {
    RuntimeEventKind::MessageCommitted {
        message: Message::Tool {
            call_id: call.id,
            name: call.name,
            output: output.into(),
            is_error: true,
            truncated: false,
        },
    }
}

fn append_prefix(fixture: &RunFixture, kinds: Vec<RuntimeEventKind>) {
    let store = fixture_store(fixture);
    for (seq, kind) in kinds.into_iter().enumerate() {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: fixture.conversation_id.clone(),
                run_id: "incomplete-run".into(),
                seq: u64::try_from(seq + 1).expect("event sequence"),
                emitted_at_ms: u64::try_from(seq + 1).expect("event time"),
                kind,
            })
            .expect("seed resumable event");
    }
}

fn seed_tool_prefix(fixture: &RunFixture, key: &str, include_finished: bool) {
    seed_incomplete_run(fixture, key);
    let call = tool_call();
    let mut kinds = vec![
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
                text: "Inspecting README.md.".into(),
                tool_calls: vec![call.clone()],
            },
        },
        RuntimeEventKind::ToolStarted { call: call.clone() },
    ];
    if include_finished {
        kinds.push(RuntimeEventKind::ToolFinished {
            call_id: call.id.clone(),
            name: call.name.clone(),
            output: "# Durable run\n".into(),
            is_error: false,
            truncated: false,
        });
    }
    let store = fixture_store(fixture);
    for (seq, kind) in kinds.into_iter().enumerate() {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: fixture.conversation_id.clone(),
                run_id: "incomplete-run".into(),
                seq: u64::try_from(seq + 1).expect("event sequence"),
                emitted_at_ms: u64::try_from(seq + 1).expect("event time"),
                kind,
            })
            .expect("seed resumable event");
    }
}

fn tool_call() -> ToolCall {
    ToolCall {
        id: "fixture-call-1".into(),
        name: "read_file".into(),
        arguments: json!({ "path": "README.md" }),
    }
}
