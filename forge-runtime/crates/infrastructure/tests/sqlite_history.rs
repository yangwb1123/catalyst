use std::path::{Path, PathBuf};

use forge_runtime_domain::{ConversationScope, HubEntity, HubStore, HubStoreError};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::{Connection, params};
use tempfile::TempDir;

fn fixture() -> (TempDir, PathBuf, SqliteHubStore, String) {
    let root = TempDir::new().expect("state root");
    let database = root.path().join("private-state").join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("open Hub");
    let conversation = store
        .create_conversation(&ConversationScope::Global, "History", "history-session")
        .expect("conversation");
    (root, database, store, conversation.id)
}

#[test]
fn boundary_uses_append_order_when_timestamps_and_ids_disagree() {
    let (_root, database, store, conversation_id) = fixture();
    insert_prompt(&database, "z-old", &conversation_id, "user", 100);
    insert_prompt(&database, "m-answer", &conversation_id, "assistant", 100);
    insert_prompt(&database, "a-current", &conversation_id, "user", 100);
    insert_prompt(&database, "0-future", &conversation_id, "assistant", 100);

    let records = store
        .list_prompts_before(&conversation_id, "a-current", 10)
        .expect("bounded history");
    let ids: Vec<_> = records.iter().map(|record| record.id.as_str()).collect();

    assert_eq!(ids, ["m-answer", "z-old"]);
}

#[test]
fn old_boundary_is_found_even_after_more_than_one_thousand_future_appends() {
    let (_root, database, store, conversation_id) = fixture();
    insert_prompt(&database, "prior", &conversation_id, "user", 1);
    insert_prompt(&database, "old-current", &conversation_id, "user", 2);
    let connection = Connection::open(&database).expect("raw database");
    for index in 0..1_001 {
        insert_prompt_on(
            &connection,
            &format!("future-{index:04}"),
            &conversation_id,
            "assistant",
            3,
        );
    }

    let records = store
        .list_prompts_before(&conversation_id, "old-current", 1_001)
        .expect("old boundary remains addressable");

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, "prior");
}

#[test]
fn boundary_must_belong_to_the_conversation_and_have_user_role() {
    let (_root, database, store, conversation_id) = fixture();
    let other = store
        .create_conversation(&ConversationScope::Global, "Other", "other-session")
        .expect("other conversation");
    insert_prompt(&database, "foreign", &other.id, "user", 1);
    insert_prompt(
        &database,
        "assistant-boundary",
        &conversation_id,
        "assistant",
        2,
    );

    let foreign = store
        .list_prompts_before(&conversation_id, "foreign", 10)
        .expect_err("foreign boundary denied");
    let assistant = store
        .list_prompts_before(&conversation_id, "assistant-boundary", 10)
        .expect_err("assistant boundary denied");

    assert!(matches!(
        foreign,
        HubStoreError::NotFound {
            entity: HubEntity::Prompt,
            ..
        }
    ));
    assert!(matches!(
        assistant,
        HubStoreError::Conflict {
            entity: HubEntity::Prompt,
            ..
        }
    ));
}

fn insert_prompt(database: &Path, id: &str, conversation_id: &str, role: &str, created_at_ms: i64) {
    let connection = Connection::open(database).expect("raw database");
    insert_prompt_on(&connection, id, conversation_id, role, created_at_ms);
}

fn insert_prompt_on(
    connection: &Connection,
    id: &str,
    conversation_id: &str,
    role: &str,
    created_at_ms: i64,
) {
    connection
        .execute(
            "INSERT INTO prompts
             (id, conversation_id, role, content, idempotency_key, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                conversation_id,
                role,
                format!("content-{id}"),
                format!("key-{id}"),
                created_at_ms
            ],
        )
        .expect("insert prompt fixture");
}
