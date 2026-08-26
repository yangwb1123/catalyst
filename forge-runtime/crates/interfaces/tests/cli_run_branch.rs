mod support;

use forge_runtime_domain::{
    BeginRun, HubStore, Message, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits,
    RunOutcome, RunProvider, RunStore, RuntimeEvent, RuntimeEventKind,
};
use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

use support::{
    RunFixture, assert_success, fixture, fixture_store, invoke, invoke_without_openai_key,
    parse_jsonl, path_text, run_json, start_arguments, text_field,
};

#[test]
fn terminal_root_branch_is_content_free_queryable_and_resumable() {
    let fixture = fixture();
    let source_output = invoke(&start_arguments(&fixture, "source-run-key"));
    assert_success(&source_output);
    let source_run_id = text_field(&parse_jsonl(&source_output)[0], &["run_id"]);
    let source_before = show_run(&fixture, &source_run_id);

    let created = branch_json(&fixture, &source_run_id, "branch-key");
    assert_eq!(created["type"], "run_branch_prepared");
    assert_eq!(created["disposition"], "created");
    assert_eq!(created["branch_mode"], "root_input");
    assert_eq!(created["parent_run_id"], source_run_id);
    assert_eq!(created["source_event_seq"], 1);
    assert_eq!(created["state"], "ready_to_resume");
    assert_eq!(created["resume_required"], true);
    assert_eq!(created["external_effects"], false);
    assert_eq!(created["journal_events"], 1);
    assert_eq!(created["context_snapshot_bound"], false);
    assert_eq!(created["workspace_snapshot_bound"], false);
    assert_content_free(&created);
    let child_run_id = text_field(&created, &["run_id"]);
    assert!(child_run_id.starts_with("run-branch-"));

    let replayed = branch_json(&fixture, &source_run_id, "branch-key");
    assert_eq!(replayed["disposition"], "replayed");
    assert_eq!(replayed["run_id"], child_run_id);
    assert_eq!(show_run(&fixture, &source_run_id), source_before);
    assert_lineage(&fixture, &child_run_id, &source_run_id);

    let resumed = invoke(&resume_arguments(&fixture, &child_run_id));
    assert_success(&resumed);
    let events = parse_jsonl(&resumed);
    assert_eq!(events[0]["seq"], 2);
    assert_eq!(events[0]["type"], "message_committed");

    let terminal = branch_json(&fixture, &source_run_id, "branch-key");
    assert_eq!(terminal["state"], "terminal");
    assert_eq!(terminal["resume_required"], false);
    assert_lineage(&fixture, &child_run_id, &source_run_id);

    remove_assistant_writeback(&fixture, &child_run_id);
    std::fs::remove_file(fixture.project.path().join("README.md"))
        .expect("remove workspace target before writeback recovery");
    let reconciled = invoke_without_openai_key(&resume_arguments(&fixture, &child_run_id));
    assert_success(&reconciled);
    assert!(reconciled.stdout.is_empty());
    assert_assistant_writeback(&fixture, &child_run_id);
}

#[test]
fn branch_refuses_nonterminal_parent_and_project_mismatch_without_child() {
    let fixture = fixture();
    seed_source(&fixture, "incomplete-parent", "incomplete-key", false);
    let incomplete = invoke(&branch_arguments(
        &fixture,
        "incomplete-parent",
        "incomplete-branch-key",
    ));
    assert!(!incomplete.status.success());
    assert!(incomplete.stdout.is_empty());
    assert!(String::from_utf8_lossy(&incomplete.stderr).contains("terminal"));

    seed_source(&fixture, "terminal-parent", "terminal-key", true);
    let other_project = TempDir::new().expect("other Project");
    let mismatch = invoke(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--idempotency-key",
        "mismatch-branch-key",
        "-C",
        path_text(other_project.path()),
        "run",
        "branch",
        "terminal-parent",
    ]);
    assert!(!mismatch.status.success());
    assert!(mismatch.stdout.is_empty());
    assert!(String::from_utf8_lossy(&mismatch.stderr).contains("does not match"));

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
fn live_branch_preparation_needs_no_credential_or_workspace_read() {
    let fixture = fixture();
    seed_source(&fixture, "live-parent", "live-parent-key", true);
    let read_target = fixture.project.path().join("README.md");
    std::fs::remove_file(read_target).expect("remove workspace target");

    let output = invoke_without_openai_key(&branch_arguments(
        &fixture,
        "live-parent",
        "live-branch-key",
    ));
    assert_success(&output);
    let prepared: Value = serde_json::from_slice(&output.stdout).expect("branch JSON");
    assert_eq!(prepared["external_effects"], false);
    assert_content_free(&prepared);
    let child_run_id = text_field(&prepared, &["run_id"]);

    let resume = invoke_without_openai_key(&resume_arguments(&fixture, &child_run_id));
    assert!(!resume.status.success());
    assert!(resume.stdout.is_empty());
    assert!(String::from_utf8_lossy(&resume.stderr).contains("OPENAI_API_KEY"));
}

#[test]
fn ordinary_run_reports_no_recorded_lineage() {
    let fixture = fixture();
    let source_output = invoke(&start_arguments(&fixture, "ordinary-key"));
    assert_success(&source_output);
    let run_id = text_field(&parse_jsonl(&source_output)[0], &["run_id"]);

    let lineage = lineage_json(&fixture, &run_id);
    assert_eq!(lineage["type"], "run_lineage");
    assert_eq!(lineage["run_id"], run_id);
    assert_eq!(lineage["recorded"], false);
    assert!(lineage["lineage"].is_null());
    assert_eq!(lineage["scope"], "direct_parent_only");
    assert_eq!(lineage["content_included"], false);
}

fn branch_json(fixture: &RunFixture, parent_run_id: &str, key: &str) -> Value {
    let output = invoke(&branch_arguments(fixture, parent_run_id, key));
    assert_success(&output);
    serde_json::from_slice(&output.stdout).expect("branch JSON")
}

fn branch_arguments<'a>(
    fixture: &'a RunFixture,
    parent_run_id: &'a str,
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
        "branch",
        parent_run_id,
    ]
}

fn resume_arguments<'a>(fixture: &'a RunFixture, run_id: &'a str) -> Vec<&'a str> {
    vec![
        "--state-dir",
        path_text(fixture.state.path()),
        "-C",
        path_text(fixture.project.path()),
        "run",
        "resume",
        run_id,
    ]
}

fn lineage_json(fixture: &RunFixture, run_id: &str) -> Value {
    run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "lineage",
        run_id,
    ])
}

fn assert_lineage(fixture: &RunFixture, child_run_id: &str, parent_run_id: &str) {
    let lineage = lineage_json(fixture, child_run_id);
    assert_eq!(lineage["recorded"], true);
    assert_eq!(lineage["lineage"]["child_run_id"], child_run_id);
    assert_eq!(lineage["lineage"]["parent_run_id"], parent_run_id);
    assert_eq!(lineage["lineage"]["branch_mode"], "root_input");
    assert_eq!(lineage["lineage"]["source_event_seq"], 1);
    assert_eq!(lineage["scope"], "direct_parent_only");
    assert_eq!(lineage["content_included"], false);
    assert_eq!(
        lineage["lineage"]["source_event_sha256"]
            .as_str()
            .map(str::len),
        Some(64)
    );
    assert_content_free(&lineage);
}

fn show_run(fixture: &RunFixture, run_id: &str) -> Value {
    run_json(&[
        "--state-dir",
        path_text(fixture.state.path()),
        "--json",
        "run",
        "show",
        run_id,
    ])
}

fn remove_assistant_writeback(fixture: &RunFixture, run_id: &str) {
    let connection =
        Connection::open(fixture.state.path().join("hub.sqlite3")).expect("open Hub fixture");
    let deleted = connection
        .execute(
            "DELETE FROM prompts WHERE id = (
               SELECT prompt_id FROM run_assistant_prompts WHERE run_id = ?1
             )",
            [run_id],
        )
        .expect("simulate terminal-before-writeback interruption");
    assert_eq!(deleted, 1);
}

fn assert_assistant_writeback(fixture: &RunFixture, run_id: &str) {
    let connection =
        Connection::open(fixture.state.path().join("hub.sqlite3")).expect("open Hub fixture");
    let (role, content): (String, String) = connection
        .query_row(
            "SELECT p.role,p.content FROM run_assistant_prompts w
             JOIN prompts p ON p.id=w.prompt_id WHERE w.run_id=?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("reconciled assistant writeback");
    assert_eq!(role, "assistant");
    assert!(content.contains("Read README.md successfully"));
}

fn assert_content_free(value: &Value) {
    let encoded = value.to_string();
    for forbidden in [
        "inspect the durable workspace",
        "Read README.md successfully",
        "terminal source answer",
        "Answer the user without tools",
        "gpt-5.6-sol",
        "README.md",
    ] {
        assert!(!encoded.contains(forbidden), "leaked {forbidden}");
    }
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
        execution: live_execution(),
        idempotency_key: key.into(),
        created_at_ms: 1,
    };
    store.begin_run(&begin).expect("begin source Run");
    if terminal {
        append_terminal(&store, &begin);
    }
}

fn live_execution() -> RunExecution {
    RunExecution {
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
