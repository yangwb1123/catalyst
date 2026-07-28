use std::{fs, path::PathBuf, sync::Arc};

use forge_runtime_application::{ConversationHistoryBridge, HistoryError};
use forge_runtime_domain::{
    BeginRun, ConversationScope, HubStore, HubStoreError, Message, PROTOCOL_VERSION,
    RUN_STORE_VERSION, RunExecution, RunLimits, RunOutcome, RunProvider, RunStore, RuntimeEvent,
    RuntimeEventKind,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::{Connection, params};
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    database: PathBuf,
    store: Arc<SqliteHubStore>,
    project_id: String,
    conversation_id: String,
    source_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("state root");
        let project_path = root.path().join("project");
        fs::create_dir(&project_path).expect("project directory");
        let project_path = project_path.canonicalize().expect("canonical project");
        let database = root.path().join("private-state").join("hub.sqlite3");
        let store = Arc::new(SqliteHubStore::open(&database).expect("open Hub"));
        let project = store.open_project(&project_path).expect("project");
        let conversation = store
            .create_conversation(
                &ConversationScope::Project(project.id.clone()),
                "Causal history",
                "conversation-key",
            )
            .expect("conversation");
        let source = store
            .append_prompt(&conversation.id, "user", "P", "prompt-p")
            .expect("source prompt");
        Self {
            _root: root,
            database,
            store,
            project_id: project.id,
            conversation_id: conversation.id,
            source_id: source.id,
        }
    }

    fn begin(&self, run_id: &str) {
        self.begin_for(run_id, &self.source_id);
    }

    fn begin_for(&self, run_id: &str, prompt_id: &str) {
        self.store
            .begin_run(&BeginRun {
                v: RUN_STORE_VERSION,
                run_id: run_id.into(),
                conversation_id: self.conversation_id.clone(),
                prompt_id: prompt_id.into(),
                project_id: self.project_id.clone(),
                execution: execution(),
                idempotency_key: format!("key-{run_id}"),
                created_at_ms: 1,
            })
            .expect("begin Run");
    }

    fn complete(&self, run_id: &str, answer: &str) {
        self.complete_for(run_id, &self.source_id, "P", answer);
    }

    fn complete_for(&self, run_id: &str, prompt_id: &str, prompt: &str, answer: &str) {
        self.begin_for(run_id, prompt_id);
        for (seq, kind) in completed_events(prompt, answer).into_iter().enumerate() {
            self.store
                .append_event(&RuntimeEvent {
                    v: PROTOCOL_VERSION,
                    session_id: self.conversation_id.clone(),
                    run_id: run_id.into(),
                    seq: u64::try_from(seq + 1).expect("event sequence"),
                    emitted_at_ms: u64::try_from(seq + 1).expect("event time"),
                    kind,
                })
                .expect("append event");
        }
        self.store
            .reconcile_completed_assistant(run_id)
            .expect("assistant writeback");
    }

    fn current(&self, key: &str) -> String {
        self.store
            .append_prompt(&self.conversation_id, "user", key, key)
            .expect("current prompt")
            .id
    }
}

#[test]
fn many_answers_keep_their_source_and_the_fifteen_newest_answers() {
    let fixture = Fixture::new();
    for index in 0..16 {
        fixture.complete(&format!("run-{index:02}"), &format!("answer-{index:02}"));
    }
    let current = fixture.current("Q");

    let history = ConversationHistoryBridge::new(fixture.store.clone())
        .load_before(&fixture.conversation_id, &current, usize::MAX)
        .expect("bounded causal history");

    assert_eq!(history.messages().len(), 16);
    assert!(history.source_truncated());
    assert_eq!(history.omitted_messages_lower_bound(), 1);
    assert_eq!(history.messages()[0], Message::User { text: "P".into() });
    let answers: Vec<_> = history.messages()[1..].iter().map(assistant_text).collect();
    let expected: Vec<_> = (1..16).map(|index| format!("answer-{index:02}")).collect();
    assert_eq!(answers, expected);
}

#[test]
fn mixed_answer_groups_reserve_the_cutoff_groups_source() {
    let fixture = Fixture::new();
    for index in 0..16 {
        fixture.complete(
            &format!("run-p-{index:02}"),
            &format!("p-answer-{index:02}"),
        );
    }
    let q = fixture.current("Q");
    fixture.complete_for("run-q", &q, "Q", "q-answer");
    let current = fixture.current("R");

    let history = ConversationHistoryBridge::new(fixture.store.clone())
        .load_before(&fixture.conversation_id, &current, usize::MAX)
        .expect("mixed causal history");
    let messages = history.messages();

    assert_eq!(messages.len(), 16);
    assert!(history.source_truncated());
    assert_eq!(messages[0], Message::User { text: "P".into() });
    let p_answers: Vec<_> = messages[1..14].iter().map(assistant_text).collect();
    let expected: Vec<_> = (3..16)
        .map(|index| format!("p-answer-{index:02}"))
        .collect();
    assert_eq!(p_answers, expected);
    assert_eq!(messages[14], Message::User { text: "Q".into() });
    assert_eq!(assistant_text(&messages[15]), "q-answer");
}

#[test]
fn corrupt_run_assistant_associations_fail_before_history_is_returned() {
    assert_boundary_cannot_be_an_answer();
    assert_answer_cannot_cross_conversations();
    assert_run_source_must_remain_a_user();
}

fn assert_boundary_cannot_be_an_answer() {
    let fixture = Fixture::new();
    fixture.begin("run-boundary");
    let boundary = fixture.current("boundary");
    associate(&fixture.database, "run-boundary", &boundary);
    assert_corrupt(&fixture, &boundary);
}

fn assert_answer_cannot_cross_conversations() {
    let fixture = Fixture::new();
    fixture.begin("run-foreign");
    let foreign = fixture
        .store
        .create_conversation(&ConversationScope::Global, "Foreign", "foreign-key")
        .expect("foreign conversation");
    let answer = fixture
        .store
        .append_prompt(&foreign.id, "assistant", "foreign", "foreign-answer")
        .expect("foreign answer");
    associate(&fixture.database, "run-foreign", &answer.id);
    let boundary = fixture.current("cross-conversation-boundary");
    assert_corrupt(&fixture, &boundary);
}

fn assert_run_source_must_remain_a_user() {
    let fixture = Fixture::new();
    fixture.begin("run-source-role");
    let answer = fixture
        .store
        .append_prompt(
            &fixture.conversation_id,
            "assistant",
            "answer",
            "source-role-answer",
        )
        .expect("answer");
    let connection = Connection::open(&fixture.database).expect("raw database");
    connection
        .execute(
            "UPDATE prompts SET role = 'assistant' WHERE id = ?1",
            [&fixture.source_id],
        )
        .expect("corrupt source role");
    associate_on(&connection, "run-source-role", &answer.id);
    let boundary = fixture.current("source-role-boundary");
    assert_corrupt(&fixture, &boundary);
}

fn assert_corrupt(fixture: &Fixture, boundary: &str) {
    let error = ConversationHistoryBridge::new(fixture.store.clone())
        .load_before(&fixture.conversation_id, boundary, usize::MAX)
        .expect_err("corrupt association must fail");
    assert!(matches!(
        error,
        HistoryError::Store(HubStoreError::Corrupt { .. })
    ));
}

fn associate(database: &PathBuf, run_id: &str, prompt_id: &str) {
    let connection = Connection::open(database).expect("raw database");
    associate_on(&connection, run_id, prompt_id);
}

fn associate_on(connection: &Connection, run_id: &str, prompt_id: &str) {
    connection
        .execute(
            "INSERT INTO run_assistant_prompts(run_id,prompt_id) VALUES(?1,?2)",
            params![run_id, prompt_id],
        )
        .expect("install association");
}

fn completed_events(prompt: &str, answer: &str) -> [RuntimeEventKind; 5] {
    [
        RuntimeEventKind::RunStarted {
            prompt: prompt.into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: prompt.into(),
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
    ]
}

fn execution() -> RunExecution {
    RunExecution {
        provider: RunProvider::DeterministicRead {
            path: "README.md".into(),
        },
        system_prompt: "test".into(),
        allowed_read_paths: vec!["README.md".into()],
        limits: RunLimits::default(),
    }
}

fn assistant_text(message: &Message) -> String {
    match message {
        Message::Assistant { text, .. } => text.clone(),
        other => panic!("expected assistant, got {other:?}"),
    }
}
