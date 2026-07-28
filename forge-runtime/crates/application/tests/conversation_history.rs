use std::sync::Arc;

use forge_runtime_application::{
    AgentRuntime, ConversationHistoryBridge, HistoryError, HubService, ToolCatalog,
};
use forge_runtime_domain::{
    Cancellation, Capability, Conversation, ConversationScope, HubEntity, HubStoreError, Message,
    ModelEvent, ModelFinishReason, PromptRecord, RunLimits, RunOutcome, RunRequest,
};
use forge_runtime_infrastructure::{CapStdWorkspaceFactory, MemoryEventSink, ScriptedProvider};
use tempfile::TempDir;

mod hub_support;

use hub_support::MemoryHubStore;

#[test]
fn empty_history_is_valid_and_a_missing_conversation_fails() {
    let (store, service, session) = fixture("empty");
    let current = service
        .append_prompt(&session.id, "user", "current", "current")
        .expect("current prompt");
    let bridge = ConversationHistoryBridge::new(store);

    let history = bridge
        .load_before(&session.id, &current.id, 1024)
        .expect("empty history");

    assert!(history.messages().is_empty());
    assert_eq!(history.content_bytes(), 0);
    assert_eq!(history.omitted_messages_lower_bound(), 0);
    assert_eq!(
        bridge
            .load_before("missing", "prompt", 1024)
            .expect_err("missing fails"),
        HistoryError::Store(HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            id: "missing".into(),
        })
    );
    drop(service);
}

#[test]
fn load_before_preserves_append_order_and_excludes_future_prompts() {
    let (store, _, session) = fixture("ordering");
    seed(&store, &session, "z", "user", "first", 100);
    seed(&store, &session, "y", "assistant", "second", 100);
    seed(&store, &session, "x", "user", "current", 100);
    seed(&store, &session, "w", "assistant", "future", 100);
    let bridge = ConversationHistoryBridge::new(store);

    let history = bridge
        .load_before(&session.id, "x", 1024)
        .expect("history before current");

    assert_eq!(
        history.messages(),
        [
            Message::User {
                text: "first".into()
            },
            Message::Assistant {
                text: "second".into(),
                tool_calls: vec![]
            }
        ]
    );
}

#[test]
fn load_before_rejects_a_prompt_from_another_conversation() {
    let store = MemoryHubStore::shared();
    let service = HubService::new(store.clone());
    let first = session(&service, "first", "session-first");
    let second = session(&service, "second", "session-second");
    let foreign = service
        .append_prompt(&second.id, "user", "foreign", "foreign-key")
        .expect("foreign prompt");
    let bridge = ConversationHistoryBridge::new(store);

    let error = bridge
        .load_before(&first.id, &foreign.id, 1024)
        .expect_err("foreign prompt fails");

    assert_eq!(
        error,
        HistoryError::Store(HubStoreError::NotFound {
            entity: HubEntity::Prompt,
            id: foreign.id,
        })
    );
}

#[test]
fn load_before_requires_the_boundary_prompt_to_be_a_user_message() {
    let (store, service, session) = fixture("boundary-role");
    let assistant = service
        .append_prompt(&session.id, "assistant", "answer", "assistant-key")
        .expect("assistant record");
    let bridge = ConversationHistoryBridge::new(store);

    let error = bridge
        .load_before(&session.id, &assistant.id, 1024)
        .expect_err("assistant boundary fails");

    assert_eq!(
        error,
        HistoryError::Store(HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            message: "history boundary must be a user prompt".into(),
        })
    );
}

#[test]
fn unknown_and_tool_roles_fail_closed_before_runtime_construction() {
    for (index, role) in ["tool", "system", "User"].into_iter().enumerate() {
        let store = MemoryHubStore::shared();
        let service = HubService::new(store.clone());
        let session = session(&service, role, &format!("session-{index}"));
        service
            .append_prompt(&session.id, role, "unsafe", &format!("unsafe-{index}"))
            .expect("fixture accepts raw persisted role");
        let current = service
            .append_prompt(&session.id, "user", "current", &format!("current-{index}"))
            .expect("current user");
        let error = ConversationHistoryBridge::new(store)
            .load_before(&session.id, &current.id, 1024)
            .expect_err("unsupported role fails");
        assert_eq!(
            error,
            HistoryError::UnsupportedRole {
                prompt_id: "prompt-2".into(),
                role: role.into(),
            }
        );
    }
}

#[test]
fn utf8_messages_are_kept_whole_at_the_byte_budget() {
    let (store, _, session) = fixture("utf8");
    seed(&store, &session, "a", "assistant", "old", 1);
    seed(&store, &session, "b", "user", "你好", 2);
    seed(&store, &session, "c", "assistant", "🙂", 3);
    seed(&store, &session, "d", "user", "current", 4);
    let bridge = ConversationHistoryBridge::new(store);

    let history = bridge
        .load_before(&session.id, "d", 10)
        .expect("bounded UTF-8 history");

    assert_eq!(history.content_bytes(), 10);
    assert_eq!(history.omitted_messages_lower_bound(), 1);
    assert_eq!(
        history.messages(),
        [
            Message::User {
                text: "你好".into()
            },
            Message::Assistant {
                text: "🙂".into(),
                tool_calls: vec![]
            }
        ]
    );
}

#[test]
fn an_oversized_latest_user_is_omitted_without_partial_utf8() {
    let (store, _, session) = fixture("oversized");
    seed(&store, &session, "a", "assistant", "old", 1);
    seed(&store, &session, "b", "user", "你好", 2);
    seed(&store, &session, "c", "assistant", "new", 3);
    seed(&store, &session, "d", "user", "current", 4);
    let history = ConversationHistoryBridge::new(store)
        .load_before(&session.id, "d", 5)
        .expect("oversized history is omitted");

    assert!(history.messages().is_empty());
    assert_eq!(history.content_bytes(), 0);
    assert_eq!(history.omitted_messages_lower_bound(), 3);
    assert!(!history.source_truncated());
}

#[test]
fn zero_budget_omits_every_complete_history_message() {
    let (store, service, session) = fixture("zero-budget");
    service
        .append_prompt(&session.id, "user", "prior", "prior")
        .expect("prior prompt");
    let current = service
        .append_prompt(&session.id, "user", "current", "current")
        .expect("current prompt");

    let history = ConversationHistoryBridge::new(store)
        .load_before(&session.id, &current.id, 0)
        .expect("zero-byte history");

    assert!(history.messages().is_empty());
    assert_eq!(history.content_bytes(), 0);
    assert_eq!(history.omitted_messages_lower_bound(), 1);
}

#[test]
fn record_window_reports_when_older_source_history_was_not_loaded() {
    let (store, _, session) = fixture("record-limit");
    for index in 0..17 {
        seed(
            &store,
            &session,
            &format!("record-{index:04}"),
            if index % 2 == 0 { "user" } else { "assistant" },
            "x",
            1,
        );
    }
    seed(&store, &session, "current", "user", "current", 1);

    let history = ConversationHistoryBridge::new(store)
        .load_before(&session.id, "current", usize::MAX)
        .expect("record-bounded history");

    assert_eq!(history.messages().len(), 16);
    assert!(history.source_truncated());
    assert_eq!(history.omitted_messages_lower_bound(), 1);
    assert!(matches!(
        history.messages().first(),
        Some(Message::User { .. })
    ));
}

#[test]
fn byte_trimming_drops_an_assistant_whose_user_message_did_not_fit() {
    let (store, _, session) = fixture("orphan");
    seed(&store, &session, "a", "user", "long question", 1);
    seed(&store, &session, "b", "assistant", "old answer", 2);
    seed(&store, &session, "c", "user", "latest", 3);
    seed(&store, &session, "d", "user", "current", 4);

    let history = ConversationHistoryBridge::new(store)
        .load_before(&session.id, "d", 16)
        .expect("bounded history");

    assert_eq!(
        history.messages(),
        [Message::User {
            text: "latest".into()
        }]
    );
    assert_eq!(history.omitted_messages_lower_bound(), 2);
}

#[tokio::test]
async fn agent_runtime_receives_prior_history_and_appends_current_prompt_once() {
    let (store, service, session) = fixture("runtime");
    service
        .append_prompt(&session.id, "user", "prior question", "prior-user")
        .expect("prior user");
    service
        .append_prompt(&session.id, "assistant", "prior answer", "prior-assistant")
        .expect("prior assistant");
    let current = service
        .append_prompt(&session.id, "user", "current question", "current-user")
        .expect("current user");
    let history = ConversationHistoryBridge::new(store)
        .load_before(&session.id, &current.id, 1024)
        .expect("runtime history");
    let root = TempDir::new().expect("workspace");
    let runtime = runtime();
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run_with_history(
            request(&root, &session, &current.content),
            history,
            Cancellation::default(),
            &mut sink,
        )
        .await
        .expect("run succeeds");

    assert_eq!(
        result.outcome,
        RunOutcome::Completed {
            answer: "fresh answer".into()
        }
    );
    assert_eq!(
        user_texts(&result.messages),
        ["prior question", "current question"]
    );
}

fn fixture(label: &str) -> (Arc<MemoryHubStore>, HubService, Conversation) {
    let store = MemoryHubStore::shared();
    let service = HubService::new(store.clone());
    let session = session(&service, label, &format!("session-{label}"));
    (store, service, session)
}

fn session(service: &HubService, title: &str, key: &str) -> Conversation {
    service
        .create_session(&ConversationScope::Global, title, key)
        .expect("session")
}

fn seed(
    store: &MemoryHubStore,
    session: &Conversation,
    id: &str,
    role: &str,
    content: &str,
    created_at_ms: u64,
) {
    store.seed_prompt(PromptRecord {
        id: id.into(),
        conversation_id: session.id.clone(),
        role: role.into(),
        content: content.into(),
        idempotency_key: format!("key-{id}"),
        created_at_ms,
    });
}

fn runtime() -> AgentRuntime {
    let turn = vec![
        Ok(ModelEvent::TextDelta {
            delta: "fresh answer".into(),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ];
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(vec![turn])),
        ToolCatalog::default(),
        Arc::new(CapStdWorkspaceFactory),
    )
}

fn request(root: &TempDir, session: &Conversation, prompt: &str) -> RunRequest {
    RunRequest {
        session_id: session.id.clone(),
        run_id: "history-run".into(),
        prompt: prompt.into(),
        system_prompt: "Use the supplied history.".into(),
        workspace: root.path().to_path_buf(),
        allowed_capabilities: vec![Capability::WorkspaceRead],
        limits: RunLimits::default(),
    }
}

fn user_texts(messages: &[Message]) -> Vec<&str> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::User { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}
