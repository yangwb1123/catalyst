use std::fs;

use forge_runtime_domain::{
    BeginRun, ConversationScope, HubStore, PROTOCOL_VERSION, RUN_STORE_VERSION, RunExecution,
    RunLimits, RunProvider, RunStore, RuntimeEvent, RuntimeEventKind,
};
use tempfile::TempDir;

use super::{SqliteHubStore, run_read, run_write};

struct Fixture {
    _root: TempDir,
    store: SqliteHubStore,
    conversation_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("snapshot test root");
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
        let prompt = store
            .append_prompt(&conversation.id, "user", "inspect README", "prompt-key")
            .expect("Prompt");
        store
            .begin_run(&begin_run(&project.id, &conversation.id, &prompt.id))
            .expect("begin Run");
        Self {
            _root: root,
            store,
            conversation_id: conversation.id,
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
fn inspect_uses_one_snapshot_while_an_append_commits() {
    let fixture = Fixture::new();
    let first = fixture.event(
        1,
        RuntimeEventKind::RunStarted {
            prompt: "inspect README".into(),
        },
    );
    fixture.store.append_event(&first).expect("first event");
    let second = fixture.event(
        2,
        RuntimeEventKind::MessageCommitted {
            message: forge_runtime_domain::Message::User {
                text: "inspect README".into(),
            },
        },
    );
    let mut reader = fixture.store.connect_run().expect("reader");
    let mut writer = fixture.store.connect_run().expect("writer");

    let inspection = run_read::inspect_after_cursor(&mut reader, "run-1", || {
        run_write::append_event(&mut writer, &second)
    })
    .expect("consistent inspection");

    assert_eq!(inspection.events.as_slice(), std::slice::from_ref(&first));
    assert_eq!(
        fixture
            .store
            .inspect_run("run-1")
            .expect("latest inspection")
            .events,
        [first, second]
    );
}

fn begin_run(project_id: &str, conversation_id: &str, prompt_id: &str) -> BeginRun {
    BeginRun {
        v: RUN_STORE_VERSION,
        run_id: "run-1".into(),
        conversation_id: conversation_id.into(),
        prompt_id: prompt_id.into(),
        project_id: project_id.into(),
        execution: RunExecution {
            provider: RunProvider::DeterministicRead {
                path: "README.md".into(),
            },
            system_prompt: "Read only what is authorized.".into(),
            allowed_read_paths: vec!["README.md".into()],
            limits: RunLimits::default(),
        },
        idempotency_key: "run-key".into(),
        created_at_ms: 10,
    }
}
