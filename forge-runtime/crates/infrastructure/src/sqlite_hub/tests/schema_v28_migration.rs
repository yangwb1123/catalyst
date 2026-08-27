use std::path::PathBuf;

use rusqlite::Connection;
use tempfile::TempDir;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V27_TO_V28_SQL, migrate_with_before_final_fault_for_test, open_database,
    restrict_fixture_root,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version,
};

type RunRow = (String, String, String, i64);

#[test]
fn populated_v27_run_journal_survives_v28_migration_and_reopen() {
    let (root, database) = exact_v27_fixture();
    let legacy = Connection::open(&database).expect("open exact v27 fixture");
    let before = run_rows(&legacy);
    drop(legacy);

    let migrated = open_database(&database).expect("migrate populated v27 to v28");
    assert_eq!(schema_version(&migrated), super::SCHEMA_VERSION);
    assert_eq!(run_rows(&migrated), before);
    assert_eq!(row_count(&migrated, "run_lineages"), 0);
    assert!(schema_object_named(&migrated, "run_lineages_parent"));
    drop(migrated);

    let reopened = open_database(&database).expect("reopen migrated v28 Hub");
    assert_eq!(schema_version(&reopened), super::SCHEMA_VERSION);
    assert_eq!(run_rows(&reopened), before);
    drop((reopened, root));
}

#[test]
fn final_validation_failure_rolls_v27_to_v28_back_atomically() {
    let (root, database) = exact_v27_fixture();
    let connection = Connection::open(&database).expect("open v27 rollback fixture");
    let before_schema: Vec<SchemaRow> = schema_snapshot(&connection);
    let before_rows = run_rows(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::SCHEMA_VERSION);
        assert_eq!(row_count(migrated, "run_lineages"), 0);
        migrated.execute_batch("CREATE TABLE rogue_v28_final_fault(id TEXT)")
    })
    .expect_err("final v28 validation rejects a rogue object");

    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(schema_version(&connection), 27);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(run_rows(&connection), before_rows);
    for object in [
        "run_lineages",
        "run_lineages_parent",
        "rogue_v28_final_fault",
    ] {
        assert!(!schema_object_named(&connection, object), "{object}");
    }
    drop((connection, root));
}

#[test]
fn malformed_v28_lineage_schema_is_rejected_without_repair() {
    let (root, database) = exact_v27_fixture();
    let connection = Connection::open(&database).expect("open v27 malformed fixture");
    connection
        .execute_batch(MIGRATE_V27_TO_V28_SQL)
        .expect("create canonical v28 suffix");
    connection
        .execute_batch("DROP INDEX run_lineages_parent")
        .expect("remove required lineage index");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&database).expect_err("malformed v28 must not be repaired");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    let unchanged = Connection::open(&database).expect("reopen rejected v28 fixture");
    assert_eq!(schema_version(&unchanged), 28);
    assert_eq!(schema_snapshot(&unchanged), before);
    drop((unchanged, root));
}

#[test]
fn v28_lineage_constraints_bind_distinct_existing_runs() {
    let (root, database) = exact_v27_fixture();
    let connection = open_database(&database).expect("migrate lineage constraint fixture");
    connection
        .execute_batch(
            "INSERT INTO run_lineages VALUES(
               'run-child',1,'branch','root_input','run-parent',1,
               zeroblob(32),x'0101010101010101010101010101010101010101010101010101010101010101',30
             );",
        )
        .expect("insert canonical direct-parent lineage");
    for update in [
        "UPDATE run_lineages SET child_run_id=parent_run_id",
        "UPDATE run_lineages SET branch_mode='event_prefix'",
        "UPDATE run_lineages SET source_event_seq=2",
        "UPDATE run_lineages SET source_event_sha256=zeroblob(31)",
    ] {
        connection
            .execute_batch(update)
            .expect_err("v28 lineage constraint rejects drift");
    }
    connection
        .execute_batch("DELETE FROM runs WHERE id='run-parent'")
        .expect_err("lineage parent foreign key is restrictive");
    drop((connection, root));
}

fn exact_v27_fixture() -> (TempDir, PathBuf) {
    let root = TempDir::new().expect("v27 fixture root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let connection = open_database(&database).expect("create current fixture");
    seed_runs(&connection);
    connection
        .execute_batch(super::DROP_V29_CONTROLLER_SQL)
        .expect("restore pre-v29 schema");
    connection
        .execute_batch(super::DROP_V28_LINEAGE_SQL)
        .expect("restore exact v27 schema");
    connection
        .execute_batch("PRAGMA user_version=27")
        .expect("stamp exact v27 fixture");
    assert_eq!(schema_version(&connection), 27);
    drop(connection);
    (root, database)
}

fn seed_runs(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO projects VALUES('project-v28','Fixture','/fixture/v28',1);
             INSERT INTO conversations VALUES(
               'conversation-v28','global',NULL,'Fixture','conversation-v28-key',1,1
             );
             INSERT INTO prompts VALUES(
               'prompt-v28','conversation-v28','user','branch me','prompt-v28-key',2
             );
             INSERT INTO runs VALUES(
               'run-parent','conversation-v28','prompt-v28','project-v28','{}','{}',2,
               'run-parent-key',1,10
             );
             INSERT INTO runs VALUES(
               'run-child','conversation-v28','prompt-v28','project-v28','{}','{}',2,
               'run-child-key',1,20
             );
             INSERT INTO run_events VALUES('run-parent',1,'{\"v\":1}');",
        )
        .expect("seed populated v27 Run journal");
}

fn run_rows(connection: &Connection) -> Vec<RunRow> {
    let mut statement = connection
        .prepare(
            "SELECT r.id,r.execution_json,r.cursor_json,COUNT(e.seq)
             FROM runs r LEFT JOIN run_events e ON e.run_id=r.id
             GROUP BY r.id ORDER BY r.id",
        )
        .expect("prepare Run snapshot");
    statement
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("query Run snapshot")
        .collect::<Result<_, _>>()
        .expect("read Run snapshot")
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}
