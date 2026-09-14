use std::path::PathBuf;

use crate::runtime_domain::HubStore;
use rusqlite::Connection;
use tempfile::TempDir;

use crate::SqliteHubStore;

use super::{
    DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL, V34_PENDING_RUN_INTENT_OBJECTS,
    migrate_with_before_final_fault_for_test, open_database, restrict_fixture_root,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version,
};

#[test]
fn v33_to_v34_adds_inert_pending_intent_tables() {
    let (_root, database, project_id) = v33_fixture();
    let migrated = open_database(&database).expect("migrate exact v33 Hub to v34");
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
    for object in V34_PENDING_RUN_INTENT_OBJECTS {
        assert!(schema_object_named(&migrated, object), "missing {object}");
    }
    for table in ["pending_run_intents", "pending_run_intent_events"] {
        assert_eq!(
            migrated
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })
                .expect("new inert table is empty"),
            0
        );
    }
}

#[test]
fn v33_to_v34_final_validation_failure_rolls_back_atomically() {
    let (_root, database, _) = v33_fixture();
    let connection = Connection::open(&database).expect("open exact v33 rollback fixture");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), 34);
        for object in V34_PENDING_RUN_INTENT_OBJECTS {
            assert!(schema_object_named(migrated, object), "missing {object}");
        }
        migrated.execute_batch("CREATE TABLE rogue_v34_final_fault(id TEXT)")
    })
    .expect_err("final v34 contract rejects a rogue object");
    assert!(matches!(
        error,
        forge_runtime_domain::HubStoreError::Corrupt { .. }
    ));
    assert_eq!(schema_version(&connection), 33);
    assert_eq!(schema_snapshot(&connection), before);
    for object in V34_PENDING_RUN_INTENT_OBJECTS
        .iter()
        .chain([&"rogue_v34_final_fault"])
    {
        assert!(
            !schema_object_named(&connection, object),
            "unexpected {object}"
        );
    }
}

fn v33_fixture() -> (TempDir, PathBuf, String) {
    let root = tempfile::tempdir().expect("v33 fixture root");
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

    let connection = Connection::open(&database).expect("open current Hub to shape v33 fixture");
    connection
        .pragma_update(None, "foreign_keys", true)
        .expect("enable fixture foreign keys");
    connection
        .execute_batch(DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL)
        .expect("remove v34-only schema additions");
    connection
        .pragma_update(None, "user_version", 33)
        .expect("mark exact v33 fixture");
    drop(connection);
    (root, database, project.id)
}
