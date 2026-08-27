use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V8_TO_V9_SQL, MIGRATE_V9_TO_V10_SQL, migrate_with_before_final_fault_for_test,
    open_database, schema_full_validation_tests::schema_snapshot, schema_object_exists,
    schema_object_named, schema_v9_migration_tests::legacy_v8_database, schema_version,
    table_columns,
};

#[test]
fn v9_graph_run_data_is_preserved_by_v10_migration_and_reopen() {
    let (root, database) = legacy_v9_database();
    let connection = open_database(&database).expect("v9 Hub migrates to v10");
    assert_v10_shape(&connection);
    assert_legacy_graph_run(&connection);
    assert_foreign_keys_clean(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated v10 Hub reopens");
    assert_v10_shape(&reopened);
    assert_legacy_graph_run(&reopened);
    drop((reopened, root));
}

#[test]
fn v9_future_contract_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v9_database();
    let blocker = Connection::open(&database).expect("open v9 blocker fixture");
    blocker
        .execute_batch("CREATE TABLE group_agent_graph_node_execution_contracts(blocker TEXT)")
        .expect("install future v10 table blocker");
    drop(blocker);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen rejected v9 fixture");
    assert_eq!(schema_version(&unchanged), 9);
    assert_eq!(
        table_columns(&unchanged, "group_agent_graph_node_execution_contracts"),
        ["blocker"]
    );
    assert_legacy_graph_run(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v9_to_v10_atomically() {
    let (root, database) = legacy_v9_database();
    let connection = Connection::open(&database).expect("open v9 rollback fixture");
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_v10_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v10_final_fault(id TEXT)")
    })
    .expect_err("final v10 validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    assert_eq!(schema_version(&connection), 9);
    assert!(!schema_object_named(
        &connection,
        "group_agent_graph_node_execution_contracts"
    ));
    assert!(!schema_object_named(&connection, "rogue_v10_final_fault"));
    assert_legacy_graph_run(&connection);
    drop((connection, root));
}

#[test]
fn malformed_v10_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v10_cases() {
        let (root, database) = legacy_v9_database();
        let connection = Connection::open(&database).expect("open v9 malformed fixture");
        connection.execute_batch(&sql).expect("forge malformed v10");
        drop(connection);
        assert_open_corrupt(&database);
        let unchanged = Connection::open(&database).expect("reopen malformed v10");
        assert_eq!(schema_version(&unchanged), 10);
        assert_legacy_graph_run(&unchanged);
        drop((unchanged, root));
    }
}

#[test]
fn future_schema_version_is_rejected_without_mutation() {
    let (root, database) = legacy_v9_database();
    let connection = open_database(&database).expect("migrate future-version fixture");
    connection
        .pragma_update(None, "user_version", 30)
        .expect("mark future schema");
    let before = schema_snapshot(&connection);
    drop(connection);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen future schema directly");
    assert_eq!(schema_version(&unchanged), 30);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert_legacy_graph_run(&unchanged);
    drop((unchanged, root));
}

#[test]
fn v10_run_state_check_rejects_partial_transition_shapes() {
    let (root, database) = legacy_v9_database();
    let connection = open_database(&database).expect("migrate state-check fixture");
    for assignment in [
        "run_version=2",
        "status='awaiting_core_dispatch'",
        "execution_contract_present=1",
        "last_event_seq=2",
    ] {
        let sql =
            format!("UPDATE group_agent_graph_runs SET {assignment} WHERE id='graph-run-legacy'");
        assert!(connection.execute_batch(&sql).is_err(), "{assignment}");
    }
    assert_legacy_graph_run(&connection);
    drop((connection, root));
}

pub(super) fn legacy_v9_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v8_database();
    let connection = Connection::open(&database).expect("open v8 fixture");
    connection
        .execute_batch(MIGRATE_V8_TO_V9_SQL)
        .expect("migrate fixture to v9");
    seed_v9_graph_run(&connection);
    drop(connection);
    (root, database)
}

fn seed_v9_graph_run(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_agent_graph_runs(
               id,graph_id,run_version,status,source_snapshot_sha256,
               graph_manifest_sha256,scheduler_protocol_version,plan_blob,plan_bytes,
               plan_sha256,node_count,wave_count,execution_contract_present,
               dispatch_authority_released,last_event_seq,journal_bytes,
               idempotency_key,created_at_ms
             ) VALUES(
               'graph-run-legacy','graph-legacy',1,'awaiting_execution_contract',
               zeroblob(32),zeroblob(32),1,x'7b7d',2,zeroblob(32),1,1,0,0,1,2,
               'graph-run-legacy-key',10
             );
             INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,
               event_sha256,created_at_ms
             ) VALUES(
               'graph-run-legacy',1,1,'graph_run_prepared',x'7b7d',2,zeroblob(32),10
             );",
        )
        .expect("seed v9 Graph Run");
}

fn malformed_v10_cases() -> Vec<String> {
    vec![
        malformed("run_version IN (1, 2)", "run_version BETWEEN 1 AND 3"),
        malformed(
            "status IN ('awaiting_execution_contract', 'awaiting_core_dispatch')",
            "status IN ('awaiting_execution_contract', 'running')",
        ),
        malformed(
            "graph_run_id TEXT NOT NULL UNIQUE",
            "graph_run_id TEXT NOT NULL",
        ),
        malformed(
            "expected_last_event_seq = 1",
            "expected_last_event_seq BETWEEN 1 AND 2",
        ),
        malformed(
            "CREATE INDEX group_agent_graph_node_contracts_created",
            "CREATE UNIQUE INDEX group_agent_graph_node_contracts_created",
        ),
        format!("{MIGRATE_V9_TO_V10_SQL}\nCREATE TABLE rogue_v10_table(id TEXT);"),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V9_TO_V10_SQL.replacen(original, replacement, 1);
    assert_ne!(sql, MIGRATE_V9_TO_V10_SQL, "fixture replacement must match");
    sql
}

fn assert_v10_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), super::SCHEMA_VERSION);
    for table in [
        "group_agent_graph_runs",
        "group_agent_graph_run_events",
        "group_agent_graph_node_execution_contracts",
        "group_agent_graph_node_dispatch_requests",
    ] {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing v10 table {table}"
        );
    }
}

fn assert_legacy_graph_run(connection: &Connection) {
    let row: (i64, String, i64, i64, i64) = connection
        .query_row(
            "SELECT run_version,status,execution_contract_present,last_event_seq,journal_bytes
             FROM group_agent_graph_runs WHERE id='graph-run-legacy'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("legacy Graph Run survives");
    assert_eq!(row, (1, "awaiting_execution_contract".into(), 0, 1, 2));
    let events: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM group_agent_graph_run_events
             WHERE graph_run_id='graph-run-legacy'",
            [],
            |row| row.get(0),
        )
        .expect("legacy event survives");
    assert_eq!(events, 1);
}

fn assert_foreign_keys_clean(connection: &Connection) {
    let violations: i64 = connection
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign key check");
    assert_eq!(violations, 0);
}

fn assert_open_corrupt(database: &std::path::Path) {
    assert!(matches!(
        open_database(database),
        Err(HubStoreError::Corrupt { .. })
    ));
}
