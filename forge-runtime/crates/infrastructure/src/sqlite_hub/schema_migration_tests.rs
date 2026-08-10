use super::{
    schema::{
        migrate_with_before_final_fault_for_test, open_database,
        open_existing_dispatch_preflight_read_only_database,
        open_existing_dispatch_reentry_read_only_database,
    },
    schema_sql::{
        CREATE_V1_SCHEMA_SQL, MIGRATE_V1_TO_V2_SQL, MIGRATE_V2_TO_V3_SQL, MIGRATE_V3_TO_V4_SQL,
        MIGRATE_V4_TO_V5_SQL, MIGRATE_V5_TO_V6_SQL, MIGRATE_V6_TO_V7_SQL, MIGRATE_V7_TO_V8_SQL,
    },
    schema_v9_sql::MIGRATE_V8_TO_V9_SQL,
    schema_v10_sql::MIGRATE_V9_TO_V10_SQL,
    schema_v11_sql::MIGRATE_V10_TO_V11_SQL,
    schema_v12_sql::MIGRATE_V11_TO_V12_SQL,
    schema_v13_sql::MIGRATE_V12_TO_V13_SQL,
    schema_v14_sql::MIGRATE_V13_TO_V14_SQL,
    schema_v15_sql::MIGRATE_V14_TO_V15_SQL,
    schema_v21_sql::MIGRATE_V20_TO_V21_SQL,
    schema_v22_sql::MIGRATE_V21_TO_V22_SQL,
    schema_v23_sql::MIGRATE_V22_TO_V23_SQL,
    schema_v24_sql::MIGRATE_V23_TO_V24_SQL,
    schema_v25_sql::MIGRATE_V24_TO_V25_SQL,
};
use crate::runtime_domain::HubStoreError;
use rusqlite::Connection;
use tempfile::TempDir;
#[path = "tests/schema_full_validation.rs"]
mod schema_full_validation_tests;
pub(super) const RESTORE_HISTORICAL_ANALYSES_SQL: &str =
    include_str!("tests/restore_historical_analyses.sql");
#[path = "tests/schema_migration_support.rs"]
mod schema_migration_support;
#[path = "tests/schema_open_adversarial.rs"]
mod schema_open_adversarial_tests;
#[path = "tests/schema_release_golden.rs"]
mod schema_release_golden_tests;
#[path = "tests/schema_transaction_rollback.rs"]
mod schema_transaction_rollback_tests;
#[path = "tests/schema_v10_migration.rs"]
mod schema_v10_migration_tests;
#[path = "tests/schema_v11_migration.rs"]
mod schema_v11_migration_tests;
#[path = "tests/schema_v12_migration.rs"]
mod schema_v12_migration_tests;
#[path = "tests/schema_v13_migration.rs"]
mod schema_v13_migration_tests;
#[path = "tests/schema_v14_migration.rs"]
mod schema_v14_migration_tests;
#[path = "tests/schema_v15_migration.rs"]
mod schema_v15_migration_tests;
#[path = "tests/schema_v24_migration.rs"]
mod schema_v24_migration_tests;
#[path = "tests/schema_v25_migration.rs"]
mod schema_v25_migration_tests;
#[path = "tests/schema_v26_migration.rs"]
mod schema_v26_migration_tests;
#[path = "tests/schema_v5_migration.rs"]
mod schema_v5_migration_tests;
#[path = "tests/schema_v6_migration.rs"]
mod schema_v6_migration_tests;
#[path = "tests/schema_v7_migration.rs"]
mod schema_v7_migration_tests;
#[path = "tests/schema_v8_migration.rs"]
mod schema_v8_migration_tests;
#[path = "tests/schema_v9_migration.rs"]
mod schema_v9_migration_tests;
#[path = "../../tests/sqlite_group_agent_graph_execution_schedule_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_execution_schedule_support;
#[path = "../../tests/sqlite_group_agent_graph_run_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_graph_run_support;
#[path = "../../tests/sqlite_group_agent_scheduled_node_contract_support/mod.rs"]
#[allow(dead_code, clippy::duplicate_mod)]
mod sqlite_group_agent_scheduled_node_contract_support;
use schema_migration_support::{
    restrict_fixture_root, schema_object_exists, schema_version, table_columns,
};
#[test]
fn v1_future_group_runs_blocker_is_rejected_before_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open v1 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_runs(blocker TEXT)")
        .expect("install future v3 table blocker");
    drop(blocker);
    let error = open_database(&database).expect_err("v1 prefix rejects future v3 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v1 database");
    assert_eq!(schema_version(&unchanged), 1);
    for table in ["runs", "run_events", "run_assistant_prompts"] {
        assert!(
            !schema_object_exists(&unchanged, "table", table),
            "unexpected post-v1 table exists: {table}"
        );
    }
    assert_eq!(table_columns(&unchanged, "group_runs"), vec!["blocker"]);
    let prompt: String = unchanged
        .query_row(
            "SELECT content FROM prompts WHERE id='prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("v1 Prompt remains after prefix rejection");
    assert_eq!(prompt, "preserve me");
    drop((unchanged, root));
}
#[test]
fn v1_future_group_executions_blocker_is_rejected_before_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open v1 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_executions(blocker TEXT)")
        .expect("install future v4 table blocker");
    drop(blocker);
    let error = open_database(&database).expect_err("v1 prefix rejects future v4 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v1 database");
    assert_eq!(schema_version(&unchanged), 1);
    for table in [
        "runs",
        "run_events",
        "run_assistant_prompts",
        "group_runs",
        "group_execution_events",
    ] {
        assert!(
            !schema_object_exists(&unchanged, "table", table),
            "unexpected post-v1 table exists: {table}"
        );
    }
    assert_eq!(
        table_columns(&unchanged, "group_executions"),
        vec!["blocker"]
    );
    for index in [
        "group_runs_group",
        "group_executions_group_run",
        "group_executions_created",
    ] {
        assert!(!schema_object_named(&unchanged, index));
    }
    let prompt: String = unchanged
        .query_row(
            "SELECT content FROM prompts WHERE id='prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("v1 Prompt remains after prefix rejection");
    assert_eq!(prompt, "preserve me");
    drop((unchanged, root));
}
#[test]
fn v3_future_group_executions_blocker_is_rejected_before_migration() {
    let (root, database) = legacy_v3_database();
    let blocker = Connection::open(&database).expect("open v3 future-table fixture");
    blocker
        .execute_batch("CREATE TABLE group_executions(blocker TEXT)")
        .expect("install future v4 table blocker");
    drop(blocker);
    let error = open_database(&database).expect_err("v3 prefix rejects future v4 table");
    assert!(matches!(error, HubStoreError::Corrupt { .. }));
    let unchanged = Connection::open(&database).expect("reopen unchanged v3 database");
    assert_v3_schema(&unchanged);
    assert_eq!(
        table_columns(&unchanged, "group_executions"),
        vec!["blocker"]
    );
    for object in ["group_execution_events", "group_executions_group_run"] {
        assert!(!schema_object_named(&unchanged, object));
    }
    drop((unchanged, root));
}
fn assert_v4_schema(connection: &Connection) {
    assert_eq!(schema_version(connection), 4);
    for table in [
        "runs",
        "run_events",
        "run_assistant_prompts",
        "group_runs",
        "group_executions",
        "group_execution_events",
    ] {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing v4 table {table}"
        );
    }
}
fn assert_v3_schema(connection: &Connection) {
    assert_eq!(schema_version(connection), 3);
    for table in ["runs", "run_events", "run_assistant_prompts", "group_runs"] {
        assert!(
            schema_object_exists(connection, "table", table),
            "missing v3 table {table}"
        );
    }
    assert!(schema_object_exists(
        connection,
        "index",
        "group_runs_group"
    ));
}
fn schema_object_named(connection: &Connection, name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = ?1)",
            [name],
            |row| row.get(0),
        )
        .expect("schema object name query")
}
fn assert_legacy_run(connection: &Connection) {
    let run: (String, String, String, i64, String) = connection
        .query_row(
            "SELECT project_id,execution_json,cursor_json,journal_bytes,idempotency_key
             FROM runs WHERE id = 'run-1'",
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
        .expect("legacy Run survives");
    assert_eq!(
        run,
        (
            "project-1".into(),
            r#"{"legacy":"execution"}"#.into(),
            r#"{"legacy":"cursor"}"#.into(),
            17,
            "run-key".into()
        )
    );
    assert_legacy_event_and_assistant(connection);
}
fn assert_legacy_event_and_assistant(connection: &Connection) {
    let event: String = connection
        .query_row(
            "SELECT event_json FROM run_events WHERE run_id = 'run-1' AND seq = 1",
            [],
            |row| row.get(0),
        )
        .expect("legacy journal event survives");
    let assistant: (String, String, String) = connection
        .query_row(
            "SELECT w.prompt_id,p.role,p.content
             FROM run_assistant_prompts w
             JOIN prompts p ON p.id = w.prompt_id
             WHERE w.run_id = 'run-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("legacy assistant association survives");
    assert_eq!(event, r#"{"legacy":"event"}"#);
    assert_eq!(
        assistant,
        (
            "assistant-1".into(),
            "assistant".into(),
            "legacy answer".into()
        )
    );
}
fn legacy_v1_database() -> (TempDir, std::path::PathBuf) {
    let root = TempDir::new().expect("legacy Hub root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let connection = Connection::open(&database).expect("create v1 database");
    connection
        .execute_batch(CREATE_V1_SCHEMA_SQL)
        .expect("create legacy schema");
    seed_v1_records(&connection);
    drop(connection);
    (root, database)
}
fn legacy_v2_database() -> (TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v1_database();
    let connection = Connection::open(&database).expect("open v1 fixture");
    connection
        .execute_batch(MIGRATE_V1_TO_V2_SQL)
        .expect("migrate fixture to v2");
    seed_v2_run(&connection);
    drop(connection);
    (root, database)
}
fn legacy_v3_database() -> (TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v2_database();
    let connection = Connection::open(&database).expect("open v2 fixture");
    connection
        .execute_batch(MIGRATE_V2_TO_V3_SQL)
        .expect("migrate fixture to v3");
    seed_v3_group_run(&connection);
    drop(connection);
    (root, database)
}
fn legacy_v4_database() -> (TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v3_database();
    let connection = Connection::open(&database).expect("open v3 fixture");
    connection
        .execute_batch(MIGRATE_V3_TO_V4_SQL)
        .expect("migrate fixture to v4");
    seed_v4_group_execution(&connection);
    drop(connection);
    (root, database)
}
fn legacy_v5_database() -> (TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v4_database();
    let connection = Connection::open(&database).expect("open v4 fixture");
    connection
        .execute_batch(MIGRATE_V4_TO_V5_SQL)
        .expect("migrate fixture to v5");
    seed_v5_analysis(&connection);
    drop(connection);
    (root, database)
}
fn legacy_v6_database() -> (TempDir, std::path::PathBuf) {
    let (root, database) = legacy_v5_database();
    let connection = Connection::open(&database).expect("open v5 fixture");
    connection
        .execute_batch(MIGRATE_V5_TO_V6_SQL)
        .expect("migrate fixture to v6");
    seed_v6_panel(&connection);
    drop(connection);
    (root, database)
}
fn seed_v1_records(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO projects(id,name,canonical_path,created_at_ms)
               VALUES('project-1','project','/workspace/project',1);
             INSERT INTO conversations(
               id,scope_kind,scope_id,title,idempotency_key,created_at_ms,updated_at_ms
             ) VALUES(
               'conversation-1','project','project-1','Legacy','conversation-key',1,1
             );
             INSERT INTO prompts(
               id,conversation_id,role,content,idempotency_key,created_at_ms
             ) VALUES(
               'prompt-1','conversation-1','user','preserve me','prompt-key',1
             );",
        )
        .expect("seed v1 records");
}
fn seed_v2_run(connection: &Connection) {
    connection
        .execute_batch(
            r#"INSERT INTO prompts(
                 id,conversation_id,role,content,idempotency_key,created_at_ms
               ) VALUES(
                 'assistant-1','conversation-1','assistant','legacy answer','answer-key',2
               );
               INSERT INTO runs(
                 id,conversation_id,prompt_id,project_id,execution_json,cursor_json,
                 journal_bytes,idempotency_key,protocol_version,created_at_ms
               ) VALUES(
                 'run-1','conversation-1','prompt-1','project-1',
                 '{"legacy":"execution"}','{"legacy":"cursor"}',17,'run-key',1,2
               );
               INSERT INTO run_events(run_id,seq,event_json)
                 VALUES('run-1',1,'{"legacy":"event"}');
               INSERT INTO run_assistant_prompts(run_id,prompt_id)
                 VALUES('run-1','assistant-1');"#,
        )
        .expect("seed v2 Run records");
}
fn seed_v3_group_run(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO groups(id,name,idempotency_key,created_at_ms)
               VALUES('group-legacy','Legacy Group','group-legacy-key',3);
             INSERT INTO group_runs(
               id,group_id,run_version,status,context_version,context_slice_sha256,
               context_blob,snapshot_sha256,idempotency_key,created_at_ms
             ) VALUES(
               'group-run-legacy','group-legacy',1,'prepared',1,zeroblob(32),
               x'7b7d',zeroblob(32),'group-run-legacy-key',3
             );",
        )
        .expect("seed v3 Group Run");
}
fn seed_v4_group_execution(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_executions(
               id,group_run_id,execution_version,mode,status,source_snapshot_sha256,
               cursor_json,journal_bytes,idempotency_key,protocol_version,created_at_ms
             ) VALUES(
               'group-execution-legacy','group-run-legacy',1,
               'offline_snapshot_validation','incomplete',zeroblob(32),
               '{}',18,'group-execution-legacy-key',1,4
             );
             INSERT INTO group_execution_events(
               execution_id,seq,event_json,event_sha256
             ) VALUES(
               'group-execution-legacy',1,'{\"legacy\":\"event\"}',zeroblob(32)
             );",
        )
        .expect("seed v4 Group Execution");
}
fn seed_v5_analysis(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_model_analyses(
               id,group_run_id,analysis_version,status,source_snapshot_sha256,
               provider,endpoint,model,system_prompt_version,system_prompt_sha256,
               max_output_tokens,max_model_output_bytes,max_model_events,config_json,
               config_sha256,request_body,request_bytes,request_sha256,cursor_json,
               journal_bytes,idempotency_key,protocol_version,created_at_ms
             ) VALUES(
               'analysis-legacy','group-run-legacy',1,'completed',zeroblob(32),
               'openai_responses','https://api.openai.com/v1/responses','legacy-model',
               1,zeroblob(32),64,1024,3,'{}',zeroblob(32),x'7b7d',2,
               zeroblob(32),'{}',2,'analysis-legacy-key',1,5
             );
             INSERT INTO group_model_analysis_events(
               analysis_id,seq,event_json,event_sha256
             ) VALUES('analysis-legacy',1,'{}',zeroblob(32));
             INSERT INTO group_model_analysis_results(
               analysis_id,result_version,result_blob,result_bytes,result_sha256,created_at_ms
             ) VALUES('analysis-legacy',1,x'7b7d',2,zeroblob(32),5);",
        )
        .expect("seed v5 model analysis");
}
fn seed_v6_panel(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO group_model_analyses(
               id,group_run_id,analysis_version,status,source_snapshot_sha256,
               provider,endpoint,model,system_prompt_version,system_prompt_sha256,
               max_output_tokens,max_model_output_bytes,max_model_events,config_json,
               config_sha256,request_body,request_bytes,request_sha256,cursor_json,
               journal_bytes,idempotency_key,protocol_version,created_at_ms
             ) SELECT
               'analysis-legacy-2',group_run_id,analysis_version,status,source_snapshot_sha256,
               provider,endpoint,model,system_prompt_version,system_prompt_sha256,
               max_output_tokens,max_model_output_bytes,max_model_events,config_json,
               config_sha256,request_body,request_bytes,request_sha256,cursor_json,
               journal_bytes,'analysis-legacy-key-2',protocol_version,6
             FROM group_model_analyses WHERE id='analysis-legacy';
             INSERT INTO group_model_analysis_events(
               analysis_id,seq,event_json,event_sha256
             ) VALUES('analysis-legacy-2',1,'{}',zeroblob(32));
             INSERT INTO group_model_analysis_results(
               analysis_id,result_version,result_blob,result_bytes,result_sha256,created_at_ms
             ) VALUES('analysis-legacy-2',1,x'7b7d',2,zeroblob(32),6);
             INSERT INTO group_analysis_panels(
               id,group_run_id,panel_version,status,source_snapshot_sha256,
               analysis_count,manifest_blob,manifest_bytes,manifest_sha256,
               idempotency_key,created_at_ms
             ) VALUES(
               'panel-legacy','group-run-legacy',1,'prepared',zeroblob(32),
               2,x'7b7d',2,zeroblob(32),'panel-legacy-key',7
             );
             INSERT INTO group_analysis_panel_analyses(
               panel_id,position,analysis_id,result_sha256
             ) VALUES
               ('panel-legacy',0,'analysis-legacy',zeroblob(32)),
               ('panel-legacy',1,'analysis-legacy-2',zeroblob(32));",
        )
        .expect("seed v6 analysis panel");
}
