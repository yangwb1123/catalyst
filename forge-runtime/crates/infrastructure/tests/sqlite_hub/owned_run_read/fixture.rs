use forge_runtime_domain::{
    BeginRun, ConversationOwner, ConversationScope, HubStore, RUN_STORE_VERSION, RunExecution,
    RunLimits, RunProvider, RunStore,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::{Connection, params};
use serde_json::json;
use tempfile::TempDir;

use super::super::{fixture, project_directory};

pub(super) struct OwnedRunFixture {
    pub(super) root: TempDir,
    pub(super) store: SqliteHubStore,
    pub(super) project_id: String,
    pub(super) conversation_a: String,
    pub(super) conversation_b: String,
    pub(super) owner_a: ConversationOwner,
    pub(super) owner_b: ConversationOwner,
}

impl OwnedRunFixture {
    pub(super) fn new() -> Self {
        let (root, store) = fixture();
        let path = project_directory(&root, "project");
        let project = store.open_project(&path).expect("register Project");
        let owner_a = owner("account-a");
        let owner_b = owner("account-b");
        let conversation_a = store
            .create_owned_conversation(
                &owner_a,
                &ConversationScope::Project(project.id.clone()),
                "A conversation",
                "conversation-a-key",
            )
            .expect("create owner A Conversation");
        let conversation_b = store
            .create_owned_conversation(
                &owner_b,
                &ConversationScope::Project(project.id.clone()),
                "B conversation",
                "conversation-b-key",
            )
            .expect("create owner B Conversation");
        Self {
            root,
            store,
            project_id: project.id,
            conversation_a: conversation_a.id,
            conversation_b: conversation_b.id,
            owner_a,
            owner_b,
        }
    }

    pub(super) fn add_run(
        &self,
        owner: &ConversationOwner,
        conversation_id: &str,
        run_id: &str,
        created_at_ms: u64,
    ) -> RunFixture {
        let expected_version = self.owned_version(owner, conversation_id);
        let prompt = self
            .store
            .append_owned_prompt(
                owner,
                conversation_id,
                "private prompt body",
                &format!("private-prompt-key-{run_id}"),
                expected_version,
            )
            .expect("append owner Prompt");
        self.begin_fixture_run(conversation_id, run_id, created_at_ms, &prompt.prompt.id)
    }

    fn owned_version(&self, owner: &ConversationOwner, conversation_id: &str) -> u64 {
        self.store
            .list_owned_conversations(owner, None, 10)
            .expect("list current owner Conversations")
            .conversations
            .into_iter()
            .find(|entry| entry.conversation.id == conversation_id)
            .expect("owned Conversation exists")
            .aggregate_version
    }

    fn begin_fixture_run(
        &self,
        conversation_id: &str,
        run_id: &str,
        created_at_ms: u64,
        prompt_id: &str,
    ) -> RunFixture {
        self.store
            .begin_run(&BeginRun {
                v: RUN_STORE_VERSION,
                run_id: run_id.into(),
                conversation_id: conversation_id.into(),
                prompt_id: prompt_id.into(),
                project_id: self.project_id.clone(),
                execution: RunExecution {
                    provider: RunProvider::DeterministicRead {
                        path: "private execution path".into(),
                    },
                    system_prompt: "private system prompt".into(),
                    allowed_read_paths: vec!["private execution path".into()],
                    limits: RunLimits::default(),
                },
                idempotency_key: format!("private idempotency key {run_id}"),
                created_at_ms,
            })
            .expect("create local Run fixture");
        RunFixture {
            conversation_id: conversation_id.into(),
            run_id: run_id.into(),
            prompt_id: prompt_id.into(),
        }
    }

    pub(super) fn add_private_events(&self, run: &RunFixture) {
        let events = [
            json!({
                "type": "run_started",
                "prompt": "private prompt body"
            }),
            json!({
                "type": "assistant_delta",
                "delta": "private assistant delta"
            }),
            json!({
                "type": "tool_started",
                "call": {
                    "id": "private tool call id",
                    "name": "private tool name",
                    "arguments": {"value": "private tool argument"}
                }
            }),
            json!({
                "type": "runtime_error",
                "code": "private error code",
                "message": "private error detail"
            }),
            json!({
                "type": "run_finished",
                "outcome": {"status": "completed", "answer": "private final answer"}
            }),
        ];
        for (index, payload) in events.into_iter().enumerate() {
            let sequence = u64::try_from(index + 1).expect("small sequence");
            let event = json!({
                "v": 1,
                "session_id": run.conversation_id,
                "run_id": run.run_id,
                "seq": sequence,
                "emitted_at_ms": 400 + sequence,
                "type": payload["type"],
                "prompt": payload.get("prompt"),
                "delta": payload.get("delta"),
                "call": payload.get("call"),
                "code": payload.get("code"),
                "message": payload.get("message"),
                "outcome": payload.get("outcome"),
            });
            self.insert_raw_event(run, sequence, &event.to_string());
        }
    }

    pub(super) fn add_single_event(
        &self,
        run: &RunFixture,
        sequence: u64,
        event_type: &str,
        payload_field: &str,
        payload: &str,
    ) {
        let mut event = json!({
            "v": 1,
            "session_id": run.conversation_id,
            "run_id": run.run_id,
            "seq": sequence,
            "emitted_at_ms": 400 + sequence,
            "type": event_type,
        });
        event.as_object_mut().expect("event object").insert(
            payload_field.to_owned(),
            serde_json::Value::String(payload.to_owned()),
        );
        self.insert_raw_event(run, sequence, &event.to_string());
    }

    pub(super) fn event_bytes(&self, run: &RunFixture, sequence: u64) -> usize {
        let connection = Connection::open(self.root.path().join("hub.sqlite3"))
            .expect("open Hub for byte count");
        let bytes = connection
            .query_row(
                "SELECT length(CAST(event_json AS BLOB)) FROM run_events
                 WHERE run_id = ?1 AND seq = ?2",
                params![run.run_id, i64::try_from(sequence).expect("small sequence")],
                |row| row.get::<_, i64>(0),
            )
            .expect("read event UTF-8 bytes");
        usize::try_from(bytes).expect("event byte count fits usize")
    }

    pub(super) fn insert_raw_event(&self, run: &RunFixture, sequence: u64, event_json: &str) {
        let connection = Connection::open(self.root.path().join("hub.sqlite3"))
            .expect("open fixture Hub for raw event");
        connection
            .execute(
                "INSERT INTO run_events(run_id, seq, event_json) VALUES (?1, ?2, ?3)",
                params![
                    run.run_id,
                    i64::try_from(sequence).expect("small sequence"),
                    event_json
                ],
            )
            .expect("insert fixture event");
    }
}

#[allow(clippy::struct_field_names)]
pub(super) struct RunFixture {
    pub(super) conversation_id: String,
    pub(super) run_id: String,
    pub(super) prompt_id: String,
}

pub(super) fn owner(subject: &str) -> ConversationOwner {
    ConversationOwner {
        issuer: "https://identity.example".into(),
        subject: subject.into(),
        tenant_id: "tenant-1".into(),
    }
}
