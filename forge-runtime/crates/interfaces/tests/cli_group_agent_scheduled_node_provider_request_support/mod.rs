use std::{collections::BTreeMap, path::Path, process::Output};

use rusqlite::{Connection, types::Value as SqlValue};
use tempfile::TempDir;

use super::group_agent_graph_run_support::invoke_with_stdin;

pub(super) fn stored_body(state: &Path, request_id: &str) -> Vec<u8> {
    Connection::open(state.join("hub.sqlite3"))
        .expect("open Hub")
        .query_row(
            "SELECT provider_request_blob
             FROM group_agent_graph_scheduled_node_provider_requests
             WHERE id=?1",
            [request_id],
            |row| row.get(0),
        )
        .expect("stored exact provider request body")
}

pub(super) fn downgrade_empty_v15_sidecar(database: &Path) {
    let connection = Connection::open(database).expect("open current Hub for v14 fixture");
    connection
        .execute_batch(
            "DROP TABLE group_agent_graph_scheduled_node_provider_requests;
             PRAGMA user_version = 14;
             PRAGMA wal_checkpoint(TRUNCATE);",
        )
        .expect("construct exact v14 fixture");
    assert!(!provider_request_table_exists(&connection));
}

pub(super) fn provider_request_table_exists(connection: &Connection) -> bool {
    connection
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM sqlite_schema
               WHERE type='table'
                 AND name='group_agent_graph_scheduled_node_provider_requests'
             )",
            [],
            |row| row.get(0),
        )
        .expect("provider request table presence")
}

pub(super) fn reject_pre_hub(args: &[&str], expected: &str) {
    let state = TempDir::new().expect("isolated state");
    let cwd = TempDir::new().expect("isolated cwd");
    let output = invoke_with_stdin(state.path(), cwd.path(), args, &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(expected), "stderr was {stderr:?}");
    assert!(!state.path().join("hub.sqlite3").exists());
}

pub(super) fn assert_cli_conflict(output: &Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("conflict"), "stderr was {stderr:?}");
}

pub(super) fn all_hub_state(state: &Path) -> BTreeMap<String, Vec<Vec<SqlValue>>> {
    let connection = Connection::open(state.join("hub.sqlite3")).expect("open Hub");
    table_names(&connection)
        .into_iter()
        .map(|table| {
            let rows = snapshot_table(&connection, &table);
            (table, rows)
        })
        .collect()
}

fn table_names(connection: &Connection) -> Vec<String> {
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare table inventory");
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query table inventory")
        .collect::<Result<_, _>>()
        .expect("collect table inventory")
}

fn snapshot_table(connection: &Connection, table: &str) -> Vec<Vec<SqlValue>> {
    let quoted = format!("\"{}\"", table.replace('"', "\"\""));
    let columns = connection
        .prepare(&format!("SELECT * FROM {quoted} LIMIT 0"))
        .expect("prepare table shape")
        .column_count();
    let order = (1..=columns)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    query_rows(
        connection,
        &format!("SELECT * FROM {quoted} ORDER BY {order}"),
    )
}

fn query_rows(connection: &Connection, sql: &str) -> Vec<Vec<SqlValue>> {
    let mut statement = connection.prepare(sql).expect("prepare snapshot query");
    let columns = statement.column_count();
    statement
        .query_map([], |row| {
            (0..columns)
                .map(|index| row.get(index))
                .collect::<Result<_, _>>()
        })
        .expect("query snapshot")
        .collect::<Result<_, _>>()
        .expect("collect snapshot")
}
