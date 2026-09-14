use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::SqliteHubStore;
use crate::runtime_domain::{ConversationScope, HubStore};

use super::{
    DROP_V31_CONVERSATION_OWNERS_SQL, DROP_V32_OWNER_CURSOR_OBJECTS_SQL,
    DROP_V33_PROJECT_CONSENT_OBJECTS_SQL, DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL,
    MIGRATE_V30_TO_V31_SQL, V31_OWNER_OBJECTS, V32_OWNER_CURSOR_OBJECTS,
    migrate_with_before_final_fault_for_test, open_database, restrict_fixture_root,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version,
};

#[test]
fn populated_v30_conversations_migrate_to_v31_without_owner_claims() {
    let (_root, database, conversation_id, prompt_id) = v30_fixture();
    let connection = Connection::open(&database).expect("open populated v30 Hub");
    connection
        .execute_batch(MIGRATE_V30_TO_V31_SQL)
        .expect("apply v31 owner migration");

    assert_eq!(schema_version(&connection), 31);
    assert_eq!(table_count(&connection, "conversation_owners"), 1);
    assert_eq!(owner_count(&connection), 0);
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM conversations WHERE id = ?1",
                [&conversation_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("preserved Conversation row"),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM prompts WHERE id = ?1 AND conversation_id = ?2",
                params![prompt_id, conversation_id],
                |row| row.get::<_, i64>(0),
            )
            .expect("preserved Prompt row"),
        1
    );
    assert!(schema_object_named(
        &connection,
        "conversation_owners_principal"
    ));
    drop(connection);

    let reopened = Connection::open(&database).expect("reopen v31 Hub");
    assert_eq!(schema_version(&reopened), 31);
    assert_eq!(owner_count(&reopened), 0);
    drop(reopened);
}

#[test]
fn v31_owner_principal_fields_enforce_exact_byte_bounds_and_foreign_key() {
    let (_root, database, conversation_id, _) = v30_fixture();
    let connection = Connection::open(&database).expect("open v30 Hub");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable v31 fixture foreign keys");
    connection
        .execute_batch(MIGRATE_V30_TO_V31_SQL)
        .expect("apply v31 owner migration");
    assert_invalid_principal_bounds(&connection, &conversation_id);
    assert!(
        insert_owner(
            &connection,
            "missing-conversation",
            "issuer",
            "subject",
            "tenant",
            0
        )
        .is_err(),
        "accepted an owner row for a missing Conversation"
    );
    assert!(
        insert_owner(
            &connection,
            &conversation_id,
            "issuer",
            "subject",
            "tenant",
            -1
        )
        .is_err(),
        "accepted a negative owner timestamp"
    );

    insert_owner(
        &connection,
        &conversation_id,
        &"i".repeat(2048),
        &"s".repeat(255),
        &"t".repeat(256),
        0,
    )
    .expect("accept exact maximum principal byte lengths");
}

fn assert_invalid_principal_bounds(connection: &Connection, conversation_id: &str) {
    let invalid_cases = [
        (String::new(), "subject".into(), "tenant".into()),
        ("i".repeat(2049), "subject".into(), "tenant".into()),
        ("issuer".into(), String::new(), "tenant".into()),
        ("issuer".into(), "s".repeat(256), "tenant".into()),
        ("issuer".into(), "subject".into(), String::new()),
        ("issuer".into(), "subject".into(), "t".repeat(257)),
    ];
    for (issuer, subject, tenant_id) in invalid_cases {
        assert!(
            insert_owner(
                connection,
                conversation_id,
                &issuer,
                &subject,
                &tenant_id,
                0
            )
            .is_err(),
            "accepted an invalid or oversized principal tuple"
        );
    }
}

fn insert_owner(
    connection: &Connection,
    conversation_id: &str,
    issuer: &str,
    subject: &str,
    tenant_id: &str,
    created_at_ms: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO conversation_owners(
           conversation_id,issuer,subject,tenant_id,created_at_ms
         ) VALUES(?1,?2,?3,?4,?5)",
        params![conversation_id, issuer, subject, tenant_id, created_at_ms],
    )
}

#[test]
fn final_validation_failure_rolls_v30_to_v32_back_atomically() {
    let (_root, database, _, _) = v30_fixture();
    let connection = Connection::open(&database).expect("open v30 rollback fixture");
    let before_schema: Vec<SchemaRow> = schema_snapshot(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::SCHEMA_VERSION);
        assert_eq!(owner_count(migrated), 0);
        migrated.execute_batch("CREATE TABLE rogue_v32_final_fault(id TEXT)")
    })
    .expect_err("final v32 validation rejects a rogue object");

    assert!(matches!(
        error,
        crate::runtime_domain::HubStoreError::Corrupt { .. }
    ));
    assert_eq!(schema_version(&connection), 30);
    assert_eq!(schema_snapshot(&connection), before_schema);
    for object in V31_OWNER_OBJECTS
        .iter()
        .chain(V32_OWNER_CURSOR_OBJECTS)
        .chain([&"rogue_v32_final_fault"])
    {
        assert!(!schema_object_named(&connection, object), "{object}");
    }
}

#[test]
fn malformed_v31_catalog_is_rejected_without_repair() {
    let (_root, database, _, _) = v30_fixture();
    let connection = Connection::open(&database).expect("open v30 malformed-schema fixture");
    connection
        .execute_batch(MIGRATE_V30_TO_V31_SQL)
        .expect("apply canonical v31 suffix");
    connection
        .execute_batch("DROP INDEX conversation_owners_principal")
        .expect("remove required principal lookup index");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&database).expect_err("malformed v31 must not be repaired");
    assert!(matches!(
        error,
        crate::runtime_domain::HubStoreError::Corrupt { .. }
    ));
    let unchanged = Connection::open(&database).expect("reopen rejected v31 fixture");
    assert_eq!(schema_version(&unchanged), 31);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert!(!schema_object_named(
        &unchanged,
        "conversation_owners_principal"
    ));
}

#[test]
fn existing_current_writable_open_never_creates_or_migrates_and_enables_foreign_keys() {
    let (_root, database, conversation_id, _) = v30_fixture();

    assert!(
        SqliteHubStore::open_existing_current_writable(&database).is_err(),
        "exact-current writable open must reject a v30 Hub"
    );
    let unchanged = Connection::open(&database).expect("inspect rejected v30 Hub");
    assert_eq!(schema_version(&unchanged), 30);
    assert_eq!(table_count(&unchanged, "conversation_owners"), 0);
    drop(unchanged);

    drop(open_database(&database).expect("upgrade v30 to current v34"));
    let store = SqliteHubStore::open_existing_current_writable(&database)
        .expect("open existing exact-current Hub writable");
    let connection = store.connect().expect("open writable Hub connection");
    let foreign_keys: i64 = connection
        .pragma_query_value(None, "foreign_keys", |row| row.get(0))
        .expect("inspect foreign-key enforcement");
    assert_eq!(foreign_keys, 1);
    drop(connection);
    store
        .append_prompt(
            &conversation_id,
            "user",
            "write through exact-current writable open",
            "prompt-current-writable-key",
        )
        .expect("writable open supports Hub writes");
}

fn v30_fixture() -> (TempDir, std::path::PathBuf, String, String) {
    let root = tempfile::tempdir().expect("create fixture root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("create current Hub");
    let conversation = store
        .create_conversation(
            &ConversationScope::Global,
            "legacy v30",
            "conversation-v30-key",
        )
        .expect("create fixture Conversation");
    let prompt = store
        .append_prompt(
            &conversation.id,
            "user",
            "preserve this legacy Prompt",
            "prompt-v30-key",
        )
        .expect("create fixture Prompt");
    drop(store);

    let downgrade = Connection::open(&database).expect("open current Hub to shape v30 fixture");
    downgrade
        .pragma_update(None, "foreign_keys", true)
        .expect("enable fixture foreign keys");
    downgrade
        .execute_batch(DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL)
        .expect("remove v34 pending-intent additions");
    downgrade
        .execute_batch(DROP_V33_PROJECT_CONSENT_OBJECTS_SQL)
        .expect("remove v33 Project consent additions");
    downgrade
        .execute_batch(DROP_V32_OWNER_CURSOR_OBJECTS_SQL)
        .expect("remove v32 owner-local cursor additions");
    downgrade
        .execute_batch(DROP_V31_CONVERSATION_OWNERS_SQL)
        .expect("remove only v31 schema additions");
    downgrade
        .pragma_update(None, "user_version", 30)
        .expect("mark exact v30 schema fixture");
    drop(downgrade);

    (root, database, conversation.id, prompt.id)
}

fn table_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .expect("query table presence")
}

fn owner_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM conversation_owners", [], |row| {
            row.get(0)
        })
        .expect("query owner count")
}
