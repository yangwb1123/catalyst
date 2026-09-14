use std::path::PathBuf;

use crate::runtime_domain::HubStore;
use rusqlite::Connection;
use tempfile::TempDir;

use crate::SqliteHubStore;

use super::{
    DROP_V33_PROJECT_CONSENT_OBJECTS_SQL, DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL,
    V33_PROJECT_CONSENT_OBJECTS, V34_PENDING_RUN_INTENT_OBJECTS,
    migrate_with_before_final_fault_for_test, open_database, restrict_fixture_root,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version,
};

#[test]
fn v32_migrates_through_consent_and_pending_intent_contracts() {
    let (_root, database, project_id) = v32_fixture();
    let migrated = open_database(&database).expect("migrate exact v32 Hub to current");
    assert_eq!(schema_version(&migrated), 34);
    assert_eq!(
        migrated
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE id = ?1",
                [&project_id],
                |row| { row.get::<_, i64>(0) }
            )
            .expect("preserved Project row"),
        1
    );
    for object in V33_PROJECT_CONSENT_OBJECTS
        .iter()
        .chain(V34_PENDING_RUN_INTENT_OBJECTS)
    {
        assert!(schema_object_named(&migrated, object), "missing {object}");
    }
}

#[test]
fn v32_to_current_final_validation_failure_rolls_back_atomically() {
    let (_root, database, _) = v32_fixture();
    let connection = Connection::open(&database).expect("open exact v32 rollback fixture");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), 34);
        for object in V33_PROJECT_CONSENT_OBJECTS
            .iter()
            .chain(V34_PENDING_RUN_INTENT_OBJECTS)
        {
            assert!(schema_object_named(migrated, object), "missing {object}");
        }
        migrated.execute_batch("CREATE TABLE rogue_v34_final_fault(id TEXT)")
    })
    .expect_err("final v34 contract rejects a rogue object");
    assert!(matches!(
        error,
        forge_runtime_domain::HubStoreError::Corrupt { .. }
    ));
    assert_eq!(schema_version(&connection), 32);
    assert_eq!(schema_snapshot(&connection), before);
    for object in V33_PROJECT_CONSENT_OBJECTS
        .iter()
        .chain(V34_PENDING_RUN_INTENT_OBJECTS)
        .chain([&"rogue_v34_final_fault"])
    {
        assert!(
            !schema_object_named(&connection, object),
            "unexpected {object}"
        );
    }
}

fn v32_fixture() -> (TempDir, PathBuf, String) {
    let root = tempfile::tempdir().expect("v32 fixture root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let project_directory = root.path().join("project");
    std::fs::create_dir(&project_directory).expect("create Project directory");
    let project_path = project_directory
        .canonicalize()
        .expect("canonicalize Project directory");
    let store = SqliteHubStore::open(&database).expect("create current Hub");
    let project = store
        .open_project(&project_path)
        .expect("register existing Project");
    drop(store);

    let connection = Connection::open(&database).expect("open current Hub to shape v32 fixture");
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
        .pragma_update(None, "user_version", 32)
        .expect("mark exact v32 fixture");
    drop(connection);
    (root, database, project.id)
}
