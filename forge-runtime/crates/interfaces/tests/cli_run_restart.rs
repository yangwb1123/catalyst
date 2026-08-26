mod support;

use forge_runtime_domain::{
    BeginRun, HubStore, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits,
    RunOutcome, RunProvider, RunStore, RuntimeEvent, RuntimeEventKind, ToolCall,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use support::{
    RunFixture, assert_success, fixture, fixture_store, invoke, invoke_without_openai_key,
    parse_jsonl, path_text, run_json, start_arguments, text_field,
};

#[test]
fn terminal_run_restart_is_content_free_idempotent_and_resumable() {
    let fixture = fixture();
    let source_output = invoke(&start_arguments(&fixture, "source-run-key"));
    assert_success(&source_output);
    let source_run_id = text_field(&parse_jsonl(&source_output)[0], &["run_id"]);

    let created = restart_json(&fixture, &source_run_id, "restart-key");
    assert_eq!(created["type"], "run_restart_prepared");
    assert_eq!(created["disposition"], "created");
    assert_eq!(created["state"], "ready_to_resume");
    assert_eq!(created["resume_required"], true);
    assert_eq!(created["external_effects"], false);
    assert_eq!(created["journal_events"], 1);
    assert!(
        !created
            .to_string()
            .contains("inspect the durable workspace")
    );
    assert!(!created.to_string().contains("Read README.md successfully"));
    let restarted_run_id = text_field(&created, &["run_id"]);

    let replayed = restart_json(&fixture, &source_run_id, "restart-key");
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(replayed["run_id"], restarted_run_id);
    assert_restart_prefix(&fixture, &restarted_run_id);

    let resumed = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(fixture.project.path()),
        "run",
        "resume",
        &restarted_run_id,
    ]);
    assert_success(&resumed);
    let events = parse_jsonl(&resumed);
    assert_eq!(events[0]["seq"], 2);
    assert_eq!(events[0]["type"], "message_committed");
    assert_eq!(
        events.last().expect("terminal event")["type"],
        "run_finished"
    );

    assert_terminal_replay(&fixture, &source_run_id, &restarted_run_id);
    assert_changed_source_conflicts(&fixture, &restarted_run_id);
}

fn assert_terminal_replay(fixture: &RunFixture, source_run_id: &str, restarted_run_id: &str) {
    let terminal_replay = restart_json(fixture, source_run_id, "restart-key");
    assert_eq!(terminal_replay["disposition"], "replayed");
    assert_eq!(terminal_replay["run_id"], restarted_run_id);
    assert_eq!(terminal_replay["state"], "terminal");
    assert_eq!(terminal_replay["resume_required"], false);
    assert!(terminal_replay["journal_events"].as_u64().unwrap_or(0) > 1);
}

fn assert_changed_source_conflicts(fixture: &RunFixture, restarted_run_id: &str) {
    let changed_source = invoke(&restart_arguments(fixture, restarted_run_id, "restart-key"));
    assert!(!changed_source.status.success());
    assert!(changed_source.stdout.is_empty());
    assert!(String::from_utf8_lossy(&changed_source.stderr).contains("idempotency"));
}

#[test]
fn restart_refuses_nonterminal_source_and_a_mismatched_project() {
    let fixture = fixture();
    seed_source(
        &fixture,
        "incomplete-source",
        "incomplete-source-key",
        false,
    );
    let incomplete = invoke(&restart_arguments(
        &fixture,
        "incomplete-source",
        "restart-incomplete",
    ));
    assert!(!incomplete.status.success());
    assert!(incomplete.stdout.is_empty());
    assert!(String::from_utf8_lossy(&incomplete.stderr).contains("terminal"));

    seed_source(&fixture, "terminal-source", "terminal-source-key", true);
    let other_project = TempDir::new().expect("other Project");
    let mismatched = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        "restart-mismatched",
        "-C",
        path_text(other_project.path()),
        "run",
        "restart",
        "terminal-source",
    ]);
    assert!(!mismatched.status.success());
    assert!(mismatched.stdout.is_empty());
    assert!(String::from_utf8_lossy(&mismatched.stderr).contains("does not match"));

    let runs = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "list",
    ]);
    assert_eq!(runs["runs"].as_array().map(Vec::len), Some(2));
}

#[test]
fn live_restart_preparation_reads_no_credential_and_sends_nothing() {
    let fixture = fixture();
    seed_source(&fixture, "live-source", "live-source-key", true);
    let output = invoke_without_openai_key(&restart_arguments(
        &fixture,
        "live-source",
        "live-restart-key",
    ));
    assert_success(&output);
    let prepared: Value = serde_json::from_slice(&output.stdout).expect("restart JSON");
    let run_id = text_field(&prepared, &["run_id"]);
    assert_restart_prefix(&fixture, &run_id);

    let resume = invoke_without_openai_key(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(fixture.project.path()),
        "run",
        "resume",
        &run_id,
    ]);
    assert!(!resume.status.success());
    assert!(resume.stdout.is_empty());
    assert!(String::from_utf8_lossy(&resume.stderr).contains("OPENAI_API_KEY"));
    assert_restart_prefix(&fixture, &run_id);
}

#[test]
fn late_restart_replay_reports_nonterminal_target_states() {
    let fixture = fixture();
    let source_output = invoke(&start_arguments(&fixture, "late-source-key"));
    assert_success(&source_output);
    let source_run_id = text_field(&parse_jsonl(&source_output)[0], &["run_id"]);
    let prepared = restart_json(&fixture, &source_run_id, "late-restart-key");
    let target_run_id = text_field(&prepared, &["run_id"]);
    append_target_events(
        &fixture,
        &target_run_id,
        2,
        vec![RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "inspect the durable workspace".into(),
            },
        }],
    );

    let incomplete = restart_json(&fixture, &source_run_id, "late-restart-key");
    assert_eq!(incomplete["state"], "incomplete");
    assert_eq!(incomplete["resume_required"], true);

    let call = ToolCall {
        id: "late-call".into(),
        name: "read_file".into(),
        arguments: json!({ "path": "README.md" }),
    };
    append_target_events(
        &fixture,
        &target_run_id,
        3,
        vec![
            RuntimeEventKind::TurnStarted { turn: 1 },
            RuntimeEventKind::MessageCommitted {
                message: Message::Assistant {
                    text: "Inspecting.".into(),
                    tool_calls: vec![call.clone()],
                },
            },
            RuntimeEventKind::ToolStarted { call },
        ],
    );

    let pending = restart_json(&fixture, &source_run_id, "late-restart-key");
    assert_eq!(pending["state"], "pending_tool_effect");
    assert_eq!(pending["resume_required"], false);
}

fn restart_json(fixture: &RunFixture, source_run_id: &str, key: &str) -> Value {
    let output = invoke(&restart_arguments(fixture, source_run_id, key));
    assert_success(&output);
    serde_json::from_slice(&output.stdout).expect("restart JSON")
}

fn restart_arguments<'a>(
    fixture: &'a RunFixture,
    source_run_id: &'a str,
    key: &'a str,
) -> Vec<&'a str> {
    vec![
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "--idempotency-key",
        key,
        "-C",
        path_text(fixture.project.path()),
        "run",
        "restart",
        source_run_id,
    ]
}

fn assert_restart_prefix(fixture: &RunFixture, run_id: &str) {
    let inspection = run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "show",
        run_id,
    ]);
    assert_eq!(inspection["inspection"]["recovery"]["status"], "incomplete");
    let events = inspection["inspection"]["events"]
        .as_array()
        .expect("event array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["type"], "run_started");
}

fn seed_source(fixture: &RunFixture, run_id: &str, key: &str, terminal: bool) {
    let store = fixture_store(fixture);
    let project = store
        .open_project(
            &fixture
                .project
                .path()
                .canonicalize()
                .expect("canonical Project"),
        )
        .expect("open Project");
    let begin = BeginRun {
        v: RUN_STORE_VERSION,
        run_id: run_id.into(),
        conversation_id: fixture.conversation_id.clone(),
        prompt_id: fixture.prompt_id.clone(),
        project_id: project.id,
        execution: RunExecution {
            provider: RunProvider::OpenAiResponses {
                endpoint: "https://api.openai.com/v1".into(),
                model: "gpt-5.6-sol".into(),
            },
            system_prompt: "Answer the user without tools.".into(),
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
    };
    store.begin_run(&begin).expect("begin source Run");
    if terminal {
        append_terminal(&store, &begin);
    }
}

fn append_terminal(store: &impl RunStore, run: &BeginRun) {
    let answer = "terminal source answer";
    let kinds = [
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
                text: answer.into(),
                tool_calls: Vec::new(),
            },
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: answer.into(),
            },
        },
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: run.conversation_id.clone(),
                run_id: run.run_id.clone(),
                seq: u64::try_from(index + 1).expect("event sequence"),
                emitted_at_ms: u64::try_from(index + 1).expect("event time"),
                kind,
            })
            .expect("append terminal event");
    }
}

fn append_target_events(
    fixture: &RunFixture,
    run_id: &str,
    first_sequence: u64,
    kinds: Vec<RuntimeEventKind>,
) {
    let store = fixture_store(fixture);
    for (offset, kind) in kinds.into_iter().enumerate() {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: fixture.conversation_id.clone(),
                run_id: run_id.into(),
                seq: first_sequence + u64::try_from(offset).expect("event offset"),
                emitted_at_ms: first_sequence + u64::try_from(offset).expect("event time"),
                kind,
            })
            .expect("append target event");
    }
}
