use rusqlite::Connection;
use tempfile::TempDir;

pub(super) fn schema_version(connection: &Connection) -> i64 {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .expect("schema version")
}

pub(super) fn schema_object_exists(connection: &Connection, kind: &str, name: &str) -> bool {
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

pub(super) fn table_columns(connection: &Connection, table: &str) -> Vec<String> {
    let sql = format!("SELECT name FROM pragma_table_info('{table}') ORDER BY cid");
    let mut statement = connection.prepare(&sql).expect("table column query");
    statement
        .query_map([], |row| row.get(0))
        .expect("query table columns")
        .collect::<Result<_, _>>()
        .expect("read table columns")
}

#[cfg(unix)]
pub(super) fn restrict_fixture_root(root: &TempDir) {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
        .expect("private legacy Hub root");
}

#[cfg(not(unix))]
pub(super) fn restrict_fixture_root(_root: &TempDir) {}
