use rusqlite::Connection;
use tempfile::TempDir;

use super::{
    schema::open_database,
    schema_sql::{
        CREATE_V1_SCHEMA_SQL, MIGRATE_V1_TO_V2_SQL, MIGRATE_V2_TO_V3_SQL, MIGRATE_V3_TO_V4_SQL,
    },
};

#[path = "tests/schema_v5_migration.rs"]
mod schema_v5_migration_tests;

#[test]
fn conflicting_group_runs_table_rolls_back_v2_to_v3_migration() {
    let (root, database) = legacy_v2_database();
    let blocker = Connection::open(&database).expect("open migration blocker");
    blocker
        .execute_batch("CREATE TABLE group_runs(blocker TEXT)")
        .expect("install deterministic migration conflict");
    drop(blocker);

    open_database(&database).expect_err("conflicting v3 migration fails");
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
fn v3_conflict_rolls_back_the_entire_v1_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open migration blocker");
    blocker
        .execute_batch("CREATE TABLE group_runs(blocker TEXT)")
        .expect("install deterministic migration conflict");
    drop(blocker);

    open_database(&database).expect_err("second migration stage fails");
    let unchanged = Connection::open(&database).expect("reopen unchanged v1 database");
    assert_eq!(schema_version(&unchanged), 1);
    for table in ["runs", "run_events", "run_assistant_prompts"] {
        assert!(
            !schema_object_exists(&unchanged, "table", table),
            "rolled-back v2 table remains: {table}"
        );
    }
    assert_eq!(table_columns(&unchanged, "group_runs"), vec!["blocker"]);
    let prompt: String = unchanged
        .query_row(
            "SELECT content FROM prompts WHERE id='prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("v1 Prompt survives rollback");
    assert_eq!(prompt, "preserve me");
    drop((unchanged, root));
}

#[test]
fn v4_conflict_rolls_back_the_entire_v1_migration_chain() {
    let (root, database) = legacy_v1_database();
    let blocker = Connection::open(&database).expect("open migration blocker");
    blocker
        .execute_batch("CREATE TABLE group_executions(blocker TEXT)")
        .expect("install deterministic v4 migration conflict");
    drop(blocker);

    open_database(&database).expect_err("third migration stage fails");
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
            "rolled-back migration table remains: {table}"
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
        .expect("v1 Prompt survives v4 rollback");
    assert_eq!(prompt, "preserve me");
    drop((unchanged, root));
}

#[test]
fn conflicting_group_executions_table_rolls_back_v3_to_v4_migration() {
    let (root, database) = legacy_v3_database();
    let blocker = Connection::open(&database).expect("open migration blocker");
    blocker
        .execute_batch("CREATE TABLE group_executions(blocker TEXT)")
        .expect("install deterministic migration conflict");
    drop(blocker);

    open_database(&database).expect_err("conflicting v4 migration fails");
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

fn schema_version(connection: &Connection) -> i64 {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version")
}

fn schema_object_exists(connection: &Connection, kind: &str, name: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_schema WHERE type = ?1 AND name = ?2
             )",
            [kind, name],
            |row| row.get(0),
        )
        .expect("schema object query")
}

fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
    let sql = format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid");
    let mut statement = connection.prepare(&sql).expect("table column query");
    statement
        .query_map([], |row| row.get(0))
        .expect("query table columns")
        .collect::<Result<_, _>>()
        .expect("read table columns")
}

#[cfg(unix)]
fn restrict_fixture_root(root: &TempDir) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private legacy Hub root");
}

#[cfg(not(unix))]
fn restrict_fixture_root(_root: &TempDir) {}
