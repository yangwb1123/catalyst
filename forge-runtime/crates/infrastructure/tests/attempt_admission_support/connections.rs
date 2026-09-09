use forge_runtime_infrastructure::sqlite_execution::{AttemptJournalError, SqliteAttemptJournal};
use rusqlite::{Connection, OpenFlags};

use super::{counts, database, event, request};

#[test]
fn in_memory_temporary_attached_and_active_transaction_connections_are_rejected() {
    assert_rejection(
        Connection::open_in_memory().unwrap(),
        AttemptJournalError::Invalid,
    );
    assert_rejection(Connection::open("").unwrap(), AttemptJournalError::Invalid);
    let directory = tempfile::tempdir().unwrap();
    let attached = Connection::open(directory.path().join("attached.sqlite")).unwrap();
    attached
        .execute_batch("ATTACH DATABASE ':memory:' AS attached")
        .unwrap();
    assert_rejection(attached, AttemptJournalError::Invalid);
    let active = Connection::open(directory.path().join("active.sqlite")).unwrap();
    active.execute_batch("BEGIN IMMEDIATE").unwrap();
    assert_rejection(active, AttemptJournalError::Invalid);
}

#[test]
fn foreign_tables_and_unknown_identity_profiles_are_rejected_without_repair() {
    let directory = tempfile::tempdir().unwrap();
    for (index, sql) in [
        "CREATE TABLE existing_owner (id INTEGER PRIMARY KEY)",
        "PRAGMA application_id=42",
        "PRAGMA user_version=999",
    ]
    .iter()
    .enumerate()
    {
        let path = directory.path().join(format!("foreign-{index}.sqlite"));
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(sql).unwrap();
        assert_rejection(connection, AttemptJournalError::Corrupt);
        let check = Connection::open(&path).unwrap();
        let created: i64 = check
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'attempt_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(created, 0);
    }
}

#[test]
fn a_read_only_empty_database_cannot_be_initialized() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("readonly.sqlite");
    drop(Connection::open(&path).unwrap());
    let connection = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    assert_rejection(connection, AttemptJournalError::Unavailable);
    let tables: i64 = Connection::open(&path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM sqlite_schema", [], |row| row.get(0))
        .unwrap();
    assert_eq!(tables, 0);
}

fn assert_rejection(connection: Connection, error: AttemptJournalError) {
    assert_eq!(
        SqliteAttemptJournal::from_connection(connection).unwrap_err(),
        error
    );
}

#[test]
fn initialization_replaces_relaxed_connection_settings_with_the_durable_profile() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("settings.sqlite");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch("PRAGMA synchronous=OFF; PRAGMA foreign_keys=OFF")
        .unwrap();
    let mut journal = SqliteAttemptJournal::from_connection(connection).unwrap();
    let request = request(1);
    journal.admit(&request, &event(&request, 1)).unwrap();
    drop(journal);
    let check = Connection::open(&path).unwrap();
    let mode: String = check
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    assert_eq!(counts(&path), (1, 1, 1));
}

#[test]
fn competing_transaction_returns_unavailable_and_a_later_exact_retry_succeeds() {
    let (_directory, path, mut journal) = database();
    let blocker = Connection::open(&path).unwrap();
    blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
    let request = request(1);
    let event = event(&request, 1);
    assert_eq!(
        journal.admit(&request, &event),
        Err(AttemptJournalError::Unavailable)
    );
    blocker.execute_batch("ROLLBACK").unwrap();
    assert_eq!(counts(&path), (0, 0, 0));
    assert_eq!(journal.admit(&request, &event).unwrap().admission.cursor, 1);
    assert_eq!(counts(&path), (1, 1, 1));
}
