use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V9_TO_V10_SQL, MIGRATE_V10_TO_V11_SQL, migrate_with_before_final_fault_for_test,
    open_database, schema_full_validation_tests::schema_snapshot, schema_object_exists,
    schema_object_named, schema_v10_migration_tests::legacy_v9_database, schema_version,
    table_columns,
};

#[test]
fn v10_contract_and_run_data_survive_current_migration_and_reopen() {
    let (root, database) = legacy_v10_database();
    let connection = open_database(&database).expect("v10 Hub migrates to current");
    assert_current_shape(&connection);
    assert_legacy_v2_contract(&connection);
    assert_foreign_keys_clean(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated current Hub reopens");
    assert_current_shape(&reopened);
    assert_legacy_v2_contract(&reopened);
    drop((reopened, root));
}

#[test]
fn v10_future_dispatch_request_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v10_database();
    let blocker = Connection::open(&database).expect("open v10 blocker fixture");
    blocker
        .execute_batch("CREATE TABLE group_agent_graph_node_dispatch_requests(blocker TEXT)")
        .expect("install future v11 table blocker");
    drop(blocker);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen rejected v10 fixture");
    assert_eq!(schema_version(&unchanged), 10);
    assert_eq!(
        table_columns(&unchanged, "group_agent_graph_node_dispatch_requests"),
        ["blocker"]
    );
    assert_legacy_v10_contract(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v10_to_v11_atomically() {
    let (root, database) = legacy_v10_database();
    let connection = Connection::open(&database).expect("open v10 rollback fixture");
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_current_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_v11_final_fault(id TEXT)")
    })
    .expect_err("final v11 validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    assert_eq!(schema_version(&connection), 10);
    assert!(!schema_object_named(
        &connection,
        "group_agent_graph_node_dispatch_requests"
    ));
    assert!(!schema_object_named(&connection, "rogue_v11_final_fault"));
    assert_legacy_v10_contract(&connection);
    drop((connection, root));
}

#[test]
fn malformed_v11_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v11_cases() {
        let (root, database) = legacy_v10_database();
        let connection = Connection::open(&database).expect("open v10 malformed fixture");
        connection.execute_batch(&sql).expect("forge malformed v11");
        drop(connection);
        assert_open_corrupt(&database);
        let unchanged = Connection::open(&database).expect("reopen malformed v11");
        assert_eq!(schema_version(&unchanged), 11);
        assert_legacy_v2_contract(&unchanged);
        drop((unchanged, root));
    }
}

#[test]
fn v11_run_state_check_rejects_partial_preparation_shapes() {
    let (root, database) = legacy_v10_database();
    let connection = open_database(&database).expect("migrate state-check fixture");
    for assignment in [
        "run_version=3",
        "status='awaiting_dispatch_authorization'",
        "dispatch_request_present=1",
        "last_event_seq=3",
    ] {
        let sql =
            format!("UPDATE group_agent_graph_runs SET {assignment} WHERE id='graph-run-legacy'");
        assert!(connection.execute_batch(&sql).is_err(), "{assignment}");
    }
    assert_legacy_v2_contract(&connection);
    drop((connection, root));
}

#[test]
fn future_schema_version_is_rejected_without_mutation() {
    let (root, database) = legacy_v10_database();
    let connection = open_database(&database).expect("migrate future-version fixture");
    connection
        .pragma_update(None, "user_version", 19)
        .expect("mark future schema");
    let before = schema_snapshot(&connection);
    drop(connection);

    assert_open_corrupt(&database);
    let unchanged = Connection::open(&database).expect("reopen future schema directly");
    assert_eq!(schema_version(&unchanged), 19);
    assert_eq!(schema_snapshot(&unchanged), before);
    drop((unchanged, root));
}

pub(super) fn legacy_v10_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v9_database();
    let connection = Connection::open(&database).expect("open v9 fixture");
    connection
        .execute_batch(MIGRATE_V9_TO_V10_SQL)
        .expect("migrate fixture to v10");
    seed_v10_contract(&connection);
    drop(connection);
    (root, database)
}

fn seed_v10_contract(connection: &Connection) {
    connection
        .execute_batch(
            "UPDATE group_agent_graph_runs
             SET run_version=2,status='awaiting_core_dispatch',
                 execution_contract_present=1,last_event_seq=2,journal_bytes=4
             WHERE id='graph-run-legacy';
             INSERT INTO group_agent_graph_run_events(
               graph_run_id,seq,event_version,kind,event_blob,event_bytes,
               event_sha256,created_at_ms
             ) VALUES(
               'graph-run-legacy',2,2,'node_execution_contract_admitted',
               x'7b7d',2,zeroblob(32),11
             );
             INSERT INTO group_agent_graph_node_execution_contracts(
               id,graph_run_id,contract_version,node_id,attempt,control_snapshot_sha256,
               contract_blob,contract_bytes,contract_sha256,request_sha256,
               project_lane_sha256,expected_last_event_seq,expected_last_event_sha256,
               idempotency_key,created_at_ms
             ) VALUES(
               'contract-legacy','graph-run-legacy',1,'node-legacy',1,zeroblob(32),
               x'7b7d',2,zeroblob(32),zeroblob(32),zeroblob(32),1,zeroblob(32),
               'contract-legacy-key',11
             );",
        )
        .expect("seed v10 contract");
}

fn malformed_v11_cases() -> Vec<String> {
    vec![
        malformed("run_version IN (1, 2, 3)", "run_version BETWEEN 1 AND 3"),
        malformed(
            "dispatch_request_present IN (0, 1)",
            "dispatch_request_present BETWEEN 0 AND 1",
        ),
        malformed(
            "graph_run_id TEXT NOT NULL UNIQUE",
            "graph_run_id TEXT NOT NULL",
        ),
        malformed(
            "provider_request_bytes BETWEEN 1 AND 16777216",
            "provider_request_bytes BETWEEN 0 AND 16777216",
        ),
        malformed(
            "codec_protocol_version = 1",
            "codec_protocol_version BETWEEN 1 AND 2",
        ),
        malformed(
            "CREATE INDEX group_agent_graph_node_dispatch_requests_created",
            "CREATE UNIQUE INDEX group_agent_graph_node_dispatch_requests_created",
        ),
        format!("{MIGRATE_V10_TO_V11_SQL}\nCREATE TABLE rogue_v11_table(id TEXT);"),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V10_TO_V11_SQL.replacen(original, replacement, 1);
    assert_ne!(
        sql, MIGRATE_V10_TO_V11_SQL,
        "fixture replacement must match"
    );
    sql
}

fn assert_current_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), 18);
    for table in [
        "group_agent_graph_runs",
        "group_agent_graph_run_events",
        "group_agent_graph_node_execution_contracts",
        "group_agent_graph_node_dispatch_requests",
    ] {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing v11 table {table}"
        );
    }
}

fn assert_legacy_v2_contract(connection: &Connection) {
    let row: (i64, String, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT run_version,status,execution_contract_present,
                    dispatch_request_present,last_event_seq,journal_bytes
             FROM group_agent_graph_runs WHERE id='graph-run-legacy'",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("legacy v2 Graph Run survives");
    assert_eq!(row, (2, "awaiting_core_dispatch".into(), 1, 0, 2, 4));
    let contracts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM group_agent_graph_node_execution_contracts
             WHERE graph_run_id='graph-run-legacy'",
            [],
            |row| row.get(0),
        )
        .expect("legacy contract survives");
    assert_eq!(contracts, 1);
}

fn assert_legacy_v10_contract(connection: &Connection) {
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
        .expect("legacy v10 Graph Run survives");
    assert_eq!(row, (2, "awaiting_core_dispatch".into(), 1, 2, 4));
    let contracts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM group_agent_graph_node_execution_contracts
             WHERE graph_run_id='graph-run-legacy'",
            [],
            |row| row.get(0),
        )
        .expect("legacy v10 contract survives");
    assert_eq!(contracts, 1);
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
