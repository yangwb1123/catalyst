use forge_runtime_domain::{
    Conversation, ConversationOwner, ConversationScope, HubStore, HubStoreError,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::{TempDir, tempdir};

fn owner(tenant_id: &str) -> ConversationOwner {
    ConversationOwner {
        issuer: "https://id.example.test".into(),
        subject: "user-123".into(),
        tenant_id: tenant_id.into(),
    }
}

#[test]
fn owner_binding_lists_and_reads_owned_detail() {
    let fixture = owned_conversation_fixture();
    let page = fixture
        .store
        .list_owned_conversations(&fixture.account, None, 10)
        .expect("list account Conversations");
    assert_eq!(page.conversations.len(), 1);
    assert_eq!(page.conversations[0].conversation, fixture.conversation);
    assert_eq!(page.conversations[0].aggregate_version, 1);
    assert!(!page.has_more);
    let detail = fixture
        .store
        .get_owned_conversation(&fixture.account, &fixture.conversation.id)
        .expect("owner can read Conversation detail");
    assert_eq!(detail.conversation, fixture.conversation);
    assert_eq!(detail.aggregate_version, 1);
    assert_not_found(
        fixture
            .store
            .get_owned_conversation(&owner("tenant-b"), &fixture.conversation.id),
        "wrong tenant cannot read Conversation detail",
    );
}

#[test]
fn owner_binding_hides_legacy_and_foreign_history() {
    let fixture = owned_conversation_fixture();
    assert_not_found(
        fixture
            .store
            .get_owned_conversation(&fixture.account, &fixture.legacy.id),
        "legacy Conversation remains hidden from detail",
    );
    assert_eq!(
        fixture
            .store
            .list_owned_conversations(&owner("tenant-b"), None, 10)
            .expect("other tenant sees no sessions")
            .conversations
            .len(),
        0
    );

    assert_not_found(
        fixture.store.owned_conversation_prompt_page(
            &owner("tenant-b"),
            &fixture.conversation.id,
            None,
            10,
        ),
        "wrong tenant cannot read history",
    );
    assert_not_found(
        fixture.store.owned_conversation_prompt_page(
            &fixture.account,
            &fixture.legacy.id,
            None,
            10,
        ),
        "legacy Conversation remains unclaimed",
    );
}

#[test]
fn owned_prompt_writes_replay_idempotently_and_guard_cas() {
    let fixture = owned_conversation_fixture();
    let appended = fixture
        .store
        .append_owned_prompt(
            &fixture.account,
            &fixture.conversation.id,
            "first prompt",
            "prompt-key-1",
            1,
        )
        .expect("append Prompt at current version");
    assert_eq!(appended.prompt.role, "user");
    assert_eq!(appended.aggregate_version, 2);
    assert!(!appended.replayed);

    let replay = fixture
        .store
        .append_owned_prompt(
            &fixture.account,
            &fixture.conversation.id,
            "first prompt",
            "prompt-key-1",
            1,
        )
        .expect("retry accepted Prompt despite stale original CAS version");
    assert_eq!(replay.prompt.id, appended.prompt.id);
    assert_eq!(replay.aggregate_version, 2);
    assert!(replay.replayed);

    let second = fixture
        .store
        .append_owned_prompt(
            &fixture.account,
            &fixture.conversation.id,
            "second prompt",
            "prompt-key-2",
            2,
        )
        .expect("append a second Prompt at the next version");
    assert_eq!(second.aggregate_version, 3);

    let replay_after_later_write = fixture
        .store
        .append_owned_prompt(
            &fixture.account,
            &fixture.conversation.id,
            "first prompt",
            "prompt-key-1",
            1,
        )
        .expect("replay retains the original Prompt result after later writes");
    assert_eq!(replay_after_later_write.prompt.id, appended.prompt.id);
    assert_eq!(replay_after_later_write.aggregate_version, 2);
    assert!(replay_after_later_write.replayed);

    let stale_write = fixture
        .store
        .append_owned_prompt(
            &fixture.account,
            &fixture.conversation.id,
            "third prompt",
            "prompt-key-3",
            1,
        )
        .expect_err("stale new write must fail");
    assert!(matches!(stale_write, HubStoreError::Conflict { .. }));

    let history = fixture
        .store
        .owned_conversation_prompt_page(&fixture.account, &fixture.conversation.id, None, 10)
        .expect("read account history");
    assert_eq!(history.prompts.len(), 2);
    assert_eq!(history.prompts[0], second.prompt);
    assert_eq!(history.prompts[1], appended.prompt);
}

struct OwnedConversationFixture {
    _temporary_directory: TempDir,
    store: SqliteHubStore,
    legacy: Conversation,
    account: ConversationOwner,
    conversation: Conversation,
}

fn owned_conversation_fixture() -> OwnedConversationFixture {
    let temporary_directory = tempdir().expect("private temp directory");
    let database = temporary_directory.path().join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("open Hub");
    let legacy = store
        .create_conversation(&ConversationScope::Global, "Legacy", "legacy-create")
        .expect("create legacy ownerless Conversation");
    let account = owner("tenant-a");
    let conversation = store
        .create_owned_conversation(
            &account,
            &ConversationScope::Global,
            "Shared",
            "owned-create",
        )
        .expect("create owner-bound Conversation");
    OwnedConversationFixture {
        _temporary_directory: temporary_directory,
        store,
        legacy,
        account,
        conversation,
    }
}

fn assert_not_found<T: std::fmt::Debug>(result: Result<T, HubStoreError>, expectation: &str) {
    let error = result.expect_err(expectation);
    assert!(matches!(error, HubStoreError::NotFound { .. }));
}

#[test]
fn owner_create_replay_cannot_claim_or_replay_another_principals_session() {
    let state = tempdir().expect("private temp directory");
    let database = state.path().join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("open Hub");
    let conversation = store
        .create_owned_conversation(
            &owner("tenant-a"),
            &ConversationScope::Global,
            "Shared",
            "same-key",
        )
        .expect("create owner-bound Conversation");

    let replay = store
        .create_owned_conversation(
            &owner("tenant-a"),
            &ConversationScope::Global,
            "Shared",
            "same-key",
        )
        .expect("identical owner-bound replay");
    assert_eq!(replay, conversation);

    let denied = store
        .create_owned_conversation(
            &owner("tenant-b"),
            &ConversationScope::Global,
            "Shared",
            "same-key",
        )
        .expect_err("a different principal cannot inherit the idempotency key");
    assert!(matches!(denied, HubStoreError::NotFound { .. }));
}
