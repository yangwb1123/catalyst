use crate::runtime_domain::{ConversationOwner, ConversationScope, HubStore};
use rusqlite::{Connection, params};
use tempfile::TempDir;

use crate::SqliteHubStore;

use super::{
    DROP_V32_OWNER_CURSOR_OBJECTS_SQL, DROP_V33_PROJECT_CONSENT_OBJECTS_SQL,
    DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL, V32_OWNER_CURSOR_OBJECTS,
    migrate_with_before_final_fault_for_test, open_database, restrict_fixture_root,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version,
};

#[test]
fn v31_migration_backfills_dense_owner_cursors_without_legacy_or_foreign_rows() {
    let fixture = v31_fixture();
    let connection = open_database(&fixture.database).expect("migrate exact v31 Hub to v32");
    assert_eq!(schema_version(&connection), super::SCHEMA_VERSION);

    assert_eq!(
        owner_rows(&connection, &fixture.owner_a),
        [(1, 3), (2, 5), (3, 7)]
    );
    assert_eq!(owner_rows(&connection, &fixture.owner_b), [(1, 4), (2, 6)]);
    assert_eq!(
        owner_conversation_row_count(&connection, &fixture.legacy_id),
        0,
        "ownerless legacy events must not be backfilled"
    );
    assert_eq!(owner_head(&connection, &fixture.owner_a), 3);
    assert_eq!(owner_head(&connection, &fixture.owner_b), 2);
    drop(connection);

    let store = SqliteHubStore::open(&fixture.database).expect("reopen migrated Hub");
    let page = store
        .owned_conversation_changes_after(&fixture.owner_a, 0, 10)
        .expect("read owner-local feed");
    assert_eq!(
        page.changes
            .iter()
            .map(|change| change.cursor)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(page.scanned_through_cursor, 3);
    assert!(!page.has_more);
}

#[test]
fn v31_to_v32_final_validation_failure_rolls_back_atomically() {
    let fixture = v31_fixture();
    let connection = Connection::open(&fixture.database).expect("open exact v31 rollback fixture");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::SCHEMA_VERSION);
        migrated.execute_batch("CREATE TABLE rogue_v32_final_fault(id TEXT)")
    })
    .expect_err("final v32 contract rejects a rogue object");
    assert!(matches!(
        error,
        forge_runtime_domain::HubStoreError::Corrupt { .. }
    ));
    assert_eq!(schema_version(&connection), 31);
    assert_eq!(schema_snapshot(&connection), before);
    for object in V32_OWNER_CURSOR_OBJECTS
        .iter()
        .chain([&"rogue_v32_final_fault"])
    {
        assert!(!schema_object_named(&connection, object), "{object}");
    }
}

#[test]
fn malformed_v32_catalog_is_rejected_without_repair() {
    let fixture = v31_fixture();
    let connection = open_database(&fixture.database).expect("migrate v31 Hub");
    connection
        .execute_batch("DROP INDEX conversation_owners_principal_conversation")
        .expect("remove required v32 owner lookup index");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    assert!(open_database(&fixture.database).is_err());
    let unchanged = Connection::open(&fixture.database).expect("inspect rejected v32 Hub");
    assert_eq!(schema_version(&unchanged), super::SCHEMA_VERSION);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert!(!schema_object_named(
        &unchanged,
        "conversation_owners_principal_conversation"
    ));
}

struct V31Fixture {
    _root: TempDir,
    database: std::path::PathBuf,
    owner_a: ConversationOwner,
    owner_b: ConversationOwner,
    legacy_id: String,
}

fn v31_fixture() -> V31Fixture {
    let root = tempfile::tempdir().expect("create v31 fixture root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let store = SqliteHubStore::open(&database).expect("create v32 seed Hub");
    let legacy = store
        .create_conversation(&ConversationScope::Global, "legacy", "legacy-key")
        .expect("create ownerless Conversation");
    store
        .append_prompt(&legacy.id, "user", "legacy prompt", "legacy-prompt-key")
        .expect("append ownerless legacy prompt");
    let owner_a = owner("issuer", "subject-a", "tenant");
    let owner_b = owner("issuer", "subject-b", "tenant");
    let first = create_owned(&store, &owner_a, "owner-a-first");
    let foreign = create_owned(&store, &owner_b, "owner-b-first");
    store
        .append_owned_prompt(&owner_a, &first, "A prompt", "owner-a-prompt", 1)
        .expect("append A prompt");
    store
        .append_owned_prompt(&owner_b, &foreign, "B prompt", "owner-b-prompt", 1)
        .expect("append B prompt");
    create_owned(&store, &owner_a, "owner-a-second");
    drop(store);

    let connection = Connection::open(&database).expect("open v32 fixture for downgrade");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable fixture foreign keys");
    connection
        .execute_batch(DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL)
        .expect("remove v34-only schema additions");
    connection
        .execute_batch(DROP_V33_PROJECT_CONSENT_OBJECTS_SQL)
        .expect("remove v33-only schema additions");
    connection
        .execute_batch(DROP_V32_OWNER_CURSOR_OBJECTS_SQL)
        .expect("remove v32-only schema additions");
    connection
        .pragma_update(None, "user_version", 31)
        .expect("mark exact v31 fixture");
    drop(connection);
    V31Fixture {
        _root: root,
        database,
        owner_a,
        owner_b,
        legacy_id: legacy.id,
    }
}

fn create_owned(store: &SqliteHubStore, owner: &ConversationOwner, key: &str) -> String {
    store
        .create_owned_conversation(owner, &ConversationScope::Global, key, key)
        .expect("create owner-bound Conversation")
        .id
}

fn owner(issuer: &str, subject: &str, tenant_id: &str) -> ConversationOwner {
    ConversationOwner {
        issuer: issuer.into(),
        subject: subject.into(),
        tenant_id: tenant_id.into(),
    }
}

fn owner_rows(connection: &Connection, owner: &ConversationOwner) -> Vec<(i64, i64)> {
    let mut statement = connection
        .prepare(
            "SELECT owner_cursor, hub_cursor FROM conversation_owner_change_rows
             WHERE issuer = ?1 AND subject = ?2 AND tenant_id = ?3
             ORDER BY owner_cursor",
        )
        .expect("prepare owner cursor query");
    statement
        .query_map(
            params![owner.issuer, owner.subject, owner.tenant_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("query owner cursor rows")
        .collect::<Result<_, _>>()
        .expect("read owner cursor rows")
}

fn owner_conversation_row_count(connection: &Connection, conversation_id: &str) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_owner_change_rows WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("count owner rows for Conversation")
}

fn owner_head(connection: &Connection, owner: &ConversationOwner) -> i64 {
    connection
        .query_row(
            "SELECT last_cursor FROM conversation_owner_change_heads
             WHERE issuer = ?1 AND subject = ?2 AND tenant_id = ?3",
            params![owner.issuer, owner.subject, owner.tenant_id],
            |row| row.get(0),
        )
        .expect("read owner-local sequence head")
}
