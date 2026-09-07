use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use forge_runtime_domain::{
    BeginRun, BeginRunDisposition, BeginRunWithPrompt, ConversationScope, HubStore, Message,
    PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution, RunLimits, RunProvider, RunStore,
    RuntimeEventKind,
};
use rusqlite::Connection;
use tempfile::TempDir;

use super::{SqliteHubStore, run_seed};

struct Fixture {
    _root: TempDir,
    store: SqliteHubStore,
    connection: Connection,
    request: BeginRunWithPrompt,
}

#[test]
fn late_prefix_failure_rolls_back_prompt_run_and_events_before_retry() {
    let mut fixture = fixture();
    fixture
        .connection
        .execute_batch(
            "CREATE TRIGGER fail_second_seed_event BEFORE INSERT ON run_events
             WHEN NEW.seq = 2 BEGIN SELECT RAISE(ABORT, 'injected seed failure'); END",
        )
        .expect("install late seed failure");

    assert!(run_seed::begin(&mut fixture.connection, &fixture.request).is_err());
    assert_eq!(row_count(&fixture.connection, "prompts"), 0);
    assert_eq!(row_count(&fixture.connection, "runs"), 0);
    assert_eq!(row_count(&fixture.connection, "run_events"), 0);

    fixture
        .connection
        .execute_batch("DROP TRIGGER fail_second_seed_event")
        .expect("remove late seed failure");
    let result = run_seed::begin(&mut fixture.connection, &fixture.request)
        .expect("same logical retry succeeds");
    assert_eq!(result.disposition, BeginRunDisposition::Created);
    assert_pristine_seed(&fixture.store, &result.run.run_id, "inspect README");
}

#[test]
fn logical_replay_keeps_original_ids_and_does_not_duplicate_prefix() {
    let fixture = fixture();
    let created = fixture
        .store
        .begin_run_with_prompt(&fixture.request)
        .expect("create atomic seed");
    let mut replay = fixture.request.clone();
    replay.run.run_id = "fresh-proposed-run".into();
    replay.run.prompt_id = "fresh-proposed-prompt".into();
    replay.run.created_at_ms = 999;

    let replayed = fixture
        .store
        .begin_run_with_prompt(&replay)
        .expect("replay atomic seed");

    assert_eq!(created.disposition, BeginRunDisposition::Created);
    assert_eq!(replayed.disposition, BeginRunDisposition::Replayed);
    assert_eq!(replayed.run, created.run);
    assert_eq!(replayed.prompt, created.prompt);
    assert_pristine_seed(&fixture.store, &created.run.run_id, "inspect README");
    assert_eq!(row_count(&fixture.connection, "prompts"), 1);
    assert_eq!(row_count(&fixture.connection, "runs"), 1);
    assert_eq!(row_count(&fixture.connection, "run_events"), 2);
}

#[test]
fn replay_rejects_and_does_not_repair_a_partial_seed_prefix() {
    let fixture = fixture();
    let created = fixture
        .store
        .begin_run_with_prompt(&fixture.request)
        .expect("create atomic seed");
    fixture
        .connection
        .execute(
            "DELETE FROM run_events WHERE run_id = ?1 AND seq = 2",
            [&created.run.run_id],
        )
        .expect("inject partial durable seed");

    let error = fixture
        .store
        .begin_run_with_prompt(&fixture.request)
        .expect_err("partial seed must fail closed");

    assert!(matches!(
        error,
        forge_runtime_domain::RunStoreError::Corrupt { .. }
    ));
    assert_eq!(row_count(&fixture.connection, "run_events"), 1);
}

#[test]
fn concurrent_seed_contenders_have_one_created_execution_owner() {
    let fixture = fixture();
    let barrier = Arc::new(Barrier::new(3));
    let contenders: Vec<_> = (0..2)
        .map(|index| {
            let store = fixture.store.clone();
            let barrier = barrier.clone();
            let mut request = fixture.request.clone();
            request.run.run_id = format!("contender-run-{index}");
            request.run.prompt_id = format!("contender-prompt-{index}");
            request.run.created_at_ms = request.run.created_at_ms.saturating_add(index);
            thread::spawn(move || {
                barrier.wait();
                store
                    .begin_run_with_prompt(&request)
                    .expect("concurrent atomic seed")
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = contenders
        .into_iter()
        .map(|contender| contender.join().expect("seed contender"))
        .collect();

    assert_eq!(
        results
            .iter()
            .filter(|result| result.disposition == BeginRunDisposition::Created)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| result.disposition == BeginRunDisposition::Replayed)
            .count(),
        1
    );
    assert_eq!(results[0].run, results[1].run);
    assert_eq!(results[0].prompt, results[1].prompt);
    assert_pristine_seed(&fixture.store, &results[0].run.run_id, "inspect README");
    assert_eq!(row_count(&fixture.connection, "prompts"), 1);
    assert_eq!(row_count(&fixture.connection, "runs"), 1);
    assert_eq!(row_count(&fixture.connection, "run_events"), 2);
}

fn fixture() -> Fixture {
    let root = TempDir::new().expect("atomic Run seed root");
    let project_path = root.path().join("project");
    fs::create_dir(&project_path).expect("project directory");
    let database = root.path().join("private-state").join("hub.sqlite3");
    let store = SqliteHubStore::open(database).expect("Run store");
    let project = store
        .open_project(&project_path.canonicalize().expect("canonical project"))
        .expect("Project");
    let conversation = store
        .create_conversation(
            &ConversationScope::Project(project.id.clone()),
            "Runtime",
            "conversation-key",
        )
        .expect("Conversation");
    let request = BeginRunWithPrompt {
        run: BeginRun {
            v: RUN_STORE_VERSION,
            run_id: "run-atomic-1".into(),
            conversation_id: conversation.id,
            prompt_id: "prompt-atomic-1".into(),
            project_id: project.id,
            execution: execution(),
            idempotency_key: "run-atomic-key".into(),
            created_at_ms: now_ms(),
        },
        prompt_content: "inspect README".into(),
        prompt_idempotency_key: "prompt-atomic-key".into(),
    };
    let connection = store.connect_run().expect("validated raw connection");
    Fixture {
        _root: root,
        store,
        connection,
        request,
    }
}

fn execution() -> RunExecution {
    RunExecution {
        provider: RunProvider::OpenAiAgent {
            endpoint: "http://127.0.0.1:1/v1/responses".into(),
            model: "offline-test-model".into(),
            dev: false,
            toolset_version: forge_runtime_domain::CURRENT_AGENT_TOOLSET_VERSION,
            workspace_identity: None,
        },
        system_prompt: "Operate only in the authorized workspace.".into(),
        allowed_read_paths: Vec::new(),
        limits: RunLimits::default(),
    }
}

fn assert_pristine_seed(store: &SqliteHubStore, run_id: &str, prompt: &str) {
    let inspection = store.inspect_run(run_id).expect("inspect atomic seed");
    assert_eq!(inspection.events.len(), 2);
    assert_eq!(inspection.events[0].v, PROTOCOL_VERSION);
    assert_eq!(inspection.events[0].seq, 1);
    assert_eq!(
        inspection.events[0].kind,
        RuntimeEventKind::RunStarted {
            prompt: prompt.into()
        }
    );
    assert_eq!(inspection.events[1].seq, 2);
    assert_eq!(
        inspection.events[1].kind,
        RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: prompt.into()
            }
        }
    );
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
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
