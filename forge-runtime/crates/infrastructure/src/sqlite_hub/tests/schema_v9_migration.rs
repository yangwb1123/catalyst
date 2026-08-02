use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V7_TO_V8_SQL, MIGRATE_V8_TO_V9_SQL, migrate_with_before_final_fault_for_test,
    open_database, schema_full_validation_tests::schema_snapshot, schema_object_exists,
    schema_object_named, schema_v8_migration_tests::legacy_v7_database, schema_version,
    table_columns,
};

#[test]
fn v8_graph_data_is_preserved_through_v9_and_v10_migration() {
    let (root, database) = legacy_v8_database();
    let connection = open_database(&database).expect("v8 Hub migrates to current");
    assert_current_graph_run_shape(&connection);
    assert_legacy_graph(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated Hub reopens");
    assert_current_graph_run_shape(&reopened);
    assert_legacy_graph(&reopened);
    drop((reopened, root));
}

#[test]
fn v8_future_graph_run_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v8_database();
    let blocker = Connection::open(&database).expect("open v8 blocker fixture");
    blocker
        .execute_batch("CREATE TABLE group_agent_graph_runs(blocker TEXT)")
        .expect("install future v9 table blocker");
    drop(blocker);

    let error = open_database(&database).expect_err("v8 prefix rejects future v9 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen rejected v8 fixture");
    assert_eq!(schema_version(&unchanged), 8);
    assert_eq!(
        table_columns(&unchanged, "group_agent_graph_runs"),
        ["blocker"]
    );
    assert!(!schema_object_named(
        &unchanged,
        "group_agent_graph_runs_graph"
    ));
    assert_legacy_graph(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v8_to_current_atomically() {
    let (root, database) = legacy_v8_database();
    let connection = Connection::open(&database).expect("open v8 rollback fixture");
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_current_graph_run_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_current_final_fault(id TEXT)")
    })
    .expect_err("final current validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    assert_eq!(schema_version(&connection), 8);
    assert!(!schema_object_named(&connection, "group_agent_graph_runs"));
    assert!(!schema_object_named(
        &connection,
        "rogue_current_final_fault"
    ));
    assert_legacy_graph(&connection);
    drop(connection);

    let reopened = Connection::open(&database).expect("reopen rolled-back v8 fixture");
    assert_eq!(schema_version(&reopened), 8);
    assert_legacy_graph(&reopened);
    drop((reopened, root));
}

#[test]
fn malformed_v9_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v9_cases() {
        assert_malformed_v9_is_rejected(&sql);
    }
}

#[test]
fn future_schema_version_is_rejected_without_mutation() {
    let (root, database) = legacy_v8_database();
    let connection = open_database(&database).expect("migrate future-version fixture");
    connection
        .pragma_update(None, "user_version", 16)
        .expect("mark future schema");
    let before = schema_snapshot(&connection);
    drop(connection);

    let error = open_database(&database).expect_err("future v16 schema is unsupported");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen future schema directly");
    assert_eq!(schema_version(&unchanged), 16);
    assert_eq!(schema_snapshot(&unchanged), before);
    assert_legacy_graph(&unchanged);
    drop((unchanged, root));
}

fn malformed_v9_cases() -> Vec<String> {
    let mut cases = malformed_v9_run_cases();
    cases.extend(malformed_v9_event_cases());
    cases
}

fn malformed_v9_run_cases() -> Vec<String> {
    vec![
        malformed("plan_blob BLOB NOT NULL", "plan_blob TEXT NOT NULL"),
        malformed("plan_bytes INTEGER NOT NULL", "plan_bytes INTEGER"),
        malformed("id TEXT NOT NULL PRIMARY KEY", "id TEXT NOT NULL"),
        malformed(
            "idempotency_key TEXT NOT NULL UNIQUE",
            "idempotency_key TEXT NOT NULL",
        ),
        malformed("run_version = 1", "run_version IN (1,2)"),
        malformed(
            "status = 'awaiting_execution_contract'",
            "status IN ('awaiting_execution_contract','running')",
        ),
        malformed(
            "scheduler_protocol_version = 1",
            "scheduler_protocol_version IN (1,2)",
        ),
        malformed(
            "execution_contract_present = 0",
            "execution_contract_present IN (0,1)",
        ),
        malformed(
            "dispatch_authority_released = 0",
            "dispatch_authority_released IN (0,1)",
        ),
        malformed("last_event_seq = 1", "last_event_seq BETWEEN 1 AND 2"),
        malformed(
            "REFERENCES group_agent_graphs(id)",
            "REFERENCES group_runs(id)",
        ),
        malformed("ON DELETE RESTRICT", "ON DELETE CASCADE"),
        malformed("seq = 1", "seq BETWEEN 1 AND 2"),
    ]
}

fn malformed_v9_event_cases() -> Vec<String> {
    vec![
        malformed(
            "kind = 'graph_run_prepared'",
            "kind IN ('graph_run_prepared','dispatched')",
        ),
        malformed(
            "PRIMARY KEY(graph_run_id, seq)",
            "UNIQUE(graph_run_id, seq)",
        ),
        malformed(
            "graph_id, created_at_ms DESC, id DESC",
            "graph_id, id DESC, created_at_ms DESC",
        ),
        malformed(
            "CREATE INDEX group_agent_graph_runs_created",
            "CREATE UNIQUE INDEX group_agent_graph_runs_created",
        ),
        format!("{MIGRATE_V8_TO_V9_SQL}\nCREATE TABLE rogue_v9_table(id TEXT);"),
        format!(
            "{MIGRATE_V8_TO_V9_SQL}
             CREATE TRIGGER rogue_v9_trigger AFTER INSERT ON group_agent_graph_runs
             BEGIN SELECT 1; END;"
        ),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V8_TO_V9_SQL.replacen(original, replacement, 1);
    assert_ne!(sql, MIGRATE_V8_TO_V9_SQL, "fixture replacement must match");
    sql
}

fn assert_malformed_v9_is_rejected(sql: &str) {
    let (root, database) = legacy_v8_database();
    let connection = Connection::open(&database).expect("open v8 fixture");
    connection
        .execute_batch(sql)
        .expect("forge malformed v9 schema");
    drop(connection);

    let error = open_database(&database).expect_err("malformed v9 schema is corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen malformed v9 fixture");
    assert_eq!(schema_version(&unchanged), 9);
    assert_legacy_graph(&unchanged);
    drop((unchanged, root));
}

pub(super) fn legacy_v8_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v7_database();
    let connection = Connection::open(&database).expect("open v7 fixture");
    connection
        .execute_batch(MIGRATE_V7_TO_V8_SQL)
        .expect("migrate fixture to v8");
    seed_v8_graph(&connection);
    drop(connection);
    (root, database)
}

fn seed_v8_graph(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_agent_graphs(
               id,group_run_id,graph_version,status,source_snapshot_sha256,
               manifest_blob,manifest_bytes,manifest_sha256,node_count,edge_count,
               wave_count,idempotency_key,created_at_ms
             ) VALUES(
               'graph-legacy','group-run-legacy',1,'prepared',zeroblob(32),
               x'7b7d',2,zeroblob(32),1,0,1,'graph-legacy-key',9
             );",
        )
        .expect("seed v8 Group Agent Graph");
}

fn assert_current_graph_run_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), 15);
    for table in [
        "group_agent_graph_runs",
        "group_agent_graph_run_events",
        "group_agent_graph_node_execution_contracts",
        "group_agent_graph_node_dispatch_requests",
    ] {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing current table {table}"
        );
    }
    for index in [
        "group_agent_graph_runs_graph",
        "group_agent_graph_runs_created",
        "group_agent_graph_node_contracts_project_lane",
        "group_agent_graph_node_contracts_created",
        "group_agent_graph_node_dispatch_requests_project_lane",
        "group_agent_graph_node_dispatch_requests_created",
    ] {
        assert!(
            schema_object_exists(connection, "index", index),
            "missing current index {index}"
        );
    }
}

fn assert_legacy_graph(connection: &Connection) {
    let row: (String, String, i64, i64) = connection
        .query_row(
            "SELECT group_run_id,status,node_count,created_at_ms
             FROM group_agent_graphs WHERE id='graph-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("legacy graph survives");
    assert_eq!(row, ("group-run-legacy".into(), "prepared".into(), 1, 9));
}
