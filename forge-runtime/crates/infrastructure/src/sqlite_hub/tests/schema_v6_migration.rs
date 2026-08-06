use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V5_TO_V6_SQL, legacy_v5_database, migrate_with_before_final_fault_for_test,
    open_database, schema_object_exists, schema_object_named, schema_version, table_columns,
};

#[test]
fn v5_analysis_data_is_preserved_by_current_migration_and_reopen() {
    let (root, database) = legacy_v5_database();
    let connection = open_database(&database).expect("v5 Hub migrates to current");
    assert_current_shape(&connection);
    assert_legacy_analysis(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated current Hub reopens");
    assert_current_shape(&reopened);
    assert_legacy_analysis(&reopened);
    drop((reopened, root));
}

#[test]
fn v5_future_panel_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v5_database();
    let blocker = Connection::open(&database).expect("open v5 blocker fixture");
    blocker
        .execute_batch("CREATE TABLE group_analysis_panels(blocker TEXT)")
        .expect("install future v6 table blocker");
    drop(blocker);

    let error = open_database(&database).expect_err("v5 prefix rejects future v6 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen rejected v5 fixture");
    assert_eq!(schema_version(&unchanged), 5);
    assert_eq!(
        table_columns(&unchanged, "group_analysis_panels"),
        ["blocker"]
    );
    assert!(!schema_object_named(
        &unchanged,
        "group_analysis_panel_analyses"
    ));
    assert_legacy_analysis(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v5_to_current_atomically() {
    let (root, database) = legacy_v5_database();
    let connection = Connection::open(&database).expect("open v5 rollback fixture");
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_current_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_current_final_fault(id TEXT)")
    })
    .expect_err("final current validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    assert_eq!(schema_version(&connection), 5);
    assert!(!schema_object_named(&connection, "group_analysis_panels"));
    assert!(!schema_object_named(
        &connection,
        "rogue_current_final_fault"
    ));
    assert_legacy_analysis(&connection);
    drop(connection);

    let reopened = Connection::open(&database).expect("reopen rolled-back v5 fixture");
    assert_eq!(schema_version(&reopened), 5);
    assert_legacy_analysis(&reopened);
    drop((reopened, root));
}

#[test]
fn malformed_v6_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v6_cases() {
        assert_malformed_v6_is_rejected(&sql);
    }
}

fn malformed_v6_cases() -> Vec<String> {
    vec![
        malformed("manifest_blob BLOB NOT NULL", "manifest_blob TEXT NOT NULL"),
        malformed("analysis_count INTEGER NOT NULL", "analysis_count INTEGER"),
        malformed("id TEXT NOT NULL PRIMARY KEY", "id TEXT NOT NULL"),
        malformed(
            "idempotency_key TEXT NOT NULL UNIQUE",
            "idempotency_key TEXT NOT NULL",
        ),
        malformed(
            "PRIMARY KEY(panel_id, position)",
            "UNIQUE(panel_id, position)",
        ),
        malformed(
            "UNIQUE(panel_id, analysis_id)",
            "UNIQUE(position, analysis_id)",
        ),
        malformed("REFERENCES group_runs(id)", "REFERENCES groups(id)"),
        malformed(
            "REFERENCES group_analysis_panels(id)",
            "REFERENCES group_runs(id)",
        ),
        malformed(
            "REFERENCES group_model_analysis_results(analysis_id)",
            "REFERENCES group_model_analyses(id)",
        ),
        malformed("ON DELETE RESTRICT", "ON DELETE CASCADE"),
        malformed(
            "group_run_id, created_at_ms DESC, id DESC",
            "group_run_id, id DESC, created_at_ms DESC",
        ),
        malformed(
            "CREATE INDEX group_analysis_panels_created",
            "CREATE UNIQUE INDEX group_analysis_panels_created",
        ),
        format!("{MIGRATE_V5_TO_V6_SQL}\nCREATE TABLE rogue_v6_table(id TEXT);"),
        format!(
            "{MIGRATE_V5_TO_V6_SQL}
             CREATE TRIGGER rogue_v6_trigger AFTER INSERT ON group_analysis_panels
             BEGIN SELECT 1; END;"
        ),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V5_TO_V6_SQL.replacen(original, replacement, 1);
    assert_ne!(sql, MIGRATE_V5_TO_V6_SQL, "fixture replacement must match");
    sql
}

fn assert_malformed_v6_is_rejected(sql: &str) {
    let (root, database) = legacy_v5_database();
    let connection = Connection::open(&database).expect("open v5 fixture");
    connection
        .execute_batch(sql)
        .expect("forge malformed v6 schema");
    drop(connection);

    let error = open_database(&database).expect_err("malformed v6 schema is corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen malformed v6 fixture");
    assert_eq!(schema_version(&unchanged), 6);
    assert_legacy_analysis(&unchanged);
    drop((unchanged, root));
}

fn assert_current_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), 19);
    for table in [
        "group_analysis_panels",
        "group_analysis_panel_analyses",
        "group_panel_syntheses",
        "group_panel_synthesis_events",
        "group_panel_synthesis_results",
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
        "group_analysis_panels_group_run",
        "group_analysis_panels_created",
        "group_panel_syntheses_panel",
        "group_panel_syntheses_created",
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

fn assert_legacy_analysis(connection: &Connection) {
    let row: (String, String, i64, i64) = connection
        .query_row(
            "SELECT group_run_id,status,request_bytes,created_at_ms
             FROM group_model_analyses WHERE id='analysis-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("legacy analysis survives");
    assert_eq!(row, ("group-run-legacy".into(), "completed".into(), 2, 5));
    let result: (i64, i64) = connection
        .query_row(
            "SELECT result_bytes,created_at_ms FROM group_model_analysis_results
             WHERE analysis_id='analysis-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("legacy analysis result survives");
    assert_eq!(result, (2, 5));
}
