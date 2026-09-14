use forge_runtime_domain::{ConversationScope, HubEntity, HubStore, HubStoreError};
use forge_runtime_infrastructure::SqliteHubStore;

use super::{fixture, snapshot_has_session};

#[test]
fn conversation_change_journal_is_ordered_idempotent_and_durable() {
    let (root, store) = fixture();
    let initial = store.snapshot_at_cursor().expect("initial snapshot cursor");
    assert_eq!(initial.cursor, 0);
    let (conversation_id, prompt_id) = create_conversation_and_prompt(&store);

    let first_page = store
        .conversation_changes_after(initial.cursor, 1)
        .expect("first change page");
    assert_eq!(first_page.head_cursor, 2);
    assert_eq!(first_page.next_cursor, 1);
    assert!(first_page.has_more);
    assert_eq!(first_page.changes[0].aggregate_version, 1);
    let second_page = store
        .conversation_changes_after(first_page.next_cursor, 1)
        .expect("second change page");
    assert_eq!(second_page.head_cursor, 2);
    assert_eq!(second_page.next_cursor, 2);
    assert!(!second_page.has_more);
    assert_eq!(second_page.changes[0].entity_id, prompt_id);
    assert_eq!(second_page.changes[0].aggregate_version, 2);
    assert_prompt_key_conflict(&store, &conversation_id);

    drop(store);
    let reopened = SqliteHubStore::open(root.path().join("hub.sqlite3"))
        .expect("reopen Hub with change journal");
    let resumed = reopened
        .conversation_changes_after(1, 10)
        .expect("resume after stored cursor");
    assert_eq!(resumed.changes, second_page.changes);
    let snapshot = reopened
        .snapshot_at_cursor()
        .expect("snapshot at durable head");
    assert_eq!(snapshot.cursor, 2);
    assert!(snapshot_has_session(&snapshot.snapshot, &conversation_id));
}

fn create_conversation_and_prompt(store: &SqliteHubStore) -> (String, String) {
    let conversation = store
        .create_conversation(&ConversationScope::Global, "Shared", "session-key")
        .expect("create Conversation");
    assert_eq!(
        store
            .create_conversation(&ConversationScope::Global, "Shared", "session-key")
            .expect("idempotent Conversation replay"),
        conversation
    );
    let prompt = store
        .append_prompt(&conversation.id, "user", "hello", "prompt-key")
        .expect("append Prompt");
    assert_eq!(
        store
            .append_prompt(&conversation.id, "user", "hello", "prompt-key")
            .expect("idempotent Prompt replay"),
        prompt
    );
    (conversation.id, prompt.id)
}

fn assert_prompt_key_conflict(store: &SqliteHubStore, conversation_id: &str) {
    assert!(matches!(
        store.append_prompt(conversation_id, "user", "changed", "prompt-key"),
        Err(HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            ..
        })
    ));
}

#[test]
fn conversation_change_cursor_gap_fails_closed() {
    let (root, store) = fixture();
    let conversation = store
        .create_conversation(&ConversationScope::Global, "Shared", "session-key")
        .expect("create Conversation");
    store
        .append_prompt(&conversation.id, "user", "hello", "prompt-key")
        .expect("append Prompt");
    drop(store);

    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .execute("DELETE FROM conversation_changes WHERE cursor = 1", [])
        .expect("remove first cursor from corruption fixture");
    drop(connection);
    let reopened = SqliteHubStore::open(database).expect("open schema-valid damaged journal");
    assert!(matches!(
        reopened.conversation_changes_after(0, 10),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn interior_change_deletion_prevents_prompt_commit() {
    let (root, store) = fixture();
    let conversation = store
        .create_conversation(&ConversationScope::Global, "Shared", "session-key")
        .expect("create Conversation");
    for (content, key) in [("one", "prompt-one"), ("two", "prompt-two")] {
        store
            .append_prompt(&conversation.id, "user", content, key)
            .expect("append Prompt");
    }
    drop(store);

    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .execute("DELETE FROM conversation_changes WHERE cursor = 2", [])
        .expect("remove an interior journal change");
    drop(connection);

    let reopened = SqliteHubStore::open(database).expect("open schema-valid damaged journal");
    assert!(matches!(
        reopened.append_prompt(&conversation.id, "user", "three", "prompt-three"),
        Err(HubStoreError::Corrupt { .. })
    ));
    let prompts = reopened
        .list_prompts(Some(&conversation.id), 10)
        .expect("read canonical Prompt rows after rejected append");
    assert_eq!(prompts.len(), 2);
    assert!(matches!(
        reopened.conversation_changes_after(0, 10),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn reordered_changes_prevent_prompt_commit() {
    let (root, store) = fixture();
    let conversation = store
        .create_conversation(&ConversationScope::Global, "Shared", "session-key")
        .expect("create Conversation");
    store
        .append_prompt(&conversation.id, "user", "first", "first-prompt")
        .expect("append first Prompt");
    drop(store);

    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .execute_batch(
            "BEGIN IMMEDIATE;
             UPDATE conversation_changes SET cursor = 3 WHERE cursor = 1;
             UPDATE conversation_changes SET cursor = 1 WHERE cursor = 2;
             UPDATE conversation_changes SET cursor = 2 WHERE cursor = 3;
             COMMIT;",
        )
        .expect("swap event cursors without changing durable heads");
    drop(connection);

    let reopened = SqliteHubStore::open(database).expect("open schema-valid damaged journal");
    assert!(matches!(
        reopened.append_prompt(&conversation.id, "user", "second", "second-prompt"),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(
        reopened
            .list_prompts(Some(&conversation.id), 10)
            .expect("list Prompts after rejected append")
            .len(),
        1
    );
    assert!(matches!(
        reopened.conversation_changes_after(0, 10),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn another_conversations_damaged_head_prevents_prompt_commit() {
    let (root, store) = fixture();
    let target = store
        .create_conversation(&ConversationScope::Global, "Target", "target-session")
        .expect("create target Conversation");
    let damaged = store
        .create_conversation(&ConversationScope::Global, "Damaged", "damaged-session")
        .expect("create second Conversation");
    drop(store);

    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .execute(
            "DELETE FROM conversation_change_heads WHERE conversation_id = ?1",
            [&damaged.id],
        )
        .expect("remove another Conversation's durable version head");
    drop(connection);

    let reopened = SqliteHubStore::open(database).expect("open schema-valid damaged journal");
    assert!(matches!(
        reopened.append_prompt(&target.id, "user", "must not commit", "target-prompt"),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(
        reopened
            .list_prompts(Some(&target.id), 10)
            .expect("read target Prompts after rejected append")
            .is_empty()
    );
    assert!(matches!(
        reopened.snapshot_at_cursor(),
        Err(HubStoreError::Corrupt { .. })
    ));
}

#[test]
fn missing_creation_change_prevents_prompt_commit() {
    let (root, store) = fixture();
    let conversation = store
        .create_conversation(&ConversationScope::Global, "Shared", "session-key")
        .expect("create Conversation");
    drop(store);

    let database = root.path().join("hub.sqlite3");
    let connection = rusqlite::Connection::open(&database).expect("open raw Hub");
    connection
        .execute(
            "DELETE FROM conversation_changes WHERE conversation_id = ?1",
            [&conversation.id],
        )
        .expect("remove creation event from corruption fixture");
    drop(connection);

    let reopened = SqliteHubStore::open(database).expect("open schema-valid damaged journal");
    assert!(matches!(
        reopened.append_prompt(&conversation.id, "user", "must not commit", "prompt-key"),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(
        reopened
            .list_prompts(Some(&conversation.id), 10)
            .expect("read Prompt rows after rejected append")
            .is_empty()
    );
    assert!(matches!(
        reopened.snapshot_at_cursor(),
        Err(HubStoreError::Corrupt { .. })
    ));
}
