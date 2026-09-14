use forge_runtime_domain::{
    ConversationOwner, ConversationScope, HubEntity, HubStore, HubStoreError,
    MAX_CONVERSATION_CHANGE_PAGE_LIMIT, OwnedConversationChangePage,
};
use forge_runtime_infrastructure::SqliteHubStore;
use tempfile::TempDir;

use super::fixture;

#[test]
fn owner_change_pages_continue_through_dense_owner_cursors() {
    let fixture = owner_change_fixture();
    let [first, prompt, last] = owner_change_pages(&fixture.store, &fixture.account);

    assert_eq!(first.after_cursor, 0);
    assert_eq!(first.changes.len(), 1);
    assert_eq!(first.changes[0].conversation_id, fixture.owned_ids[0]);
    assert_eq!(first.changes[0].cursor, 1);
    assert_eq!(first.scanned_through_cursor, first.changes[0].cursor);
    assert!(first.has_more);

    assert_eq!(prompt.after_cursor, first.scanned_through_cursor);
    assert_eq!(prompt.changes.len(), 1);
    assert_eq!(prompt.changes[0].entity_id, fixture.owned_prompt_id);
    assert_eq!(prompt.changes[0].cursor, 2);
    assert_eq!(prompt.scanned_through_cursor, prompt.changes[0].cursor);
    assert!(prompt.has_more);

    assert_eq!(last.after_cursor, prompt.scanned_through_cursor);
    assert_eq!(last.changes.len(), 1);
    assert_eq!(last.changes[0].conversation_id, fixture.owned_ids[1]);
    assert_eq!(last.changes[0].cursor, 3);
    assert_eq!(last.scanned_through_cursor, last.changes[0].cursor);
    assert!(!last.has_more);
}

#[test]
fn owner_change_pages_hide_foreign_ids_and_foreign_only_suffixes() {
    let fixture = owner_change_fixture();
    let pages = owner_change_pages(&fixture.store, &fixture.account);
    for change in pages.iter().flat_map(|page| &page.changes) {
        assert!(
            !fixture
                .foreign_ids
                .iter()
                .chain(std::iter::once(&fixture.legacy_id))
                .any(|foreign_id| foreign_id == &change.conversation_id)
        );
        assert!(
            !fixture
                .foreign_ids
                .iter()
                .chain(std::iter::once(&fixture.legacy_id))
                .any(|foreign_id| foreign_id == &change.entity_id)
        );
    }

    let last_cursor = 3;
    assert_eq!(pages[2].scanned_through_cursor, last_cursor);
    let empty_suffix = fixture
        .store
        .owned_conversation_changes_after(&fixture.account, last_cursor, 1)
        .expect("foreign-only journal suffix remains private");
    assert!(empty_suffix.changes.is_empty());
    assert_eq!(empty_suffix.scanned_through_cursor, last_cursor);
    assert!(!empty_suffix.has_more);
}

#[test]
fn owner_change_pages_reject_future_cursors_and_invalid_limits() {
    let fixture = owner_change_fixture();
    for after_cursor in [u64::MAX, 4] {
        assert!(matches!(
            fixture
                .store
                .owned_conversation_changes_after(&fixture.account, after_cursor, 1),
            Err(HubStoreError::Conflict {
                entity: HubEntity::Conversation,
                ..
            })
        ));
    }
    for limit in [0, MAX_CONVERSATION_CHANGE_PAGE_LIMIT + 1] {
        assert!(matches!(
            fixture
                .store
                .owned_conversation_changes_after(&fixture.account, 0, limit),
            Err(HubStoreError::Conflict {
                entity: HubEntity::Conversation,
                ..
            })
        ));
    }
}

#[test]
fn each_principal_starts_with_its_own_cursor_one() {
    let fixture = owner_change_fixture();
    let first = fixture
        .store
        .owned_conversation_changes_after(&fixture.other_subject, 0, 1)
        .expect("read another principal's first change");
    assert_eq!(first.changes[0].cursor, 1);
    assert_eq!(first.scanned_through_cursor, 1);
    assert_eq!(first.changes[0].conversation_id, fixture.other_subject_id);
}

#[test]
fn concurrent_owner_writes_allocate_one_dense_cursor_sequence() {
    use std::sync::{Arc, Barrier};

    const CONVERSATIONS: usize = 4;
    let (root, store) = fixture();
    let owner = owner("issuer", "concurrent-user", "tenant");
    let conversation_ids = (0..CONVERSATIONS)
        .map(|index| {
            create_owned_conversation(&store, &owner, "concurrent", &format!("key-{index}")).id
        })
        .collect::<Vec<_>>();
    let barrier = Arc::new(Barrier::new(CONVERSATIONS));
    let database = root.path().join("hub.sqlite3");
    let writers = conversation_ids
        .into_iter()
        .enumerate()
        .map(|(index, conversation_id)| {
            let barrier = Arc::clone(&barrier);
            let database = database.clone();
            let owner = owner.clone();
            std::thread::spawn(move || {
                let store = SqliteHubStore::open(database).expect("open concurrent writer Hub");
                barrier.wait();
                store
                    .append_owned_prompt(
                        &owner,
                        &conversation_id,
                        "concurrent prompt",
                        &format!("prompt-{index}"),
                        1,
                    )
                    .expect("append owner-bound prompt concurrently");
            })
        })
        .collect::<Vec<_>>();
    for writer in writers {
        writer.join().expect("join owner change writer");
    }
    let page = store
        .owned_conversation_changes_after(&owner, 0, 10)
        .expect("read concurrent owner feed");
    assert_eq!(
        page.changes
            .iter()
            .map(|change| change.cursor)
            .collect::<Vec<_>>(),
        (1..=(CONVERSATIONS * 2) as u64).collect::<Vec<_>>()
    );
    assert!(!page.has_more);
}

struct OwnerChangeFixture {
    _root: TempDir,
    store: SqliteHubStore,
    account: ConversationOwner,
    owned_ids: [String; 2],
    owned_prompt_id: String,
    foreign_ids: Vec<String>,
    legacy_id: String,
    other_subject: ConversationOwner,
    other_subject_id: String,
}

fn owner_change_fixture() -> OwnerChangeFixture {
    let (root, store) = fixture();
    let account = owner("https://id.example.test", "user-1", "tenant-a");
    let tenant_other = owner("https://id.example.test", "user-1", "tenant-b");
    let subject_other = owner("https://id.example.test", "user-2", "tenant-a");
    let issuer_other = owner("https://other-id.example.test", "user-1", "tenant-a");
    let first = create_owned_conversation(&store, &account, "first", "owner-first");
    let legacy = store
        .create_conversation(&ConversationScope::Global, "legacy", "legacy-session")
        .expect("create ownerless legacy Conversation");
    let tenant_foreign = create_owned_conversation(&store, &tenant_other, "tenant", "tenant-key");
    let subject_foreign =
        create_owned_conversation(&store, &subject_other, "subject", "subject-key");
    let issuer_foreign = create_owned_conversation(&store, &issuer_other, "issuer", "issuer-key");
    let prompt = store
        .append_owned_prompt(&account, &first.id, "private prompt", "owner-prompt", 1)
        .expect("append owner Prompt");
    let second = create_owned_conversation(&store, &account, "second", "owner-second");
    let trailing_foreign =
        create_owned_conversation(&store, &tenant_other, "trailing", "trailing-key");
    OwnerChangeFixture {
        _root: root,
        store,
        account,
        owned_ids: [first.id, second.id],
        owned_prompt_id: prompt.prompt.id,
        foreign_ids: [
            tenant_foreign.id,
            subject_foreign.id.clone(),
            issuer_foreign.id,
            trailing_foreign.id,
        ]
        .into(),
        legacy_id: legacy.id,
        other_subject: subject_other,
        other_subject_id: subject_foreign.id,
    }
}

fn owner_change_pages(
    store: &impl HubStore,
    account: &ConversationOwner,
) -> [OwnedConversationChangePage; 3] {
    let first = store
        .owned_conversation_changes_after(account, 0, 1)
        .expect("first dense owner page");
    let prompt = store
        .owned_conversation_changes_after(account, first.scanned_through_cursor, 1)
        .expect("second dense owner page");
    let last = store
        .owned_conversation_changes_after(account, prompt.scanned_through_cursor, 1)
        .expect("last dense owner page");
    [first, prompt, last]
}

fn owner(issuer: &str, subject: &str, tenant_id: &str) -> ConversationOwner {
    ConversationOwner {
        issuer: issuer.into(),
        subject: subject.into(),
        tenant_id: tenant_id.into(),
    }
}

fn create_owned_conversation(
    store: &impl HubStore,
    owner: &ConversationOwner,
    title: &str,
    idempotency_key: &str,
) -> forge_runtime_domain::Conversation {
    store
        .create_owned_conversation(owner, &ConversationScope::Global, title, idempotency_key)
        .expect("create owner-bound Conversation")
}
