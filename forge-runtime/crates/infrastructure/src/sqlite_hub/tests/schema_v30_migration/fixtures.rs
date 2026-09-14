use std::path::PathBuf;

use rusqlite::Connection;
use tempfile::TempDir;

use super::super::{
    DROP_V30_CONVERSATION_CHANGES_SQL, DROP_V31_CONVERSATION_OWNERS_SQL,
    DROP_V32_OWNER_CURSOR_OBJECTS_SQL, DROP_V33_PROJECT_CONSENT_OBJECTS_SQL,
    DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL, open_database, restrict_fixture_root, schema_version,
};

type ProjectRow = (String, String, String, i64);
type ConversationRow = (String, String, Option<String>, String, String, i64, i64);
type PromptRow = (String, String, String, String, String, i64);
type RunRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    String,
    i64,
    i64,
);
type RunEventRow = (String, i64, String);

#[derive(Debug, PartialEq, Eq)]
pub(super) struct LegacyRows {
    projects: Vec<ProjectRow>,
    conversations: Vec<ConversationRow>,
    prompts: Vec<PromptRow>,
    runs: Vec<RunRow>,
    run_events: Vec<RunEventRow>,
}

pub(super) fn exact_v29_fixture() -> (TempDir, PathBuf) {
    let root = TempDir::new().expect("v29 fixture root");
    restrict_fixture_root(&root);
    let database = root.path().join("hub.sqlite3");
    let connection = open_database(&database).expect("create current fixture");
    seed_v29_rows(&connection);
    connection
        .execute_batch(DROP_V34_PENDING_RUN_INTENT_OBJECTS_SQL)
        .expect("remove v34 pending-intent additions");
    connection
        .execute_batch(DROP_V33_PROJECT_CONSENT_OBJECTS_SQL)
        .expect("remove v33 Project consent additions");
    connection
        .execute_batch(DROP_V32_OWNER_CURSOR_OBJECTS_SQL)
        .expect("remove v32 owner-local cursor additions");
    connection
        .execute_batch(DROP_V30_CONVERSATION_CHANGES_SQL)
        .expect("restore exact v29 schema");
    connection
        .execute_batch(DROP_V31_CONVERSATION_OWNERS_SQL)
        .expect("remove v31 owners when restoring exact v29 schema");
    connection
        .execute_batch("PRAGMA user_version=29")
        .expect("stamp exact v29 fixture");
    assert_eq!(schema_version(&connection), 29);
    drop(connection);
    (root, database)
}

fn seed_v29_rows(connection: &Connection) {
    connection
        .execute_batch(
            "INSERT INTO projects VALUES('project-v30','Fixture','/fixture/v30',1);
             INSERT INTO conversations VALUES(
               'conversation-v30','global',NULL,'Fixture','conversation-v30-key',1,2
             );
             INSERT INTO prompts VALUES(
               'prompt-v30','conversation-v30','user','preserve me','prompt-v30-key',2
             );
             INSERT INTO runs VALUES(
               'run-v30','conversation-v30','prompt-v30','project-v30','{}','{}',2,
               'run-v30-key',1,3
             );
             INSERT INTO run_events VALUES('run-v30',1,'event-v30');",
        )
        .expect("seed populated v29 Hub rows");
}

pub(super) fn legacy_rows(connection: &Connection) -> LegacyRows {
    LegacyRows {
        projects: project_rows(connection),
        conversations: conversation_rows(connection),
        prompts: prompt_rows(connection),
        runs: run_rows(connection),
        run_events: run_event_rows(connection),
    }
}

fn project_rows(connection: &Connection) -> Vec<ProjectRow> {
    query_rows(
        connection,
        "SELECT id,name,canonical_path,created_at_ms FROM projects ORDER BY id",
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
}

fn conversation_rows(connection: &Connection) -> Vec<ConversationRow> {
    query_rows(
        connection,
        "SELECT id,scope_kind,scope_id,title,idempotency_key,created_at_ms,updated_at_ms
         FROM conversations ORDER BY id",
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
            ))
        },
    )
}

fn prompt_rows(connection: &Connection) -> Vec<PromptRow> {
    query_rows(
        connection,
        "SELECT id,conversation_id,role,content,idempotency_key,created_at_ms
         FROM prompts ORDER BY id",
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
}

fn run_rows(connection: &Connection) -> Vec<RunRow> {
    query_rows(
        connection,
        "SELECT id,conversation_id,prompt_id,project_id,execution_json,cursor_json,
                journal_bytes,idempotency_key,protocol_version,created_at_ms
         FROM runs ORDER BY id",
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
            ))
        },
    )
}

fn run_event_rows(connection: &Connection) -> Vec<RunEventRow> {
    query_rows(
        connection,
        "SELECT run_id,seq,event_json FROM run_events ORDER BY run_id,seq",
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
}

fn query_rows<T>(
    connection: &Connection,
    sql: &str,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Vec<T> {
    let mut statement = connection
        .prepare(sql)
        .expect("prepare legacy row snapshot");
    statement
        .query_map([], map)
        .expect("query legacy row snapshot")
        .collect::<Result<_, _>>()
        .expect("read legacy row snapshot")
}

pub(super) fn change_count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM conversation_changes", [], |row| {
            row.get(0)
        })
        .expect("query Conversation change row count")
}

pub(super) fn baseline_count(connection: &Connection) -> i64 {
    connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_change_baselines",
            [],
            |row| row.get(0),
        )
        .expect("query Conversation change baseline count")
}

pub(super) fn change_cursor_head(connection: &Connection) -> i64 {
    connection
        .query_row(
            "SELECT last_cursor FROM conversation_change_state WHERE state_id = 1",
            [],
            |row| row.get(0),
        )
        .expect("query durable Hub cursor head")
}

pub(super) fn conversation_version_head(connection: &Connection, conversation_id: &str) -> i64 {
    connection
        .query_row(
            "SELECT last_version FROM conversation_change_heads WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("query durable Conversation version head")
}

pub(super) fn has_baseline(connection: &Connection, conversation_id: &str) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_change_baselines WHERE conversation_id = ?1
             )",
            [conversation_id],
            |row| row.get(0),
        )
        .expect("query Conversation change baseline")
}
