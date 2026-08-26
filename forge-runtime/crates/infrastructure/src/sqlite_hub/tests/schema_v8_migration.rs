use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V6_TO_V7_SQL, MIGRATE_V7_TO_V8_SQL, legacy_v6_database,
    migrate_with_before_final_fault_for_test, open_database, schema_object_exists,
    schema_object_named, schema_version, table_columns,
};

#[test]
fn v7_synthesis_data_is_preserved_by_current_migration_and_reopen() {
    let (root, database) = legacy_v7_database();
    let connection = open_database(&database).expect("v7 Hub migrates to current");
    assert_current_shape(&connection);
    assert_legacy_synthesis(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated current Hub reopens");
    assert_current_shape(&reopened);
    assert_legacy_synthesis(&reopened);
    drop((reopened, root));
}

#[test]
fn v7_future_graph_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v7_database();
    let blocker = Connection::open(&database).expect("open v7 blocker fixture");
    blocker
        .execute_batch("CREATE TABLE group_agent_graphs(blocker TEXT)")
        .expect("install future v8 table blocker");
    drop(blocker);

    let error = open_database(&database).expect_err("v7 prefix rejects future v8 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen rejected v7 fixture");
    assert_eq!(schema_version(&unchanged), 7);
    assert_eq!(table_columns(&unchanged, "group_agent_graphs"), ["blocker"]);
    assert!(!schema_object_named(
        &unchanged,
        "group_agent_graphs_group_run"
    ));
    assert_legacy_synthesis(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v7_to_current_atomically() {
    let (root, database) = legacy_v7_database();
    let connection = Connection::open(&database).expect("open v7 rollback fixture");
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_current_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_current_final_fault(id TEXT)")
    })
    .expect_err("final current validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    assert_eq!(schema_version(&connection), 7);
    assert!(!schema_object_named(&connection, "group_agent_graphs"));
    assert!(!schema_object_named(
        &connection,
        "rogue_current_final_fault"
    ));
    assert_legacy_synthesis(&connection);
    drop(connection);

    let reopened = Connection::open(&database).expect("reopen rolled-back v7 fixture");
    assert_eq!(schema_version(&reopened), 7);
    assert_legacy_synthesis(&reopened);
    drop((reopened, root));
}

#[test]
fn malformed_v8_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v8_cases() {
        assert_malformed_v8_is_rejected(&sql);
    }
}

#[test]
fn future_schema_version_is_rejected_without_mutation() {
    let (root, database) = legacy_v7_database();
    let connection = open_database(&database).expect("migrate future-version fixture");
    connection
        .pragma_update(None, "user_version", 29)
        .expect("mark future schema");
    drop(connection);

    let error = open_database(&database).expect_err("future v29 schema is unsupported");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen future schema directly");
    assert_eq!(schema_version(&unchanged), 29);
    assert_legacy_synthesis(&unchanged);
    assert!(schema_object_named(&unchanged, "group_agent_graphs"));
    drop((unchanged, root));
}

fn malformed_v8_cases() -> Vec<String> {
    vec![
        malformed("manifest_blob BLOB NOT NULL", "manifest_blob TEXT NOT NULL"),
        malformed("manifest_bytes INTEGER NOT NULL", "manifest_bytes INTEGER"),
        malformed("id TEXT NOT NULL PRIMARY KEY", "id TEXT NOT NULL"),
        malformed(
            "idempotency_key TEXT NOT NULL UNIQUE",
            "idempotency_key TEXT NOT NULL",
        ),
        malformed("graph_version = 1", "graph_version IN (1,2)"),
        malformed("status = 'prepared'", "status IN ('prepared','running')"),
        malformed(
            "length(source_snapshot_sha256) = 32",
            "length(source_snapshot_sha256) IN (16,32)",
        ),
        malformed("REFERENCES group_runs(id)", "REFERENCES groups(id)"),
        malformed("ON DELETE RESTRICT", "ON DELETE CASCADE"),
        malformed("node_count BETWEEN 1 AND 32", "node_count BETWEEN 0 AND 32"),
        malformed("edge_count BETWEEN 0 AND 512", "edge_count >= 0"),
        malformed("wave_count <= node_count", "wave_count <= edge_count"),
        malformed(
            "group_run_id, created_at_ms DESC, id DESC",
            "group_run_id, id DESC, created_at_ms DESC",
        ),
        malformed(
            "CREATE INDEX group_agent_graphs_created",
            "CREATE UNIQUE INDEX group_agent_graphs_created",
        ),
        format!("{MIGRATE_V7_TO_V8_SQL}\nCREATE TABLE rogue_v8_table(id TEXT);"),
        format!(
            "{MIGRATE_V7_TO_V8_SQL}
             CREATE TRIGGER rogue_v8_trigger AFTER INSERT ON group_agent_graphs
             BEGIN SELECT 1; END;"
        ),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V7_TO_V8_SQL.replacen(original, replacement, 1);
    assert_ne!(sql, MIGRATE_V7_TO_V8_SQL, "fixture replacement must match");
    sql
}

fn assert_malformed_v8_is_rejected(sql: &str) {
    let (root, database) = legacy_v7_database();
    let connection = Connection::open(&database).expect("open v7 fixture");
    connection
        .execute_batch(sql)
        .expect("forge malformed v8 schema");
    drop(connection);

    let error = open_database(&database).expect_err("malformed v8 schema is corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen malformed v8 fixture");
    assert_eq!(schema_version(&unchanged), 8);
    assert_legacy_synthesis(&unchanged);
    drop((unchanged, root));
}

pub(super) fn legacy_v7_database() -> (tempfile::TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v6_database();
    let connection = Connection::open(&database).expect("open v6 fixture");
    connection
        .execute_batch(MIGRATE_V6_TO_V7_SQL)
        .expect("migrate fixture to v7");
    seed_v7_synthesis(&connection);
    drop(connection);
    (root, database)
}

fn seed_v7_synthesis(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_panel_syntheses(
               id,panel_id,group_run_id,synthesis_version,status,
               source_snapshot_sha256,panel_manifest_sha256,provider,endpoint,model,
               system_prompt_version,system_prompt_sha256,output_target,writeback_target,
               max_output_tokens,max_model_output_bytes,max_model_events,config_json,
               config_sha256,request_body,request_bytes,request_sha256,cursor_json,
               journal_bytes,idempotency_key,protocol_version,created_at_ms
             ) VALUES(
               'synthesis-legacy','panel-legacy','group-run-legacy',1,'awaiting_consent',
               zeroblob(32),zeroblob(32),'openai_responses',
               'https://api.openai.com/v1/responses','legacy-model',
               1,zeroblob(32),'local_artifact','none',64,1024,3,'{}',
               zeroblob(32),x'7b7d',2,zeroblob(32),'{}',2,
               'synthesis-legacy-key',1,8
             );",
        )
        .expect("seed v7 panel synthesis");
}

fn assert_current_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), super::SCHEMA_VERSION);
    for table in [
        "group_agent_graphs",
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
        "group_agent_graphs_group_run",
        "group_agent_graphs_created",
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

fn assert_legacy_synthesis(connection: &Connection) {
    let row: (String, String, String, i64) = connection
        .query_row(
            "SELECT panel_id,group_run_id,status,created_at_ms
             FROM group_panel_syntheses WHERE id='synthesis-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("legacy synthesis survives");
    assert_eq!(
        row,
        (
            "panel-legacy".into(),
            "group-run-legacy".into(),
            "awaiting_consent".into(),
            8
        )
    );
}
