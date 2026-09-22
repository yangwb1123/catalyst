use std::{
    fs,
    sync::{Arc, Barrier},
};

use forge_runtime_domain::{
    BeginRun, BeginRunDisposition, ConversationScope, EventSink, HubStore,
    MAX_RUN_EVENT_JSON_BYTES, PROTOCOL_VERSION, RUN_STORE_VERSION, RunOutcome, RunRecoveryState,
    RunResumePoint, RunStore, RunStoreError, RuntimeEvent, RuntimeEventKind, ToolCall,
    execution::fabric::{
        EXECUTION_FABRIC_ABI_VERSION, ExecutionEvidence, ExecutionEvidenceSource, ExecutionTarget,
        LocalProcessObservation,
    },
};
use forge_runtime_infrastructure::{DurableFirstEventSink, SqliteHubStore};
use tempfile::TempDir;

#[path = "support/run_store.rs"]
mod run_store_support;

use run_store_support::{
    CountingSink, ObservingFailSink, assistant, current_user, execution, run_started, tool_call,
};

#[path = "sqlite_run_store/begin_and_append.rs"]
mod begin_and_append;
#[path = "sqlite_run_store/reconciliation.rs"]
mod reconciliation;
#[path = "sqlite_run_store/recovery.rs"]
mod recovery;

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

fn append_event(fixture: &Fixture, seq: u64, kind: RuntimeEventKind) {
    fixture
        .store
        .append_event(&fixture.event(seq, kind))
        .expect("append event");
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
