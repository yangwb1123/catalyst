use forge_runtime_domain::{
    ConversationPromptCursor, ConversationScope, HubEntity, HubStore, HubStoreError,
    MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::{Connection, params};

use super::fixture;

#[test]
fn prompt_pages_are_conversation_scoped_keyset_ordered_and_read_only() {
    let (root, store) = fixture();
    let first = conversation(&store, "first");
    let other = conversation(&store, "other");
    store
        .append_prompt(&first, "user", "older", "first-older")
        .expect("older Prompt");
    store
        .append_prompt(&first, "assistant", "newer", "first-newer")
        .expect("newer Prompt");
    store
        .append_prompt(&other, "user", "other conversation", "other-prompt")
        .expect("other Prompt");
    let database = root.path().join("hub.sqlite3");
    let connection = Connection::open(&database).expect("open test database");
    connection
        .execute(
            "UPDATE prompts SET created_at_ms = 100 WHERE conversation_id = ?1",
            params![first],
        )
        .expect("set deterministic timestamp tie");
    drop(connection);

    let cursor_before = store
        .snapshot_at_cursor()
        .expect("cursor before reads")
        .cursor;
    let expected = store
        .list_prompts(Some(&first), 10)
        .expect("ordered Prompts");
    assert_keyset_pages(&store, &first, &expected);
    assert_other_conversation_isolation(&store, &other, cursor_before);
}

fn assert_keyset_pages(
    store: &SqliteHubStore,
    conversation_id: &str,
    expected: &[forge_runtime_domain::PromptRecord],
) {
    let first_page = store
        .conversation_prompt_page(conversation_id, None, 1)
        .expect("first Prompt page");
    assert_eq!(first_page.conversation_id, conversation_id);
    assert_eq!(first_page.prompts.len(), 1);
    assert_eq!(first_page.prompts[0].id, expected[0].id);
    assert_eq!(first_page.prompts[0].content, expected[0].content);
    assert!(first_page.has_more);
    assert_eq!(
        first_page.next_cursor,
        Some(ConversationPromptCursor {
            created_at_ms: 100,
            prompt_id: expected[0].id.clone(),
        })
    );

    let second_page = store
        .conversation_prompt_page(conversation_id, first_page.next_cursor.as_ref(), 1)
        .expect("second Prompt page");
    assert_eq!(second_page.prompts.len(), 1);
    assert_eq!(second_page.prompts[0].id, expected[1].id);
    assert!(!second_page.has_more);
    assert_eq!(second_page.next_cursor, None);
    assert_ne!(first_page.prompts[0].id, second_page.prompts[0].id);
}

fn assert_other_conversation_isolation(
    store: &SqliteHubStore,
    other_conversation_id: &str,
    cursor_before: u64,
) {
    let other_page = store
        .conversation_prompt_page(other_conversation_id, None, 10)
        .expect("other Conversation page");
    assert_eq!(other_page.prompts.len(), 1);
    assert_eq!(other_page.prompts[0].conversation_id, other_conversation_id);
    assert_eq!(
        store
            .snapshot_at_cursor()
            .expect("cursor after reads")
            .cursor,
        cursor_before
    );
    assert!(matches!(
        store.conversation_prompt_page("missing-conversation", None, 1),
        Err(HubStoreError::NotFound {
            entity: HubEntity::Conversation,
            ..
        })
    ));
}

#[test]
fn prompt_page_stops_before_aggregate_content_budget_and_resumes_without_gaps() {
    let (_root, store) = fixture();
    let conversation_id = conversation(&store, "budget");
    let content = "x".repeat(MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES * 3 / 5);
    store
        .append_prompt(&conversation_id, "user", &content, "budget-older")
        .expect("older Prompt");
    store
        .append_prompt(&conversation_id, "user", &content, "budget-newer")
        .expect("newer Prompt");

    let first = store
        .conversation_prompt_page(&conversation_id, None, 128)
        .expect("budgeted first page");
    assert_eq!(first.prompts.len(), 1);
    assert_eq!(first.prompts[0].content.len(), content.len());
    assert!(first.has_more);
    let second = store
        .conversation_prompt_page(&conversation_id, first.next_cursor.as_ref(), 128)
        .expect("budgeted continuation");
    assert_eq!(second.prompts.len(), 1);
    assert_eq!(second.prompts[0].content, content);
    assert!(!second.has_more);
    assert_ne!(first.prompts[0].id, second.prompts[0].id);
}

#[test]
fn prompt_page_rejects_database_rows_over_the_frozen_content_bound() {
    let (root, store) = fixture();
    let conversation_id = conversation(&store, "corrupt");
    store
        .append_prompt(&conversation_id, "user", "small", "corrupt-row")
        .expect("valid Prompt");
    let connection = Connection::open(root.path().join("hub.sqlite3")).expect("open test database");
    connection
        .execute(
            "UPDATE prompts SET content = ?1 WHERE conversation_id = ?2",
            params![
                "x".repeat(MAX_CONVERSATION_PROMPT_PAGE_CONTENT_BYTES + 1),
                conversation_id
            ],
        )
        .expect("inject oversized row");

    assert!(matches!(
        store.conversation_prompt_page(&conversation_id, None, 1),
        Err(HubStoreError::Corrupt { .. })
    ));
}

fn conversation(store: &SqliteHubStore, key: &str) -> String {
    store
        .create_conversation(&ConversationScope::Global, key, &format!("create-{key}"))
        .expect("create Conversation")
        .id
}
