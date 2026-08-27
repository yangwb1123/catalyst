use std::path::PathBuf;

use rusqlite::Connection;
use tempfile::TempDir;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V28_TO_V29_SQL, migrate_with_before_final_fault_for_test, open_database,
    restrict_fixture_root,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_named, schema_version,
};

const DROP_V29_CONTROLLER_SQL: &str = "DROP TABLE group_agent_scheduled_graph_controller_events;
     DROP TABLE group_agent_scheduled_graph_controllers;";

type RunRow = (String, String, String, i64);
type LineageRow = (String, String, Vec<u8>, Vec<u8>, i64);

#[derive(Debug, PartialEq)]
struct V28Rows {
    runs: Vec<RunRow>,
    lineages: Vec<LineageRow>,
}

#[test]
fn populated_v28_run_data_survives_v29_migration_and_reopen() {
    let (root, database) = exact_v28_fixture();
    let legacy = Connection::open(&database).expect("open exact v28 fixture");
    let before = v28_rows(&legacy);
    drop(legacy);

    let migrated = open_database(&database).expect("migrate populated v28 to v29");
    assert_eq!(schema_version(&migrated), super::SCHEMA_VERSION);
    assert_eq!(v28_rows(&migrated), before);
    assert_empty_controller_tables(&migrated);
    assert!(schema_object_named(
        &migrated,
        "group_agent_scheduled_graph_controllers_schedule"
    ));
    drop(migrated);

    let reopened = open_database(&database).expect("reopen migrated v29 Hub");
    assert_eq!(schema_version(&reopened), super::SCHEMA_VERSION);
    assert_eq!(v28_rows(&reopened), before);
    assert_empty_controller_tables(&reopened);
    drop((reopened, root));
}

#[test]
fn final_validation_failure_rolls_v28_to_v29_back_atomically() {
    let (root, database) = exact_v28_fixture();
    let connection = Connection::open(&database).expect("open v28 rollback fixture");
    let before_schema: Vec<SchemaRow> = schema_snapshot(&connection);
    let before_rows = v28_rows(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_eq!(schema_version(migrated), super::SCHEMA_VERSION);
        assert_empty_controller_tables(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v29_final_fault(id TEXT)")
    })
    .expect_err("final v29 validation rejects a rogue object");

    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    assert_eq!(schema_version(&connection), 28);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(v28_rows(&connection), before_rows);
    for object in v29_objects().into_iter().chain(["rogue_v29_final_fault"]) {
        assert!(!schema_object_named(&connection, object), "{object}");
    }
    drop((connection, root));
}

#[test]
fn malformed_v29_controller_schema_is_rejected_without_repair() {
    let (root, database) = exact_v28_fixture();
    let connection = Connection::open(&database).expect("open v28 malformed fixture");
    connection
        .execute_batch(MIGRATE_V28_TO_V29_SQL)
        .expect("create canonical v29 suffix");
    connection
        .execute_batch("DROP INDEX group_agent_scheduled_graph_controllers_schedule")
        .expect("remove required controller index");
    let before: Vec<SchemaRow> = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&database).expect_err("malformed v29 must not be repaired");
    assert!(matches!(error, HubStoreError::Corrupt { .. }), "{error:?}");
    let unchanged = Connection::open(&database).expect("reopen rejected v29 fixture");
    assert_eq!(schema_version(&unchanged), 29);
    assert_eq!(schema_snapshot(&unchanged), before);
    drop((unchanged, root));
}

fn exact_v28_fixture() -> (TempDir, PathBuf) {
    let root = TempDir::new().expect("v28 fixture root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let connection = open_database(&database).expect("create current fixture");
    seed_v28_rows(&connection);
    connection
        .execute_batch(DROP_V29_CONTROLLER_SQL)
        .expect("restore exact v28 schema");
    connection
        .execute_batch("PRAGMA user_version=28")
        .expect("stamp exact v28 fixture");
    assert_eq!(schema_version(&connection), 28);
    drop(connection);
    (root, database)
}

fn seed_v28_rows(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO projects VALUES('project-v29','Fixture','/fixture/v29',1);
             INSERT INTO conversations VALUES(
               'conversation-v29','global',NULL,'Fixture','conversation-v29-key',1,1
             );
             INSERT INTO prompts VALUES(
               'prompt-v29','conversation-v29','user','branch me','prompt-v29-key',2
             );
             INSERT INTO runs VALUES(
               'run-parent-v29','conversation-v29','prompt-v29','project-v29','{}','{}',2,
               'run-parent-v29-key',1,10
             );
             INSERT INTO runs VALUES(
               'run-child-v29','conversation-v29','prompt-v29','project-v29','{}','{}',2,
               'run-child-v29-key',1,20
             );
             INSERT INTO run_events VALUES('run-parent-v29',1,'{\"v\":1}');
             INSERT INTO run_lineages VALUES(
               'run-child-v29',1,'branch','root_input','run-parent-v29',1,
               zeroblob(32),x'0101010101010101010101010101010101010101010101010101010101010101',30
             );",
        )
        .expect("seed populated v28 Run data");
}

fn v28_rows(connection: &Connection) -> V28Rows {
    V28Rows {
        runs: run_rows(connection),
        lineages: lineage_rows(connection),
    }
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

fn lineage_rows(connection: &Connection) -> Vec<LineageRow> {
    let mut statement = connection
        .prepare(
            "SELECT child_run_id,parent_run_id,source_event_sha256,lineage_sha256,created_at_ms
             FROM run_lineages ORDER BY child_run_id",
        )
        .expect("prepare lineage snapshot");
    statement
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })
        .expect("query lineage snapshot")
        .collect::<Result<_, _>>()
        .expect("read lineage snapshot")
}

fn assert_empty_controller_tables(connection: &Connection) {
    for table in [
        "group_agent_scheduled_graph_controllers",
        "group_agent_scheduled_graph_controller_events",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("controller row count");
        assert_eq!(count, 0, "{table}");
    }
}

fn v29_objects() -> [&'static str; 3] {
    [
        "group_agent_scheduled_graph_controllers",
        "group_agent_scheduled_graph_controllers_schedule",
        "group_agent_scheduled_graph_controller_events",
    ]
}
