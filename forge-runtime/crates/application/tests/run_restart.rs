use std::{fs, sync::Arc};

use forge_runtime_application::{PrepareRunRestart, RunError, RunService};
use forge_runtime_domain::{
    BeginRun, BeginRunDisposition, ConversationScope, HubStore, Message, PROTOCOL_VERSION,
    RUN_STORE_VERSION, RunExecution, RunLimits, RunOutcome, RunProvider, RunResumePoint, RunStore,
    RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    store: Arc<SqliteHubStore>,
    source: BeginRun,
}

#[test]
fn terminal_run_restart_is_resume_ready_and_exactly_replayable() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let request = restart_request("restart-key", 50);

    let created = service.prepare_restart(&request).expect("prepare restart");
    assert_eq!(created.disposition, BeginRunDisposition::Created);
    assert!(created.inspection.run.run_id.starts_with("run-restart-"));
    assert_eq!(created.inspection.run.execution, fixture.source.execution);
    assert_eq!(created.inspection.events.len(), 1);
    assert!(matches!(
        created.inspection.resume_point(),
        Ok(RunResumePoint::CommitUser { prompt }) if prompt == "restart this run"
    ));
    assert_eq!(created.inspection.events[0].emitted_at_ms, 50);

    let replayed = service
        .prepare_restart(&PrepareRunRestart {
            created_at_ms: 999,
            ..request
        })
        .expect("replay restart");
    assert_eq!(replayed.disposition, BeginRunDisposition::Replayed);
    assert_eq!(
        replayed.inspection.run.run_id,
        created.inspection.run.run_id
    );
    assert_eq!(replayed.inspection.events, created.inspection.events);
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 2);
}

#[test]
fn restart_key_is_bound_to_the_selected_source_identity() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let first = service
        .prepare_restart(&restart_request("restart-key", 50))
        .expect("prepare first restart");
    let mut second_source = fixture.source.clone();
    second_source.run_id = "source-run-two".into();
    second_source.idempotency_key = "source-key-two".into();
    second_source.created_at_ms = 20;
    service
        .begin_run(&second_source)
        .expect("begin equivalent source");
    append_terminal(&fixture.store, &second_source);

    let error = service
        .prepare_restart(&PrepareRunRestart {
            source_run_id: second_source.run_id,
            idempotency_key: "restart-key".into(),
            created_at_ms: 60,
        })
        .expect_err("same key cannot select another source");

    assert!(matches!(error, RunError::RestartIdempotencyConflict));
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 3);
    assert_eq!(first.inspection.events.len(), 1);
}

#[test]
fn restart_run_id_namespace_is_reserved_from_standard_begin() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let mut standard = fixture.source.clone();
    standard.run_id = "run-restart-caller-selected".into();
    standard.idempotency_key = "standard-key".into();

    let error = service
        .begin_run(&standard)
        .expect_err("standard begin cannot claim restart namespace");

    assert!(matches!(error, RunError::ReservedRestartRunId));
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 1);
}

#[test]
fn standard_begin_cannot_replay_a_restart_owned_key() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let restarted = service
        .prepare_restart(&restart_request("restart-key", 50))
        .expect("prepare restart");
    let mut standard_retry = fixture.source.clone();
    standard_retry.run_id = "ordinary-retry".into();
    standard_retry.idempotency_key = "restart-key".into();
    standard_retry.created_at_ms = 60;

    let error = service
        .begin_run(&standard_retry)
        .expect_err("ordinary begin cannot replay restart identity");

    assert!(matches!(error, RunError::RestartIdempotencyConflict));
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 2);
    assert_eq!(restarted.inspection.events.len(), 1);
}

#[test]
fn restart_refuses_nonterminal_source_without_creating_a_run() {
    let fixture = fixture(false);
    let service = RunService::new(fixture.store.clone());

    let error = service
        .prepare_restart(&restart_request("restart-key", 50))
        .expect_err("incomplete source must fail");

    assert!(matches!(error, RunError::RestartSourceNotTerminal));
    assert_eq!(service.list_runs(None, 10).expect("list Runs").len(), 1);
}

#[test]
fn restart_key_owned_by_a_standard_run_fails_without_seeding_it() {
    let fixture = fixture(true);
    let service = RunService::new(fixture.store.clone());
    let mut standard = fixture.source.clone();
    standard.run_id = "run-standard".into();
    standard.idempotency_key = "restart-key".into();
    standard.created_at_ms = 40;
    standard.execution.system_prompt = "different standard input".into();
    service.begin_run(&standard).expect("seed standard Run");

    let error = service
        .prepare_restart(&restart_request("restart-key", 50))
        .expect_err("cross-operation key must fail");

    assert!(matches!(error, RunError::RestartIdempotencyConflict));
    assert!(
        service
            .inspect_run("run-standard")
            .expect("inspect standard Run")
            .events
            .is_empty()
    );
}

fn fixture(terminal: bool) -> Fixture {
    let root = TempDir::new().expect("temporary root");
    let project_path = root.path().join("project");
    fs::create_dir(&project_path).expect("project directory");
    let store = Arc::new(
        SqliteHubStore::open(root.path().join("state").join("hub.sqlite3"))
            .expect("open Hub store"),
    );
    let project = store
        .open_project(&project_path.canonicalize().expect("canonical project"))
        .expect("open Project");
    let conversation = store
        .create_conversation(
            &ConversationScope::Project(project.id.clone()),
            "Restart",
            "conversation-key",
        )
        .expect("create Conversation");
    let prompt = store
        .append_prompt(&conversation.id, "user", "restart this run", "prompt-key")
        .expect("append Prompt");
    let source = BeginRun {
        v: RUN_STORE_VERSION,
        run_id: "source-run".into(),
        conversation_id: conversation.id,
        prompt_id: prompt.id,
        project_id: project.id,
        execution: execution(),
        idempotency_key: "source-key".into(),
        created_at_ms: 10,
    };
    store.begin_run(&source).expect("begin source Run");
    if terminal {
        append_terminal(&store, &source);
    }
    Fixture {
        _root: root,
        store,
        source,
    }
}

fn restart_request(key: &str, created_at_ms: u64) -> PrepareRunRestart {
    PrepareRunRestart {
        source_run_id: "source-run".into(),
        idempotency_key: key.into(),
        created_at_ms,
    }
}

fn execution() -> RunExecution {
    RunExecution {
        provider: RunProvider::DeterministicRead {
            path: "README.md".into(),
        },
        system_prompt: "read only".into(),
        allowed_read_paths: vec!["README.md".into()],
        limits: RunLimits {
            max_turns: 4,
            max_tool_calls: 4,
            max_tool_output_bytes: 1_024,
            max_model_output_bytes: 1_024,
            max_model_events: 128,
            max_output_tokens_per_turn: 256,
        },
    }
}

fn append_terminal(store: &SqliteHubStore, run: &BeginRun) {
    let answer = "terminal answer";
    let kinds = [
        RuntimeEventKind::RunStarted {
            prompt: "restart this run".into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "restart this run".into(),
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
            .expect("append source event");
    }
}
