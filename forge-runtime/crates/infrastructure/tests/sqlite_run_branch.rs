use std::{
    fs,
    path::{Path, PathBuf},
};

use forge_runtime_domain::{
    BeginRun, BeginRunBranch, BeginRunDisposition, ConversationScope, HubStore, Message,
    PROTOCOL_VERSION, RUN_LINEAGE_VERSION, RUN_STORE_VERSION, RunExecution, RunJournalCursor,
    RunLimits, RunOutcome, RunProvider, RunStore, RunStoreError, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::{Connection, params};
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    database: PathBuf,
    store: SqliteHubStore,
}

#[test]
fn complete_branch_replay_is_read_only_and_succeeds() {
    let fixture = Fixture::new();
    let request = branch_request();
    let created = fixture
        .store
        .begin_run_branch(&request)
        .expect("create complete branch");
    let before = branch_state(&fixture.database, &created.run.run_id);

    let replayed = fixture
        .store
        .begin_run_branch(&BeginRunBranch {
            created_at_ms: 999,
            ..request
        })
        .expect("replay complete branch");

    assert_eq!(created.disposition, BeginRunDisposition::Created);
    assert_eq!(replayed.disposition, BeginRunDisposition::Replayed);
    assert_eq!(replayed.run, created.run);
    assert_eq!(replayed.lineage, created.lineage);
    assert_eq!(branch_state(&fixture.database, &created.run.run_id), before);
}

#[test]
fn replay_rejects_missing_seed_without_repairing_it() {
    let fixture = Fixture::new();
    let request = branch_request();
    let created = fixture
        .store
        .begin_run_branch(&request)
        .expect("create branch before seed loss");
    remove_seed_and_restore_empty_cursor(&fixture.database, &created.run);
    let before = branch_state(&fixture.database, &created.run.run_id);

    let error = fixture
        .store
        .begin_run_branch(&request)
        .expect_err("partial branch replay must fail closed");

    assert_corrupt(error, "missing its root run_started seed event");
    assert_eq!(branch_state(&fixture.database, &created.run.run_id), before);
    assert_eq!(before.0, 0, "fixture must expose the repairable old shape");
    assert_eq!(before.1, 1, "lineage remains present");
}

#[test]
fn replay_rejects_missing_lineage_as_corrupt_without_mutation() {
    let fixture = Fixture::new();
    let request = branch_request();
    let created = fixture
        .store
        .begin_run_branch(&request)
        .expect("create branch before lineage loss");
    let connection = Connection::open(&fixture.database).expect("open raw branch Hub");
    connection
        .execute(
            "DELETE FROM run_lineages WHERE child_run_id=?1",
            [&created.run.run_id],
        )
        .expect("remove lineage sidecar");
    drop(connection);
    let before = branch_state(&fixture.database, &created.run.run_id);

    let error = fixture
        .store
        .begin_run_branch(&request)
        .expect_err("branch replay without lineage must fail closed");

    assert_corrupt(error, "missing its lineage record");
    assert_eq!(branch_state(&fixture.database, &created.run.run_id), before);
    assert_eq!(before.0, 1, "root seed remains present");
    assert_eq!(before.1, 0, "lineage fixture is absent");
}

#[test]
fn branch_rejects_parent_with_corrupt_direct_lineage_atomically() {
    let fixture = Fixture::new();
    let parent = fixture
        .store
        .begin_run_branch(&branch_request())
        .expect("create branch parent");
    append_branch_terminal(&fixture.store, &parent.run);
    corrupt_lineage_digest(&fixture.database, &parent.run.run_id);
    let before = database_counts(&fixture.database);

    let error = fixture
        .store
        .begin_run_branch(&BeginRunBranch {
            v: RUN_LINEAGE_VERSION,
            child_run_id: "run-branch-2".into(),
            parent_run_id: parent.run.run_id,
            idempotency_key: "second-branch-key".into(),
            created_at_ms: 80,
        })
        .expect_err("corrupt branch parent must fail before child creation");

    assert_corrupt(error, "Run lineage digest does not match its fields");
    assert_eq!(database_counts(&fixture.database), before);
    assert_eq!(run_count(&fixture.database, "run-branch-2"), 0);
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("Run branch root");
        let project_path = root.path().join("project");
        fs::create_dir(&project_path).expect("project directory");
        let database = root.path().join("state").join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open Run branch store");
        let project = store
            .open_project(&project_path.canonicalize().expect("canonical project"))
            .expect("Project");
        let conversation = store
            .create_conversation(
                &ConversationScope::Project(project.id.clone()),
                "Branch",
                "conversation-key",
            )
            .expect("Conversation");
        let prompt = store
            .append_prompt(&conversation.id, "user", "inspect README", "prompt-key")
            .expect("Prompt");
        let parent = BeginRun {
            v: RUN_STORE_VERSION,
            run_id: "run-parent".into(),
            conversation_id: conversation.id,
            prompt_id: prompt.id,
            project_id: project.id,
            execution: execution(),
            idempotency_key: "parent-key".into(),
            created_at_ms: 10,
        };
        store.begin_run(&parent).expect("begin parent Run");
        append_terminal(&store, &parent);
        Self {
            _root: root,
            database,
            store,
        }
    }
}

fn branch_request() -> BeginRunBranch {
    BeginRunBranch {
        v: RUN_LINEAGE_VERSION,
        child_run_id: "run-branch-1".into(),
        parent_run_id: "run-parent".into(),
        idempotency_key: "branch-key".into(),
        created_at_ms: 50,
    }
}

fn execution() -> RunExecution {
    RunExecution {
        provider: RunProvider::DeterministicRead {
            path: "README.md".into(),
        },
        system_prompt: "Read only what is authorized.".into(),
        allowed_read_paths: vec!["README.md".into()],
        limits: RunLimits::default(),
    }
}

fn append_terminal(store: &impl RunStore, run: &BeginRun) {
    let kinds = [
        RuntimeEventKind::RunStarted {
            prompt: "inspect README".into(),
        },
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "inspect README".into(),
            },
        },
        RuntimeEventKind::TurnStarted { turn: 1 },
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: "done".into(),
                tool_calls: Vec::new(),
            },
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: "done".into(),
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
                emitted_at_ms: u64::try_from(index + 11).expect("event time"),
                kind,
            })
            .expect("append parent event");
    }
}

fn append_branch_terminal(store: &impl RunStore, run: &forge_runtime_domain::RunRecord) {
    let kinds = [
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: "inspect README".into(),
            },
        },
        RuntimeEventKind::TurnStarted { turn: 1 },
        RuntimeEventKind::MessageCommitted {
            message: Message::Assistant {
                text: "done".into(),
                tool_calls: Vec::new(),
            },
        },
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed {
                answer: "done".into(),
            },
        },
    ];
    for (index, kind) in kinds.into_iter().enumerate() {
        store
            .append_event(&RuntimeEvent {
                v: PROTOCOL_VERSION,
                session_id: run.conversation_id.clone(),
                run_id: run.run_id.clone(),
                seq: u64::try_from(index + 2).expect("branch event sequence"),
                emitted_at_ms: u64::try_from(index + 51).expect("branch event time"),
                kind,
            })
            .expect("append branch parent event");
    }
}

fn remove_seed_and_restore_empty_cursor(database: &Path, run: &forge_runtime_domain::RunRecord) {
    let empty = RunJournalCursor::new(run).expect("empty child cursor");
    let empty = serde_json::to_string(&empty).expect("encode empty child cursor");
    let connection = Connection::open(database).expect("open raw branch Hub");
    connection
        .execute("DELETE FROM run_events WHERE run_id=?1", [&run.run_id])
        .expect("remove branch seed");
    connection
        .execute(
            "UPDATE runs SET cursor_json=?1,journal_bytes=0 WHERE id=?2",
            params![empty, run.run_id],
        )
        .expect("restore internally consistent empty child journal");
}

fn branch_state(database: &Path, run_id: &str) -> (i64, i64, String, i64) {
    let connection = Connection::open(database).expect("open branch state");
    connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM run_events WHERE run_id=?1),
               (SELECT COUNT(*) FROM run_lineages WHERE child_run_id=?1),
               cursor_json,journal_bytes
             FROM runs WHERE id=?1",
            [run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read branch state")
}

fn corrupt_lineage_digest(database: &Path, run_id: &str) {
    let connection = Connection::open(database).expect("open lineage corruption fixture");
    connection
        .execute(
            "UPDATE run_lineages SET source_event_sha256=zeroblob(32) WHERE child_run_id=?1",
            [run_id],
        )
        .expect("corrupt direct lineage digest binding");
}

fn database_counts(database: &Path) -> (i64, i64, i64) {
    let connection = Connection::open(database).expect("open aggregate counts");
    connection
        .query_row(
            "SELECT
               (SELECT COUNT(*) FROM runs),
               (SELECT COUNT(*) FROM run_lineages),
               (SELECT COUNT(*) FROM run_events)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read aggregate counts")
}

fn run_count(database: &Path, run_id: &str) -> i64 {
    Connection::open(database)
        .expect("open Run count")
        .query_row("SELECT COUNT(*) FROM runs WHERE id=?1", [run_id], |row| {
            row.get(0)
        })
        .expect("read Run count")
}

fn assert_corrupt(error: RunStoreError, expected: &str) {
    let RunStoreError::Corrupt { message } = error else {
        panic!("expected durable corruption, got {error:?}");
    };
    assert!(message.contains(expected), "{message}");
}
