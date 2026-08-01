use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V6_TO_V7_SQL, legacy_v6_database, migrate_with_before_final_fault_for_test,
    open_database, schema_object_exists, schema_object_named, schema_version, table_columns,
};

#[test]
fn v6_panel_data_is_preserved_by_current_migration_and_reopen() {
    let (root, database) = legacy_v6_database();
    let connection = open_database(&database).expect("v6 Hub migrates to current");
    assert_current_shape(&connection);
    assert_legacy_panel(&connection);
    drop(connection);

    let reopened = open_database(&database).expect("migrated current Hub reopens");
    assert_current_shape(&reopened);
    assert_legacy_panel(&reopened);
    drop((reopened, root));
}

#[test]
fn v6_future_synthesis_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v6_database();
    let blocker = Connection::open(&database).expect("open v6 blocker fixture");
    blocker
        .execute_batch("CREATE TABLE group_panel_syntheses(blocker TEXT)")
        .expect("install future v7 table blocker");
    drop(blocker);

    let error = open_database(&database).expect_err("v6 prefix rejects future v7 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen rejected v6 fixture");
    assert_eq!(schema_version(&unchanged), 6);
    assert_eq!(
        table_columns(&unchanged, "group_panel_syntheses"),
        ["blocker"]
    );
    assert!(!schema_object_named(
        &unchanged,
        "group_panel_synthesis_events"
    ));
    assert_legacy_panel(&unchanged);
    drop((unchanged, root));
}

#[test]
fn failed_final_validation_rolls_back_v6_to_current_atomically() {
    let (root, database) = legacy_v6_database();
    let connection = Connection::open(&database).expect("open v6 rollback fixture");
    let error = migrate_with_before_final_fault_for_test(&connection, |migrated| {
        assert_current_shape(migrated);
        migrated.execute_batch("CREATE TABLE rogue_current_final_fault(id TEXT)")
    })
    .expect_err("final current validation rejects rogue object");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));

    assert_eq!(schema_version(&connection), 6);
    assert!(!schema_object_named(&connection, "group_panel_syntheses"));
    assert!(!schema_object_named(
        &connection,
        "rogue_current_final_fault"
    ));
    assert_legacy_panel(&connection);
    drop(connection);

    let reopened = Connection::open(&database).expect("reopen rolled-back v6 fixture");
    assert_eq!(schema_version(&reopened), 6);
    assert_legacy_panel(&reopened);
    drop((reopened, root));
}

#[test]
fn malformed_v7_definitions_and_rogue_objects_are_rejected() {
    for sql in malformed_v7_cases() {
        assert_malformed_v7_is_rejected(&sql);
    }
}

fn malformed_v7_cases() -> Vec<String> {
    vec![
        malformed("request_body BLOB NOT NULL", "request_body TEXT NOT NULL"),
        malformed("journal_bytes INTEGER NOT NULL", "journal_bytes INTEGER"),
        malformed("id TEXT NOT NULL PRIMARY KEY", "id TEXT NOT NULL"),
        malformed(
            "idempotency_key TEXT NOT NULL UNIQUE",
            "idempotency_key TEXT NOT NULL",
        ),
        malformed(
            "output_target = 'local_artifact'",
            "output_target IN ('local_artifact','conversation')",
        ),
        malformed(
            "writeback_target = 'none'",
            "writeback_target IN ('none','prompt')",
        ),
        malformed(
            "REFERENCES group_analysis_panels(id)",
            "REFERENCES group_runs(id)",
        ),
        malformed(
            "REFERENCES group_panel_syntheses(id)",
            "REFERENCES group_analysis_panels(id)",
        ),
        malformed("ON DELETE RESTRICT", "ON DELETE CASCADE"),
        malformed(
            "PRIMARY KEY(synthesis_id, seq)",
            "UNIQUE(synthesis_id, seq)",
        ),
        malformed(
            "panel_id, created_at_ms DESC, id DESC",
            "panel_id, id DESC, created_at_ms DESC",
        ),
        malformed(
            "CREATE INDEX group_panel_syntheses_created",
            "CREATE UNIQUE INDEX group_panel_syntheses_created",
        ),
        format!("{MIGRATE_V6_TO_V7_SQL}\nCREATE TABLE rogue_v7_table(id TEXT);"),
        format!(
            "{MIGRATE_V6_TO_V7_SQL}
             CREATE TRIGGER rogue_v7_trigger AFTER INSERT ON group_panel_syntheses
             BEGIN SELECT 1; END;"
        ),
    ]
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V6_TO_V7_SQL.replacen(original, replacement, 1);
    assert_ne!(sql, MIGRATE_V6_TO_V7_SQL, "fixture replacement must match");
    sql
}

fn assert_malformed_v7_is_rejected(sql: &str) {
    let (root, database) = legacy_v6_database();
    let connection = Connection::open(&database).expect("open v6 fixture");
    connection
        .execute_batch(sql)
        .expect("forge malformed v7 schema");
    drop(connection);

    let error = open_database(&database).expect_err("malformed v7 schema is corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen malformed v7 fixture");
    assert_eq!(schema_version(&unchanged), 7);
    assert_legacy_panel(&unchanged);
    drop((unchanged, root));
}

fn assert_current_shape(connection: &Connection) {
    assert_eq!(schema_version(connection), 10);
    for table in [
        "group_panel_syntheses",
        "group_panel_synthesis_events",
        "group_panel_synthesis_results",
        "group_agent_graphs",
        "group_agent_graph_runs",
        "group_agent_graph_run_events",
        "group_agent_graph_node_execution_contracts",
    ] {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing current table {table}"
        );
    }
    for index in [
        "group_panel_syntheses_panel",
        "group_panel_syntheses_created",
        "group_agent_graphs_group_run",
        "group_agent_graphs_created",
        "group_agent_graph_runs_graph",
        "group_agent_graph_runs_created",
        "group_agent_graph_node_contracts_project_lane",
        "group_agent_graph_node_contracts_created",
    ] {
        assert!(
            schema_object_exists(connection, "index", index),
            "missing current index {index}"
        );
    }
}

fn assert_legacy_panel(connection: &Connection) {
    let row: (String, String, i64, i64) = connection
        .query_row(
            "SELECT group_run_id,status,analysis_count,created_at_ms
             FROM group_analysis_panels WHERE id='panel-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("legacy panel survives");
    assert_eq!(row, ("group-run-legacy".into(), "prepared".into(), 2, 7));
    let members: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM group_analysis_panel_analyses
             WHERE panel_id='panel-legacy'",
            [],
            |row| row.get(0),
        )
        .expect("legacy panel members survive");
    assert_eq!(members, 2);
}
