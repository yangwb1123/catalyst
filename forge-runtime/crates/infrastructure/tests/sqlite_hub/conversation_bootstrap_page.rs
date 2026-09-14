use super::*;
use rusqlite::Connection;
use tempfile::TempDir;

fn fixture() -> (TempDir, SqliteHubStore) {
    let root = TempDir::new().expect("temporary Hub root");
    let store = SqliteHubStore::open(root.path().join("hub.sqlite3")).expect("open Hub");
    (root, store)
}

fn mark_as_v29_baselines(root: &TempDir, conversation_ids: &[String]) {
    let connection = Connection::open(root.path().join("hub.sqlite3")).expect("open Hub SQL");
    let transaction = connection
        .unchecked_transaction()
        .expect("begin legacy baseline fixture transaction");
    transaction
        .execute("DELETE FROM conversation_changes", [])
        .expect("remove post-v29 fixture changes");
    transaction
        .execute(
            "UPDATE conversation_change_state SET last_cursor = 0 WHERE state_id = 1",
            [],
        )
        .expect("reset journal state");
    transaction
        .execute("UPDATE conversation_change_heads SET last_version = 0", [])
        .expect("reset aggregate versions");
    for id in conversation_ids {
        transaction
            .execute(
                "INSERT INTO conversation_change_baselines(conversation_id) VALUES (?1)",
                [id],
            )
            .expect("record v29 Conversation baseline");
    }
    transaction.commit().expect("commit baseline fixture");
}

#[test]
fn legacy_baseline_pages_are_binary_id_ordered() {
    let (root, store) = fixture();
    let legacy_one = store
        .create_conversation(&ConversationScope::Global, "legacy one", "legacy-one")
        .expect("first legacy Conversation");
    let legacy_two = store
        .create_conversation(&ConversationScope::Global, "legacy two", "legacy-two")
        .expect("second legacy Conversation");
    mark_as_v29_baselines(&root, &[legacy_one.id.clone(), legacy_two.id.clone()]);
    let mut expected_baselines = [legacy_one.id.clone(), legacy_two.id.clone()];
    expected_baselines.sort();

    let first = store
        .conversation_bootstrap_page(None, 1)
        .expect("first baseline page");
    assert_eq!(first.snapshot_cursor, 0);
    assert_eq!(first.scanned_through_cursor, 0);
    assert_eq!(first.conversations.len(), 1);
    assert_eq!(
        first.conversations[0].conversation.id,
        expected_baselines[0]
    );
    assert_eq!(first.conversations[0].creation_cursor, 0);
    assert_eq!(first.conversations[0].aggregate_version, 0);
    assert!(first.has_more);
    let first_cursor = first.next_cursor.expect("baseline continuation");
    assert_eq!(
        first_cursor.phase,
        ConversationBootstrapPhase::LegacyBaseline
    );

    let second = store
        .conversation_bootstrap_page(Some(&first_cursor), 1)
        .expect("second baseline page");
    assert_eq!(second.conversations.len(), 1);
    assert_eq!(
        second.conversations[0].conversation.id,
        expected_baselines[1]
    );
    assert!(!second.has_more);
    assert!(second.next_cursor.is_none());
}

#[test]
fn frozen_head_reconciles_later_writes_and_reports_observed_versions() {
    let (root, store, at_head) = at_head_fixture();
    let frozen_head = store
        .snapshot_at_cursor()
        .expect("capture current cursor")
        .cursor;
    assert_eq!(frozen_head, 2);
    let journal_cursor = baseline_to_journal(&store, frozen_head);

    store
        .append_prompt(&at_head.id, "assistant", "after head", "after-head-prompt")
        .expect("append post-head Prompt");
    let after_head = store
        .create_conversation(&ConversationScope::Global, "late", "late-conversation")
        .expect("create post-head Conversation");

    let created_page = assert_created_page(&store, &journal_cursor, frozen_head, &at_head);
    assert_prompt_only_page(&store, created_page.next_cursor.as_ref(), frozen_head);
    assert_reconciled_changes(&store, frozen_head, &at_head, &after_head);
    assert_bootstrap_reads_are_read_only(&root, &store);
}

fn at_head_fixture() -> (TempDir, SqliteHubStore, Conversation) {
    let (root, store) = fixture();
    let legacy = store
        .create_conversation(&ConversationScope::Global, "legacy", "legacy")
        .expect("legacy Conversation");
    let legacy_two = store
        .create_conversation(&ConversationScope::Global, "legacy two", "legacy-two")
        .expect("second legacy Conversation");
    mark_as_v29_baselines(&root, &[legacy.id, legacy_two.id]);
    let at_head = store
        .create_conversation(&ConversationScope::Global, "at head", "at-head")
        .expect("create journal Conversation");
    store
        .append_prompt(&at_head.id, "user", "before head", "at-head-prompt")
        .expect("append journal Prompt");
    (root, store, at_head)
}

fn baseline_to_journal(store: &SqliteHubStore, frozen_head: u64) -> ConversationBootstrapCursor {
    let baseline_page = store
        .conversation_bootstrap_page(None, 1)
        .expect("baseline phase of frozen bootstrap");
    assert_eq!(baseline_page.snapshot_cursor, frozen_head);
    let baseline_cursor = baseline_page.next_cursor.expect("baseline continuation");
    let final_baseline = store
        .conversation_bootstrap_page(Some(&baseline_cursor), 1)
        .expect("finish baseline phase");
    assert_eq!(final_baseline.conversations.len(), 1);
    assert_eq!(final_baseline.conversations[0].creation_cursor, 0);
    let journal_cursor = final_baseline
        .next_cursor
        .expect("transition from baselines to journal");
    assert_eq!(journal_cursor.phase, ConversationBootstrapPhase::ChangeLog);
    assert_eq!(journal_cursor.after_change_cursor, Some(0));
    journal_cursor
}

fn assert_created_page(
    store: &SqliteHubStore,
    cursor: &ConversationBootstrapCursor,
    frozen_head: u64,
    at_head: &Conversation,
) -> ConversationBootstrapPage {
    let created_page = store
        .conversation_bootstrap_page(Some(cursor), 1)
        .expect("read frozen Conversation creation event");
    assert_eq!(created_page.snapshot_cursor, frozen_head);
    assert_eq!(created_page.scanned_through_cursor, 1);
    assert_eq!(created_page.conversations.len(), 1);
    assert_eq!(created_page.conversations[0].conversation.id, at_head.id);
    assert_eq!(created_page.conversations[0].creation_cursor, 1);
    assert_eq!(created_page.conversations[0].aggregate_version, 3);
    assert!(created_page.has_more);
    created_page
}

fn assert_prompt_only_page(
    store: &SqliteHubStore,
    cursor: Option<&ConversationBootstrapCursor>,
    frozen_head: u64,
) {
    let prompt_only_page = store
        .conversation_bootstrap_page(cursor, 1)
        .expect("advance across frozen Prompt event");
    assert!(prompt_only_page.conversations.is_empty());
    assert_eq!(prompt_only_page.scanned_through_cursor, frozen_head);
    assert!(!prompt_only_page.has_more);
}

fn assert_reconciled_changes(
    store: &SqliteHubStore,
    frozen_head: u64,
    at_head: &Conversation,
    after_head: &Conversation,
) {
    let reconciled: ConversationChangePage = store
        .conversation_changes_after(frozen_head, 2)
        .expect("read post-head changes");
    assert_eq!(reconciled.changes.len(), 2);
    assert_eq!(reconciled.changes[0].conversation_id, at_head.id);
    assert_eq!(reconciled.changes[1].conversation_id, after_head.id);
    assert_eq!(reconciled.changes[1].entity_id, after_head.id);
}

fn assert_bootstrap_reads_are_read_only(root: &TempDir, store: &SqliteHubStore) {
    let cursor_before = store
        .snapshot_at_cursor()
        .expect("read cursor before repeat reads")
        .cursor;
    let version_before = hub_schema_version(&root.path().join("hub.sqlite3"))
        .expect("schema version before repeat reads");
    let _ = store
        .conversation_bootstrap_page(None, 2)
        .expect("repeat read-only bootstrap");
    assert_eq!(
        store
            .snapshot_at_cursor()
            .expect("read cursor after repeat reads")
            .cursor,
        cursor_before
    );
    assert_eq!(
        hub_schema_version(&root.path().join("hub.sqlite3"))
            .expect("schema version after repeat reads"),
        version_before
    );
}

#[test]
fn bootstrap_rejects_future_frozen_heads_and_damaged_journal_state() {
    let (root, store) = fixture();
    let conversation = store
        .create_conversation(&ConversationScope::Global, "journal", "journal")
        .expect("create Conversation");
    let future = forge_runtime_domain::ConversationBootstrapCursor {
        snapshot_cursor: 2,
        phase: ConversationBootstrapPhase::ChangeLog,
        after_conversation_id: None,
        after_change_cursor: Some(0),
    };
    assert!(matches!(
        store.conversation_bootstrap_page(Some(&future), 1),
        Err(HubStoreError::Conflict { .. })
    ));

    let connection = Connection::open(root.path().join("hub.sqlite3")).expect("open Hub SQL");
    connection
        .execute(
            "DELETE FROM conversation_changes WHERE conversation_id = ?1",
            [&conversation.id],
        )
        .expect("damage change journal");
    assert!(matches!(
        store.conversation_bootstrap_page(None, 1),
        Err(HubStoreError::Corrupt { .. })
    ));
}
