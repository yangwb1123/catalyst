use std::{path::Path, process::Output};

use rusqlite::Connection;
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
