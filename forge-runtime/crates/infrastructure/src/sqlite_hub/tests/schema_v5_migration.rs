use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    MIGRATE_V4_TO_V5_SQL, assert_legacy_run, assert_v4_schema, legacy_v1_database,
    legacy_v2_database, legacy_v3_database, legacy_v4_database, open_database,
    schema_object_exists, schema_object_named, schema_version, table_columns,
};

#[test]
fn v1_hub_data_is_preserved_by_atomic_current_migration() {
    let (root, database) = legacy_v1_database();
    let connection = open_database(&database).expect("v1 Hub migrates to current");

    assert_current_schema(&connection);
    let prompt: String = connection
        .query_row(
            "SELECT content FROM prompts WHERE id = 'prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("legacy Prompt survives");
    assert_eq!(prompt, "preserve me");
    drop((connection, root));
}

#[test]
fn v2_run_journal_and_assistant_are_preserved_by_current_migration() {
    let (root, database) = legacy_v2_database();
    let connection = open_database(&database).expect("v2 Hub migrates to current");

    assert_current_schema(&connection);
    assert_legacy_run(&connection);
    drop((connection, root));
}

#[test]
fn v3_group_run_schema_migrates_to_current() {
    let (root, database) = legacy_v3_database();
    let connection = open_database(&database).expect("v3 Hub migrates to current");

    assert_current_schema(&connection);
    assert_legacy_run(&connection);
    assert_legacy_group_run(&connection);
    drop((connection, root));
}

#[test]
fn v4_group_execution_schema_migrates_to_current() {
    let (root, database) = legacy_v4_database();
    let connection = open_database(&database).expect("v4 Hub migrates to current");

    assert_current_schema(&connection);
    assert_legacy_run(&connection);
    assert_legacy_group_run(&connection);
    assert_legacy_group_execution(&connection);
    drop((connection, root));
}

#[test]
fn v2_future_group_runs_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v2_database();
    let blocker = Connection::open(&database).expect("open v2 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_runs(blocker TEXT)")
        .expect("install future v3 table blocker");
    drop(blocker);

    let error = open_database(&database).expect_err("v2 prefix rejects future v3 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v2 database");
    assert_eq!(schema_version(&unchanged), 2);
    assert_legacy_run(&unchanged);
    assert_eq!(table_columns(&unchanged, "group_runs"), vec!["blocker"]);
    assert!(!schema_object_exists(
        &unchanged,
        "index",
        "group_runs_group"
    ));
    drop((unchanged, root));
}

#[test]
fn v4_future_analysis_result_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v4_database();
    let blocker = Connection::open(&database).expect("open v4 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_model_analysis_results(blocker TEXT)")
        .expect("install future v5 result-table blocker");
    drop(blocker);

    let error = open_database(&database).expect_err("v4 prefix rejects future v5 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v4 database");
    assert_v4_schema(&unchanged);
    assert_eq!(
        table_columns(&unchanged, "group_model_analysis_results"),
        vec!["blocker"]
    );
    for object in [
        "group_model_analyses",
        "group_model_analysis_events",
        "group_model_analyses_group_run",
        "group_model_analyses_created",
    ] {
        assert!(!schema_object_named(&unchanged, object));
    }
    drop((unchanged, root));
}

#[test]
fn v1_future_analysis_result_blocker_is_rejected_before_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open v1 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_model_analysis_results(blocker TEXT)")
        .expect("install future v5 result-table blocker");
    drop(blocker);

    let error = open_database(&database).expect_err("v1 prefix rejects future v5 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v1 database");
    assert_eq!(schema_version(&unchanged), 1);
    for table in [
        "runs",
        "run_events",
        "run_assistant_prompts",
        "group_runs",
        "group_executions",
        "group_execution_events",
    ] {
        assert!(!schema_object_exists(&unchanged, "table", table));
    }
    assert_eq!(
        table_columns(&unchanged, "group_model_analysis_results"),
        vec!["blocker"]
    );
    drop((unchanged, root));
}

#[test]
fn malformed_v7_marker_is_rejected_without_mutation() {
    let (root, database) = legacy_v4_database();
    let connection = Connection::open(&database).expect("open v4 fixture");
    connection
        .pragma_update(None, "user_version", 7)
        .expect("mark future schema");
    drop(connection);

    let error = open_database(&database).expect_err("incomplete v7 schema is corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen future schema");
    assert_eq!(schema_version(&unchanged), 7);
    assert_legacy_run(&unchanged);
    assert!(schema_object_exists(
        &unchanged,
        "table",
        "group_executions"
    ));
    assert!(!schema_object_exists(
        &unchanged,
        "table",
        "group_model_analyses"
    ));
    drop((unchanged, root));
}

#[test]
fn malformed_v5_schema_is_rejected_without_mutation() {
    let (root, database) = legacy_v4_database();
    let connection = Connection::open(&database).expect("open v4 fixture");
    connection
        .pragma_update(None, "user_version", 5)
        .expect("forge malformed v5 marker");
    drop(connection);

    let error = open_database(&database).expect_err("missing v5 tables are corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen malformed database");
    assert_eq!(schema_version(&unchanged), 5);
    assert!(!schema_object_exists(
        &unchanged,
        "table",
        "group_model_analyses"
    ));
    drop((unchanged, root));
}

#[test]
fn malformed_v5_definitions_are_rejected() {
    for sql in malformed_column_cases()
        .into_iter()
        .chain(malformed_relation_cases())
    {
        assert_malformed_v5_is_rejected(&sql);
    }
}

fn malformed_column_cases() -> [String; 6] {
    [
        malformed("model TEXT NOT NULL", "model BLOB NOT NULL"),
        malformed("status TEXT NOT NULL", "status TEXT"),
        malformed("model TEXT NOT NULL", "model TEXT NOT NULL DEFAULT 'model'"),
        malformed(
            "provider TEXT NOT NULL\n    CHECK(typeof(provider) = 'text' AND provider = 'openai_responses')",
            "provider TEXT NOT NULL",
        ),
        malformed(
            "analysis_id TEXT NOT NULL PRIMARY KEY",
            "analysis_id TEXT NOT NULL",
        ),
        malformed(
            "idempotency_key TEXT NOT NULL UNIQUE",
            "idempotency_key TEXT NOT NULL",
        ),
    ]
}

fn malformed_relation_cases() -> [String; 13] {
    [
        malformed(
            "group_run_id, created_at_ms DESC, id DESC",
            "group_run_id, model DESC, id DESC",
        ),
        malformed("created_at_ms DESC, id DESC", "created_at_ms, id DESC"),
        malformed(
            "group_run_id, created_at_ms DESC, id DESC",
            "group_run_id COLLATE NOCASE, created_at_ms DESC, id DESC",
        ),
        malformed(
            "CREATE INDEX group_model_analyses_created",
            "CREATE UNIQUE INDEX group_model_analyses_created",
        ),
        wrong_source_foreign_key(),
        malformed("REFERENCES group_runs(id)", "REFERENCES groups(id)"),
        malformed(
            "REFERENCES group_runs(id)",
            "REFERENCES group_runs(group_id)",
        ),
        malformed(
            "REFERENCES group_runs(id)",
            "REFERENCES group_runs(id) MATCH FULL",
        ),
        malformed("ON DELETE RESTRICT", "ON UPDATE CASCADE ON DELETE RESTRICT"),
        malformed("ON DELETE RESTRICT", "ON DELETE CASCADE"),
        format!(
            "{MIGRATE_V4_TO_V5_SQL}
CREATE UNIQUE INDEX rogue_v5_index ON group_model_analyses(model);"
        ),
        format!(
            "{MIGRATE_V4_TO_V5_SQL}
CREATE VIRTUAL TABLE pragma_index_list
USING fts5(seq,name,\"unique\",origin,partial);
CREATE UNIQUE INDEX shadowed_rogue_index ON group_model_analyses(model);"
        ),
        format!(
            "{MIGRATE_V4_TO_V5_SQL}
CREATE TRIGGER rogue_v5_trigger AFTER INSERT ON GROUP_MODEL_ANALYSES
BEGIN SELECT 1; END;"
        ),
    ]
}

fn wrong_source_foreign_key() -> String {
    let source_fk = MIGRATE_V4_TO_V5_SQL
        .replacen(
            "group_run_id TEXT NOT NULL REFERENCES group_runs(id) ON DELETE RESTRICT",
            "group_run_id TEXT NOT NULL",
            1,
        )
        .replacen(
            "model TEXT NOT NULL",
            "model TEXT NOT NULL REFERENCES group_runs(id) ON DELETE RESTRICT",
            1,
        );
    assert_ne!(
        source_fk, MIGRATE_V4_TO_V5_SQL,
        "source FK replacement must match"
    );
    source_fk
}

fn malformed(original: &str, replacement: &str) -> String {
    let sql = MIGRATE_V4_TO_V5_SQL.replacen(original, replacement, 1);
    assert_ne!(sql, MIGRATE_V4_TO_V5_SQL, "fixture replacement must match");
    sql
}

fn assert_malformed_v5_is_rejected(sql: &str) {
    let (root, database) = legacy_v4_database();
    let connection = Connection::open(&database).expect("open v4 fixture");
    connection
        .execute_batch(sql)
        .expect("forge malformed v5 schema");
    drop(connection);

    let error = open_database(&database).expect_err("malformed v5 definition is corrupt");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen malformed v5 database");
    assert_eq!(schema_version(&unchanged), 5);
    drop((unchanged, root));
}

fn assert_current_schema(connection: &Connection) {
    assert_eq!(schema_version(connection), super::SCHEMA_VERSION);
    for table in [
        "runs",
        "run_events",
        "run_assistant_prompts",
        "group_runs",
        "group_executions",
        "group_execution_events",
        "group_model_analyses",
        "group_model_analysis_events",
        "group_model_analysis_results",
        "group_analysis_panels",
        "group_analysis_panel_analyses",
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
    assert_current_indexes(connection);
    assert_v11_dispatch_objects(connection);
}

fn assert_current_indexes(connection: &Connection) {
    for index in [
        "group_runs_group",
        "group_executions_group_run",
        "group_executions_created",
        "group_model_analyses_group_run",
        "group_model_analyses_created",
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
    ] {
        assert!(
            schema_object_exists(connection, "index", index),
            "missing current index {index}"
        );
    }
}

fn assert_v11_dispatch_objects(connection: &Connection) {
    assert!(schema_object_exists(
        connection,
        "table",
        "group_agent_graph_node_dispatch_requests"
    ));
    for index in [
        "group_agent_graph_node_dispatch_requests_project_lane",
        "group_agent_graph_node_dispatch_requests_created",
    ] {
        assert!(schema_object_exists(connection, "index", index));
    }
}

fn assert_legacy_group_run(connection: &Connection) {
    let row: (String, String, i64, i64) = connection
        .query_row(
            "SELECT group_id,status,length(context_blob),created_at_ms
             FROM group_runs WHERE id = 'group-run-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("legacy Group Run survives");
    assert_eq!(row, ("group-legacy".into(), "prepared".into(), 2, 3));
}

fn assert_legacy_group_execution(connection: &Connection) {
    let row: (String, String, i64) = connection
        .query_row(
            "SELECT group_run_id,status,journal_bytes
             FROM group_executions WHERE id = 'group-execution-legacy'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("legacy Group Execution survives");
    let event: String = connection
        .query_row(
            "SELECT event_json FROM group_execution_events
             WHERE execution_id = 'group-execution-legacy' AND seq = 1",
            [],
            |row| row.get(0),
        )
        .expect("legacy Group Execution event survives");
    assert_eq!(row, ("group-run-legacy".into(), "incomplete".into(), 18));
    assert_eq!(event, r#"{"legacy":"event"}"#);
}
