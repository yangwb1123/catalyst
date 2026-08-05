use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V12_TO_V13_SQL, MIGRATE_V13_TO_V14_SQL, migrate_with_before_final_fault_for_test,
    open_database,
    schema_full_validation_tests::{SchemaRow, schema_snapshot},
    schema_object_exists, schema_object_named,
    schema_v13_migration_tests::legacy_active_v12_database,
    schema_version, table_columns,
};

const CANDIDATE_TABLE: &str = "group_agent_graph_scheduled_node_contract_candidates";
const PROJECT_LANE_INDEX: &str = "group_agent_graph_scheduled_node_candidates_project_lane";
const CREATED_INDEX: &str = "group_agent_graph_scheduled_node_candidates_created";
const V14_OBJECTS: &[&str] = &[CANDIDATE_TABLE, PROJECT_LANE_INDEX, CREATED_INDEX];
const V15_OBJECTS: &[&str] = &[
    "group_agent_graph_scheduled_node_provider_requests",
    "group_agent_graph_scheduled_node_provider_requests_project_lane",
    "group_agent_graph_scheduled_node_provider_requests_created",
];
const V16_OBJECTS: &[&str] = &[
    "group_agent_graph_scheduled_node_dispatch_lifecycles",
    "group_agent_graph_scheduled_node_dispatch_lifecycles_project_lane_active",
    "group_agent_graph_scheduled_node_dispatch_lifecycles_created",
];

#[test]
fn active_v13_data_and_schema_survive_v14_migration_and_reopen() {
    let (root, database) = legacy_active_v13_database();
    let legacy = Connection::open(&database).expect("open active v13 fixture");
    let before_schema = schema_snapshot(&legacy);
    let before_run = active_run(&legacy);
    drop(legacy);

    let connection = open_database(&database).expect("active v13 Hub migrates to v15");
    assert_v14_shape(&connection);
    assert_eq!(without_v14(&schema_snapshot(&connection)), before_schema);
    assert_eq!(active_run(&connection), before_run);
    assert_foreign_keys_clean(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated v15 Hub reopens");
    assert_v14_shape(&reopened);
    assert_eq!(active_run(&reopened), before_run);
    drop((reopened, root));
}

#[test]
fn v13_future_candidate_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_active_v13_database();
    let blocker = Connection::open(&database).expect("open v13 blocker fixture");
    let before_run = active_run(&blocker);
    blocker
        .execute_batch(&format!("CREATE TABLE {CANDIDATE_TABLE}(blocker TEXT)"))
        .expect("install future v14 table blocker");
    drop(blocker);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen rejected v13 fixture");
    assert_eq!(schema_version(&unchanged), 13);
    assert_eq!(table_columns(&unchanged, CANDIDATE_TABLE), ["blocker"]);
    assert!(!schema_object_named(&unchanged, PROJECT_LANE_INDEX));
    assert!(!schema_object_named(&unchanged, CREATED_INDEX));
    assert_eq!(active_run(&unchanged), before_run);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v13_to_current_atomically() {
    let (root, database) = legacy_active_v13_database();
    let connection = Connection::open(&database).expect("open v13 rollback fixture");
    let before_schema = schema_snapshot(&connection);
    let before_run = active_run(&connection);

    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_v14_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v14_final_fault(id TEXT)")
    })
    .expect_err("final v15 validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    assert_eq!(schema_version(&connection), 13);
    assert_eq!(schema_snapshot(&connection), before_schema);
    assert_eq!(active_run(&connection), before_run);
    for object in V14_OBJECTS {
        assert!(!schema_object_named(&connection, object));
    }
    for object in V15_OBJECTS {
        assert!(!schema_object_named(&connection, object));
    }
    assert!(!schema_object_named(&connection, "rogue_v14_final_fault"));
    drop((connection, root));
}

#[test]
fn malformed_v14_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v14_cases() {
        let (root, database) = legacy_active_v13_database();
        let connection = Connection::open(&database).expect("open malformed v14 fixture");
        let before_run = active_run(&connection);
        connection.execute_batch(&sql).expect("forge malformed v14");
        drop(connection);

        assert_open_corrupt(&database);
        let unchanged = Connection::open(&database).expect("reopen malformed v14");
        assert_eq!(schema_version(&unchanged), 14);
        assert_eq!(active_run(&unchanged), before_run);
        drop((unchanged, root));
    }
}

pub(super) fn legacy_active_v13_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_active_v12_database();
    let connection = Connection::open(&database).expect("open v12 fixture");
    connection
        .execute_batch(MIGRATE_V12_TO_V13_SQL)
        .expect("migrate fixture to v13");
    drop(connection);
    (root, database)
}

fn malformed_v14_cases() -> Vec<String> {
    vec![
        malformed(
            "graph_run_id TEXT NOT NULL UNIQUE",
            "graph_run_id TEXT NOT NULL",
        ),
        malformed("contract_version = 2", "contract_version BETWEEN 1 AND 2"),
        malformed("execution_ordinal = 0", "execution_ordinal >= 0"),
        malformed("attempt = 1", "attempt BETWEEN 1 AND 2"),
        malformed(
            "predecessor_receipt_count = 0",
            "predecessor_receipt_count >= 0",
        ),
        malformed("progress_observed = 0", "progress_observed IN (0, 1)"),
        malformed(
            "ON group_agent_graph_scheduled_node_contract_candidates(created_at_ms DESC, id DESC)",
            "ON group_agent_graph_scheduled_node_contract_candidates(created_at_ms, id DESC)",
        ),
        format!("{MIGRATE_V13_TO_V14_SQL}\nCREATE TABLE rogue_v14_table(id TEXT);"),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V13_TO_V14_SQL.replacen(original, replacement, 1);
    assert_ne!(
        sql, MIGRATE_V13_TO_V14_SQL,
        "fixture replacement must match"
    );
    sql
}

fn assert_v14_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), 16);
    assert!(schema_object_exists(connection, "table", CANDIDATE_TABLE));
    assert!(schema_object_exists(
        connection,
        "index",
        PROJECT_LANE_INDEX
    ));
    assert!(schema_object_exists(connection, "index", CREATED_INDEX));
    assert_eq!(row_count(connection, CANDIDATE_TABLE), 0);
}

fn without_v14(snapshot: &[SchemaRow]) -> Vec<SchemaRow> {
    snapshot
        .iter()
        .filter(|(_, name, _, _)| {
            !V14_OBJECTS.contains(&name.as_str())
                && !V15_OBJECTS.contains(&name.as_str())
                && !V16_OBJECTS.contains(&name.as_str())
        })
        .cloned()
        .collect()
}

fn active_run(connection: &Connection) -> (i64, String, i64, i64) {
    connection
        .query_row(
            "SELECT run_version,status,dispatch_authority_released,last_event_seq
             FROM group_agent_graph_runs WHERE id='graph-run-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("active Graph Run survives")
}

fn assert_foreign_keys_clean(connection: &Connection) {
    assert_eq!(row_count(connection, "pragma_foreign_key_check"), 0);
}

fn row_count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .expect("row count")
}

fn assert_open_corrupt(database: &std::path::Path) {
    assert!(matches!(
        open_database(database),
        Err(HubStoreError::Corrupt { .. })
    ));
}
