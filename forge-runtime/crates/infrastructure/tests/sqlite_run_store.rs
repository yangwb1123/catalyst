use std::{
    fs,
    sync::{Arc, Barrier},
};

use forge_runtime_domain::{
    BeginRun, BeginRunDisposition, ConversationScope, EventSink, HubStore,
    MAX_RUN_EVENT_JSON_BYTES, PROTOCOL_VERSION, RUN_STORE_VERSION, RunOutcome, RunRecoveryState,
    RunStore, RunStoreError, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::{DurableFirstEventSink, SqliteHubStore};
use tempfile::TempDir;

#[path = "support/run_store.rs"]
mod run_store_support;

use run_store_support::{
    CountingSink, ObservingFailSink, assistant, current_user, execution, run_started, tool_call,
};

struct Fixture {
    _root: TempDir,
    store: SqliteHubStore,
    project_id: String,
    conversation_id: String,
    prompt_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("Run store root");
        let project_path = root.path().join("project");
        fs::create_dir(&project_path).expect("project directory");
        let project_path = project_path.canonicalize().expect("canonical project");
        let database = root.path().join("private-state").join("hub.sqlite3");
        let store = SqliteHubStore::open(database).expect("open Run store");
        let project = store.open_project(&project_path).expect("Project");
        let conversation = store
            .create_conversation(
                &ConversationScope::Project(project.id.clone()),
                "Runtime",
                "conversation-key",
            )
            .expect("Conversation");
        let prompt = store
            .append_prompt(&conversation.id, "user", "inspect README", "prompt-key")
            .expect("Prompt");
        Self {
            _root: root,
            store,
            project_id: project.id,
            conversation_id: conversation.id,
            prompt_id: prompt.id,
        }
    }

    fn begin(&self, key: &str) -> BeginRun {
        BeginRun {
            v: RUN_STORE_VERSION,
            run_id: "run-1".into(),
            conversation_id: self.conversation_id.clone(),
            prompt_id: self.prompt_id.clone(),
            project_id: self.project_id.clone(),
            execution: execution(),
            idempotency_key: key.into(),
            created_at_ms: 10,
        }
    }

    fn event(&self, seq: u64, kind: RuntimeEventKind) -> RuntimeEvent {
        RuntimeEvent {
            v: PROTOCOL_VERSION,
            session_id: self.conversation_id.clone(),
            run_id: "run-1".into(),
            seq,
            emitted_at_ms: 10 + seq,
            kind,
        }
    }
}

#[test]
fn begin_is_project_bound_and_reports_created_or_replayed_atomically() {
    let fixture = Fixture::new();
    let created = fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let mut retry = fixture.begin("run-key");
    retry.run_id = "newly-generated-retry-id".into();
    retry.created_at_ms = 999;
    let replayed = fixture.store.begin_run(&retry).expect("semantic replay");

    assert_eq!(created.disposition, BeginRunDisposition::Created);
    assert_eq!(replayed.disposition, BeginRunDisposition::Replayed);
    assert_eq!(replayed.run, created.run);
    assert_eq!(replayed.prompt.content, "inspect README");
    assert_eq!(
        fixture.store.list_runs(None, 10).expect("list Runs"),
        [created.run]
    );
}

#[test]
fn begin_rejects_non_project_scope_and_mismatched_prompt() {
    let fixture = Fixture::new();
    let global = fixture
        .store
        .create_conversation(&ConversationScope::Global, "Global", "global-key")
        .expect("Global Conversation");
    let global_prompt = fixture
        .store
        .append_prompt(&global.id, "user", "global", "global-prompt")
        .expect("Global Prompt");
    let mut invalid = fixture.begin("invalid-run");
    invalid.conversation_id = global.id;
    invalid.prompt_id = global_prompt.id;
    assert!(matches!(
        fixture.store.begin_run(&invalid),
        Err(RunStoreError::Conflict { .. })
    ));

    let second = fixture
        .store
        .append_prompt(
            &fixture.conversation_id,
            "assistant",
            "not user input",
            "assistant-prompt",
        )
        .expect("assistant Prompt");
    invalid = fixture.begin("assistant-run");
    invalid.prompt_id = second.id;
    assert!(matches!(
        fixture.store.begin_run(&invalid),
        Err(RunStoreError::Conflict { .. })
    ));
}

#[test]
fn append_is_contiguous_idempotent_and_terminal() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let started = fixture.event(1, run_started());
    let skipped = fixture.event(2, RuntimeEventKind::TurnStarted { turn: 1 });
    assert!(fixture.store.append_event(&skipped).is_err());
    fixture.store.append_event(&started).expect("first event");
    fixture.store.append_event(&started).expect("exact replay");

    let divergent = fixture.event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: "different".into(),
        },
    );
    assert!(matches!(
        fixture.store.append_event(&divergent),
        Err(RunStoreError::Conflict { .. })
    ));
    commit_completed(&fixture);
    assert!(fixture.store.append_event(&skipped).is_err());
    let inspection = fixture.store.inspect_run("run-1").expect("inspect");
    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { .. }
        }
    ));
}

fn commit_completed(fixture: &Fixture) {
    append_event(fixture, 2, current_user());
    append_event(fixture, 3, RuntimeEventKind::TurnStarted { turn: 1 });
    append_event(fixture, 4, assistant("done", Vec::new()));
    append_event(
        fixture,
        5,
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: "done".into(),
            },
        },
    );
}

#[test]
fn first_event_must_repeat_the_bound_prompt_exactly() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let mismatched = fixture.event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: "different prompt".into(),
        },
    );

    assert!(matches!(
        fixture.store.append_event(&mismatched),
        Err(RunStoreError::Conflict { .. })
    ));
    assert!(
        fixture
            .store
            .inspect_run("run-1")
            .expect("Run remains inspectable")
            .events
            .is_empty()
    );
}

#[test]
fn oversized_event_is_rejected_before_it_reaches_sqlite() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let oversized = fixture.event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: "x".repeat(MAX_RUN_EVENT_JSON_BYTES + 1),
        },
    );

    assert!(matches!(
        fixture.store.append_event(&oversized),
        Err(RunStoreError::Conflict { .. })
    ));
    assert!(
        fixture
            .store
            .inspect_run("run-1")
            .expect("Run remains inspectable")
            .events
            .is_empty()
    );
}

#[test]
fn unmatched_tool_start_remains_pending_and_never_becomes_terminal() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    append_event(&fixture, 1, run_started());
    append_event(&fixture, 2, current_user());
    append_event(&fixture, 3, RuntimeEventKind::TurnStarted { turn: 1 });
    append_event(&fixture, 4, assistant("reading", vec![tool_call()]));
    append_event(
        &fixture,
        5,
        RuntimeEventKind::ToolStarted { call: tool_call() },
    );
    let inspection = fixture.store.inspect_run("run-1").expect("inspect");
    assert!(matches!(
        inspection.recovery.state,
        RunRecoveryState::PendingTool { ref calls } if calls == &[tool_call()]
    ));
    assert!(
        fixture
            .store
            .append_event(&fixture.event(
                6,
                RuntimeEventKind::RunFinished {
                    outcome: RunOutcome::Cancelled,
                },
            ))
            .is_err()
    );
}

#[test]
fn completed_run_atomically_authorizes_one_assistant_prompt() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    append_event(&fixture, 1, run_started());

    assert!(
        fixture
            .store
            .reconcile_completed_assistant("run-1")
            .is_err()
    );
    commit_completed(&fixture);
    let first = fixture
        .store
        .reconcile_completed_assistant("run-1")
        .expect("first reconciliation");
    let replay = fixture
        .store
        .reconcile_completed_assistant("run-1")
        .expect("idempotent reconciliation");

    assert_eq!(first, replay);
    assert_eq!(first.conversation_id, fixture.conversation_id);
    assert_eq!(first.role, "assistant");
    assert_eq!(first.content, "done");
    assert_eq!(
        fixture
            .store
            .list_prompts(Some(&fixture.conversation_id), 10)
            .expect("prompt list")
            .into_iter()
            .filter(|prompt| prompt.role == "assistant")
            .count(),
        1
    );
}

#[test]
fn delayed_writeback_remains_causally_attached_to_its_user_prompt() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin P Run");
    append_event(&fixture, 1, run_started());
    commit_completed(&fixture);
    fixture
        .store
        .append_prompt(&fixture.conversation_id, "user", "Q", "prompt-q")
        .expect("append Q before recovering P");
    fixture
        .store
        .reconcile_completed_assistant("run-1")
        .expect("recover P assistant after Q");
    let current = fixture
        .store
        .append_prompt(&fixture.conversation_id, "user", "R", "prompt-r")
        .expect("append R");

    let mut history = fixture
        .store
        .list_prompts_before(&fixture.conversation_id, &current.id, 10)
        .expect("causal history before R");
    history.reverse();
    let messages: Vec<_> = history
        .iter()
        .map(|prompt| (prompt.role.as_str(), prompt.content.as_str()))
        .collect();

    assert_eq!(
        messages,
        [
            ("user", "inspect README"),
            ("assistant", "done"),
            ("user", "Q")
        ]
    );
}

#[test]
fn non_completed_terminal_outcome_cannot_create_an_assistant_prompt() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    append_event(&fixture, 1, run_started());
    append_event(&fixture, 2, current_user());
    append_event(
        &fixture,
        3,
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Cancelled,
        },
    );

    assert!(
        fixture
            .store
            .reconcile_completed_assistant("run-1")
            .is_err()
    );
    assert_eq!(
        fixture
            .store
            .list_prompts(Some(&fixture.conversation_id), 10)
            .expect("prompt list")
            .len(),
        1
    );
}

#[test]
fn durable_sink_commits_before_downstream_and_stops_on_store_failure() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let event = fixture.event(1, run_started());
    let observer_store = fixture.store.clone();
    let mut observer = ObservingFailSink {
        store: observer_store,
        observed_durable_event: false,
    };
    {
        let mut sink = DurableFirstEventSink::new(&fixture.store, &mut observer);
        sink.emit(&event).expect_err("downstream failure surfaces");
    }
    assert!(observer.observed_durable_event);

    let mut counter = CountingSink::default();
    let missing = RuntimeEvent {
        run_id: "missing".into(),
        ..event
    };
    {
        let mut sink = DurableFirstEventSink::new(&fixture.store, &mut counter);
        sink.emit(&missing)
            .expect_err("store failure surfaces first");
    }
    assert_eq!(counter.events, 0);
}

#[test]
fn concurrent_divergent_appends_cannot_fork_one_sequence() {
    let fixture = Fixture::new();
    fixture
        .store
        .begin_run(&fixture.begin("run-key"))
        .expect("begin");
    let barrier = Arc::new(Barrier::new(2));
    let mut workers = Vec::new();
    for emitted_at_ms in [101, 102] {
        let store = fixture.store.clone();
        let barrier = Arc::clone(&barrier);
        let mut event = fixture.event(1, run_started());
        event.emitted_at_ms = emitted_at_ms;
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            store.append_event(&event)
        }));
    }
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().expect("worker"))
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        fixture
            .store
            .inspect_run("run-1")
            .expect("inspect")
            .events
            .len(),
        1
    );
}

fn append_event(fixture: &Fixture, seq: u64, kind: RuntimeEventKind) {
    fixture
        .store
        .append_event(&fixture.event(seq, kind))
        .expect("append event");
}
