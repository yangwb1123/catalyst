use rusqlite::Connection;

use crate::SqliteHubStore;
use crate::runtime_domain::{ConversationChangeKind, ConversationScope, HubStore, HubStoreError};

use super::super::{
    MIGRATE_V29_TO_V30_SQL, migrate_with_before_final_fault_for_test, open_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version,
};
use super::fixtures::{
    baseline_count, change_count, change_cursor_head, conversation_version_head, exact_v29_fixture,
    has_baseline, legacy_rows,
};

#[test]
fn populated_v29_rows_survive_current_migration_and_reopen_without_owner_or_change_backfill() {
    let (root, database) = exact_v29_fixture();
    let legacy = Connection::open(&database).expect("open exact v29 fixture");
    let before = legacy_rows(&legacy);
    drop(legacy);

    let migrated = open_database(&database).expect("migrate populated v29 to current");
    assert_eq!(schema_version(&migrated), super::super::SCHEMA_VERSION);
    assert_eq!(legacy_rows(&migrated), before);
    assert_eq!(change_count(&migrated), 0, "v29 rows are not backfilled");
    assert_eq!(
        owner_count(&migrated),
        0,
        "v29 Conversations stay ownerless"
    );
    assert_eq!(baseline_count(&migrated), 1);
    assert!(has_baseline(&migrated, "conversation-v30"));
    assert_eq!(change_cursor_head(&migrated), 0);
    assert_eq!(conversation_version_head(&migrated, "conversation-v30"), 0);
    assert!(schema_object_named(
        &migrated,
        "conversation_changes_conversation_version"
    ));
    assert!(schema_object_named(
        &migrated,
        "conversation_changes_event_entity"
    ));
    drop(migrated);

    let reopened = open_database(&database).expect("reopen migrated current Hub");
    assert_eq!(schema_version(&reopened), super::super::SCHEMA_VERSION);
    assert_eq!(legacy_rows(&reopened), before);
    assert_eq!(change_count(&reopened), 0);
    assert_eq!(owner_count(&reopened), 0);
    assert_eq!(baseline_count(&reopened), 1);
    assert_eq!(change_cursor_head(&reopened), 0);
    assert_eq!(conversation_version_head(&reopened, "conversation-v30"), 0);
    drop((reopened, root));
}

#[test]
fn existing_v29_conversation_accepts_and_journals_new_prompts() {
    let (_root, database) = exact_v29_fixture();
    let store = SqliteHubStore::open(&database).expect("migrate populated v29 Hub");

    let prompt = store
        .append_prompt(
            "conversation-v30",
            "user",
            "continue after the schema upgrade",
            "prompt-post-v30-key",
        )
        .expect("append Prompt to an existing Conversation");
    let snapshot = store
        .snapshot_at_cursor()
        .expect("read global snapshot and journal cursor");
    assert_eq!(snapshot.cursor, 1);
    assert!(
        snapshot
            .snapshot
            .conversations
            .iter()
            .any(|conversation| conversation.id == "conversation-v30")
    );

    let page = store
        .conversation_changes_after(0, 10)
        .expect("read first post-migration change");
    assert_eq!(page.head_cursor, 1);
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].entity_id, prompt.id);
    assert_eq!(page.changes[0].aggregate_version, 1);
    assert_eq!(page.changes[0].kind, ConversationChangeKind::PromptAppended);
    let heads = Connection::open(&database).expect("inspect durable change heads");
    assert_eq!(change_cursor_head(&heads), 1);
    assert_eq!(conversation_version_head(&heads, "conversation-v30"), 1);
}

#[test]
fn deleting_the_only_legacy_change_rejects_later_writes() {
    let (_root, database) = exact_v29_fixture();
    let store = SqliteHubStore::open(&database).expect("migrate populated v29 Hub");
    store
        .append_prompt(
            "conversation-v30",
            "user",
            "first recorded prompt",
            "prompt-post-v30-first",
        )
        .expect("append first post-migration Prompt");
    drop(store);

    let connection = Connection::open(&database).expect("open tail-deletion fixture");
    connection
        .execute("DELETE FROM conversation_changes WHERE cursor = 1", [])
        .expect("delete the only recorded legacy Conversation event");
    drop(connection);

    let reopened = SqliteHubStore::open(&database).expect("open damaged but schema-valid Hub");
    assert!(matches!(
        reopened.append_prompt(
            "conversation-v30",
            "user",
            "must not commit after tail deletion",
            "prompt-post-v30-second",
        ),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        reopened.snapshot_at_cursor(),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(
        reopened
            .list_prompts(Some("conversation-v30"), 10)
            .expect("read canonical prompts after rejected append")
            .len(),
        2
    );
}

#[test]
fn deleting_legacy_baseline_marker_makes_journal_corrupt() {
    let (_root, database) = exact_v29_fixture();
    let store = SqliteHubStore::open(&database).expect("migrate populated v29 Hub");
    drop(store);

    let connection = Connection::open(&database).expect("open baseline-deletion fixture");
    connection
        .execute(
            "DELETE FROM conversation_change_baselines WHERE conversation_id = ?1",
            ["conversation-v30"],
        )
        .expect("delete legacy Conversation baseline marker");
    drop(connection);

    let reopened = SqliteHubStore::open(&database).expect("open damaged but schema-valid Hub");
    assert!(matches!(
        reopened.snapshot_at_cursor(),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        reopened.append_prompt(
            "conversation-v30",
            "user",
            "must not commit without a legacy baseline",
            "prompt-without-baseline",
        ),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert_eq!(
        reopened
            .list_prompts(Some("conversation-v30"), 10)
            .expect("read legacy Prompts after rejected append")
            .len(),
        1
    );
}

#[test]
fn deleting_legacy_baseline_after_first_change_makes_journal_corrupt() {
    let (_root, database) = exact_v29_fixture();
    let store = SqliteHubStore::open(&database).expect("migrate populated v29 Hub");
    store
        .append_prompt(
            "conversation-v30",
            "user",
            "first recorded prompt",
            "prompt-before-baseline-deletion",
        )
        .expect("append first post-migration Prompt");
    let other = store
        .create_conversation(
            &ConversationScope::Global,
            "Other",
            "other-session-after-migration",
        )
        .expect("create another post-migration Conversation");
    drop(store);

    let connection = Connection::open(&database).expect("open baseline-deletion fixture");
    connection
        .execute(
            "DELETE FROM conversation_change_baselines WHERE conversation_id = ?1",
            ["conversation-v30"],
        )
        .expect("delete used legacy Conversation baseline marker");
    drop(connection);

    let reopened = SqliteHubStore::open(&database).expect("open damaged but schema-valid Hub");
    assert!(matches!(
        reopened.snapshot_at_cursor(),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(matches!(
        reopened.append_prompt(&other.id, "user", "must not commit", "other-prompt"),
        Err(HubStoreError::Corrupt { .. })
    ));
    assert!(
        reopened
            .list_prompts(Some(&other.id), 10)
            .expect("read other Prompts after rejected append")
            .is_empty()
    );
}

#[test]
fn final_validation_failure_rolls_v29_to_v32_back_atomically() {
    let (root, database) = exact_v29_fixture();
    let connection = Connection::open(&database).expect("open v29 rollback fixture");
    let before_schema: Vec<SchemaRow> = schema_snapshot(&connection);
    let before_rows = legacy_rows(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::super::SCHEMA_VERSION);
        assert_eq!(change_count(migrated), 0);
        assert_eq!(owner_count(migrated), 0);
        migrated.execute_batch("CREATE TABLE rogue_v32_final_fault(id TEXT)")
    })
    .expect_err("final v32 validation rejects a rogue object");

    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(schema_version(&connection), 29);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(legacy_rows(&connection), before_rows);
    assert!(!schema_object_named(&connection, "conversation_changes"));
    assert!(!schema_object_named(
        &connection,
        "conversation_change_baselines"
    ));
    assert!(!schema_object_named(
        &connection,
        "conversation_change_state"
    ));
    assert!(!schema_object_named(
        &connection,
        "conversation_change_heads"
    ));
    assert!(!schema_object_named(&connection, "conversation_owners"));
    assert!(!schema_object_named(
        &connection,
        "conversation_owners_principal"
    ));
    assert!(!schema_object_named(&connection, "rogue_v32_final_fault"));
    drop((connection, root));
}

#[test]
fn malformed_v30_catalog_is_rejected_without_repair() {
    let (root, database) = exact_v29_fixture();
    let connection = Connection::open(&database).expect("open v29 drift fixture");
    connection
        .execute_batch(MIGRATE_V29_TO_V30_SQL)
        .expect("create canonical v30 suffix");
    connection
        .execute_batch("DROP INDEX conversation_changes_conversation_version")
        .expect("remove required aggregate-version index");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&database).expect_err("malformed v30 must not be repaired");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    let unchanged = Connection::open(&database).expect("reopen rejected v30 fixture");
    assert_eq!(schema_version(&unchanged), 30);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert!(!schema_object_named(
        &unchanged,
        "conversation_changes_conversation_version"
    ));
    drop((unchanged, root));
}

fn owner_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM conversation_owners", [], |row| {
            row.get(0)
        })
        .expect("query owner count")
}
