use std::path::PathBuf;

use rusqlite::{Connection, params};
use tempfile::TempDir;

use super::{AttemptJournalError, SqliteAttemptJournal, crash_fixture};

#[test]
fn every_operation_and_reopen_rejects_relational_or_redundant_column_drift() {
    for sql in [
        "UPDATE attempt_admissions SET attempt_id='atm_00000000000000000000000002'",
        "UPDATE attempt_admissions SET idempotency_key='different-key'",
        "UPDATE attempt_admissions SET request_sha256=lower(hex(zeroblob(32)))",
        "UPDATE attempt_events SET event_id='evt_00000000000000000000000002'",
        "UPDATE attempt_events SET message_id='msg_00000000000000000000000002'",
        "UPDATE attempt_events SET event_sha256=lower(hex(zeroblob(32)))",
        "UPDATE attempt_outbox SET event_id='evt_00000000000000000000000002'",
        "DELETE FROM attempt_outbox",
        "DELETE FROM attempt_events",
        "DELETE FROM attempt_admissions",
        "UPDATE attempt_admissions SET cursor=2; UPDATE attempt_events SET cursor=2; UPDATE attempt_outbox SET cursor=2",
    ] {
        let (_directory, path, mut journal) = stored();
        let external = Connection::open(&path).unwrap();
        external.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        external.execute_batch(sql).unwrap();
        assert_rejected(&mut journal);
        drop(journal);
        assert_eq!(
            SqliteAttemptJournal::from_connection(Connection::open(&path).unwrap()).unwrap_err(),
            AttemptJournalError::Corrupt,
            "{sql}"
        );
    }
}

#[test]
fn schema_and_profile_drift_is_rejected_without_repair() {
    for sql in [
        "PRAGMA application_id=0",
        "PRAGMA user_version=2",
        "CREATE TABLE rogue(value TEXT)",
        "CREATE VIEW rogue AS SELECT 1",
        "CREATE INDEX rogue ON attempt_events(message_id)",
        "CREATE TRIGGER rogue AFTER INSERT ON attempt_outbox BEGIN SELECT 1; END",
        "DROP TABLE attempt_outbox",
        "ALTER TABLE attempt_admissions ADD COLUMN extra TEXT",
    ] {
        let (_directory, path, mut journal) = stored();
        let external = Connection::open(&path).unwrap();
        external.execute_batch(sql).unwrap();
        let before = schema_snapshot(&external);
        assert_rejected(&mut journal);
        assert_eq!(schema_snapshot(&external), before, "{sql}");
    }
}

#[test]
fn request_and_event_bytes_cannot_drift_even_when_json_remains_valid() {
    for (table, column, transform) in [
        ("attempt_admissions", "request_json", "space"),
        ("attempt_admissions", "request_json", "version"),
        ("attempt_admissions", "request_json", "missing"),
        ("attempt_events", "event_json", "space"),
        ("attempt_events", "event_json", "state"),
        ("attempt_events", "event_json", "digest"),
    ] {
        let (_directory, path, mut journal) = stored();
        let external = Connection::open(&path).unwrap();
        let old: String = external
            .query_row(&format!("SELECT {column} FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        let changed = changed_json(&old, transform);
        assert_ne!(old, changed);
        external
            .execute(&format!("UPDATE {table} SET {column}=?1"), [changed])
            .unwrap();
        assert_rejected(&mut journal);
    }
}

#[test]
fn oversized_canonical_and_redundant_columns_fail_before_loading_records() {
    for (table, column, bytes) in [
        ("attempt_admissions", "request_json", 65_537),
        ("attempt_events", "event_json", 16_385),
        ("attempt_admissions", "attempt_id", 1_000_000),
        ("attempt_admissions", "idempotency_key", 1_000_000),
        ("attempt_admissions", "request_sha256", 1_000_000),
        ("attempt_events", "message_id", 1_000_000),
        ("attempt_outbox", "event_id", 1_000_000),
    ] {
        let (_directory, path, mut journal) = stored();
        let external = Connection::open(&path).unwrap();
        external
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON")
            .unwrap();
        external
            .execute(
                &format!("UPDATE {table} SET {column}=?1"),
                params!["x".repeat(bytes)],
            )
            .unwrap();
        assert_rejected(&mut journal);
    }
}

#[test]
fn connection_settings_are_rechecked_for_every_operation() {
    for sql in [
        "PRAGMA foreign_keys=OFF",
        "PRAGMA synchronous=NORMAL",
        "PRAGMA busy_timeout=1",
        "PRAGMA ignore_check_constraints=ON",
        "PRAGMA writable_schema=ON",
        "PRAGMA trusted_schema=ON",
        "ATTACH ':memory:' AS extra",
    ] {
        let (_directory, _path, mut journal) = stored();
        journal.connection.execute_batch(sql).unwrap();
        assert_rejected(&mut journal);
    }
}

#[test]
fn foreign_profile_is_rejected_before_switching_its_journal_mode() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("foreign.sqlite");
    let foreign = Connection::open(&path).unwrap();
    foreign
        .execute_batch(
            "CREATE TABLE foreign_data(value TEXT); INSERT INTO foreign_data VALUES('retained')",
        )
        .unwrap();
    let before = schema_snapshot(&foreign);
    assert_eq!(
        SqliteAttemptJournal::from_connection(Connection::open(&path).unwrap()).unwrap_err(),
        AttemptJournalError::Corrupt
    );
    assert_eq!(schema_snapshot(&foreign), before);
    let mode: String = foreign
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "delete");
    let data: String = foreign
        .query_row("SELECT value FROM foreign_data", [], |row| row.get(0))
        .unwrap();
    assert_eq!(data, "retained");
}

fn assert_rejected(journal: &mut SqliteAttemptJournal) {
    let request = crash_fixture::request();
    assert_eq!(
        journal.get(&request.attempt_ref().entity_id),
        Err(AttemptJournalError::Corrupt)
    );
    assert_eq!(journal.pending(0, 1), Err(AttemptJournalError::Corrupt));
    assert_eq!(
        journal.admit(&request, &crash_fixture::event(&request)),
        Err(AttemptJournalError::Corrupt)
    );
}

fn stored() -> (TempDir, PathBuf, SqliteAttemptJournal) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("attempt.sqlite");
    let mut journal =
        SqliteAttemptJournal::from_connection(Connection::open(&path).unwrap()).unwrap();
    let request = crash_fixture::request();
    journal
        .admit(&request, &crash_fixture::event(&request))
        .unwrap();
    (directory, path, journal)
}

fn changed_json(old: &str, transform: &str) -> String {
    if transform == "space" {
        return format!("{old} ");
    }
    let mut value: serde_json::Value = serde_json::from_str(old).unwrap();
    match transform {
        "version" => value["version"] = serde_json::json!(2),
        "missing" => {
            value.as_object_mut().unwrap().remove("grant_ref");
        }
        "state" => value["payload"]["state"] = serde_json::json!("accepted"),
        "digest" => value["payload"]["request_sha256"] = serde_json::json!("0".repeat(64)),
        _ => panic!("unknown mutation"),
    }
    serde_json::to_string(&value).unwrap()
}

fn schema_snapshot(connection: &Connection) -> Vec<(String, Option<String>)> {
    connection
        .prepare("SELECT name,sql FROM sqlite_schema ORDER BY name")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
}
