use rusqlite::Connection;
use tempfile::TempDir;

use super::{schema::open_database, schema_sql::CREATE_V1_SCHEMA_SQL};

#[test]
fn v1_hub_data_is_preserved_by_atomic_v2_migration() {
    let (root, database) = legacy_database();
    let connection = open_database(&database).expect("v1 Hub migrates");
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version");
    let prompt: String = connection
        .query_row(
            "SELECT content FROM prompts WHERE id = 'prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("legacy Prompt survives");
    let run_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name='runs'",
            [],
            |row| row.get(0),
        )
        .expect("runs table query");
    let association_table: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema
             WHERE type='table' AND name='run_assistant_prompts'",
            [],
            |row| row.get(0),
        )
        .expect("assistant association table query");
    let durable_run_columns: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('runs')
             WHERE name IN ('execution_json','cursor_json','journal_bytes')",
            [],
            |row| row.get(0),
        )
        .expect("durable Run column query");

    assert_eq!(version, 2);
    assert_eq!(prompt, "preserve me");
    assert_eq!(run_table, 1);
    assert_eq!(association_table, 1);
    assert_eq!(durable_run_columns, 3);
    drop((connection, root));
}

#[test]
fn failed_v2_migration_rolls_back_without_changing_v1_data() {
    let (root, database) = legacy_database();
    let blocker = Connection::open(&database).expect("open migration blocker");
    blocker
        .execute_batch("CREATE TABLE runs(blocker TEXT)")
        .expect("install deterministic migration conflict");
    drop(blocker);

    open_database(&database).expect_err("conflicting v1 migration fails");
    let unchanged = Connection::open(&database).expect("reopen unchanged v1 database");
    let version: i64 = unchanged
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version");
    let prompt: String = unchanged
        .query_row(
            "SELECT content FROM prompts WHERE id = 'prompt-1'",
            [],
            |row| row.get(0),
        )
        .expect("legacy Prompt remains");
    let events_table: i64 = unchanged
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name='run_events'",
            [],
            |row| row.get(0),
        )
        .expect("run_events table query");

    assert_eq!(version, 1);
    assert_eq!(prompt, "preserve me");
    assert_eq!(events_table, 0);
    drop((unchanged, root));
}

fn legacy_database() -> (TempDir, std::path::PathBuf) {
    let root = TempDir::new().expect("legacy Hub root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let connection = Connection::open(&database).expect("create v1 database");
    connection
        .execute_batch(CREATE_V1_SCHEMA_SQL)
        .expect("create legacy schema");
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
    drop(connection);
    (root, database)
}

#[cfg(unix)]
fn restrict_fixture_root(root: &TempDir) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private legacy Hub root");
}

#[cfg(not(unix))]
fn restrict_fixture_root(_root: &TempDir) {}
