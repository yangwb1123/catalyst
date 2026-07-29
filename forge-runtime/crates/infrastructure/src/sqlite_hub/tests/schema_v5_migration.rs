use rusqlite::Connection;

use crate::runtime_domain::HubStoreError;

use super::{
    assert_legacy_run, assert_v4_schema, legacy_v1_database, legacy_v2_database,
    legacy_v3_database, legacy_v4_database, open_database, schema_object_exists,
    schema_object_named, schema_version, table_columns,
};

#[test]
fn v1_hub_data_is_preserved_by_atomic_v5_migration() {
    let (root, database) = legacy_v1_database();
    let connection = open_database(&database).expect("v1 Hub migrates through v5");

    assert_v5_schema(&connection);
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
fn v2_run_journal_and_assistant_are_preserved_by_v5_migration() {
    let (root, database) = legacy_v2_database();
    let connection = open_database(&database).expect("v2 Hub migrates to v5");

    assert_v5_schema(&connection);
    assert_legacy_run(&connection);
    drop((connection, root));
}

#[test]
fn v3_group_run_schema_migrates_to_v5() {
    let (root, database) = legacy_v3_database();
    let connection = open_database(&database).expect("v3 Hub migrates to v5");

    assert_v5_schema(&connection);
    assert_legacy_run(&connection);
    assert_legacy_group_run(&connection);
    drop((connection, root));
}

#[test]
fn v4_group_execution_schema_migrates_to_v5() {
    let (root, database) = legacy_v4_database();
    let connection = open_database(&database).expect("v4 Hub migrates to v5");

    assert_v5_schema(&connection);
    assert_legacy_run(&connection);
    assert_legacy_group_run(&connection);
    assert_legacy_group_execution(&connection);
    drop((connection, root));
}

#[test]
fn late_v5_conflict_rolls_back_partially_created_tables() {
    let (root, database) = legacy_v4_database();
    let blocker = Connection::open(&database).expect("open migration blocker");
    blocker
        .execute_batch("CREATE TABLE group_model_analysis_results(blocker TEXT)")
        .expect("install deterministic migration conflict");
    drop(blocker);

    open_database(&database).expect_err("conflicting v5 migration fails");
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
fn v5_conflict_rolls_back_the_entire_v1_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open migration blocker");
    blocker
        .execute_batch("CREATE TABLE group_model_analysis_results(blocker TEXT)")
        .expect("install deterministic v5 migration conflict");
    drop(blocker);

    open_database(&database).expect_err("fourth migration stage fails");
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
fn unknown_v6_schema_is_rejected_without_mutation() {
    let (root, database) = legacy_v4_database();
    let connection = Connection::open(&database).expect("open v4 fixture");
    connection
        .pragma_update(None, "user_version", 6)
        .expect("mark future schema");
    drop(connection);

    let error = open_database(&database).expect_err("v6 schema is unsupported");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen future schema");
    assert_eq!(schema_version(&unchanged), 6);
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

fn assert_v5_schema(connection: &Connection) {
    assert_eq!(schema_version(connection), 5);
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
    ] {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing v5 table {table}"
        );
    }
    for index in [
        "group_runs_group",
        "group_executions_group_run",
        "group_executions_created",
        "group_model_analyses_group_run",
        "group_model_analyses_created",
    ] {
        assert!(
            schema_object_exists(connection, "index", index),
            "missing v5 index {index}"
        );
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
