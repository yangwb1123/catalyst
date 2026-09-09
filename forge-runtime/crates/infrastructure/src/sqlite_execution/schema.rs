use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use super::{AttemptJournalError, read, schema_sql, sqlite_error};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn open(connection: &mut Connection) -> Result<(), AttemptJournalError> {
    if !connection.is_autocommit() {
        return Err(AttemptJournalError::Invalid);
    }
    connection_boundary(connection)?;
    connection
        .busy_timeout(BUSY_TIMEOUT)
        .map_err(sqlite_error)?;
    // Foreign profiles must be rejected before any journal-mode mutation.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(sqlite_error)?;
    if !empty(&transaction)? {
        validate_schema(&transaction)?;
        read::audit_records(&transaction)?;
    }
    transaction.commit().map_err(sqlite_error)?;
    configure(connection)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error)?;
    if empty(&transaction)? {
        for (_, sql) in schema_sql::TABLES {
            transaction.execute_batch(sql).map_err(sqlite_error)?;
        }
        transaction
            .pragma_update(None, "application_id", schema_sql::APPLICATION_ID)
            .map_err(sqlite_error)?;
        transaction
            .pragma_update(None, "user_version", 1_i64)
            .map_err(sqlite_error)?;
    }
    read::audit(&transaction)?;
    transaction.commit().map_err(sqlite_error)
}

fn configure(connection: &Connection) -> Result<(), AttemptJournalError> {
    connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
        PRAGMA ignore_check_constraints=OFF; PRAGMA writable_schema=OFF; PRAGMA trusted_schema=OFF;")
        .map_err(sqlite_error)?;
    validate_settings(connection)
}

pub(super) fn validate(connection: &Connection) -> Result<(), AttemptJournalError> {
    connection_boundary(connection).map_err(|_| AttemptJournalError::Corrupt)?;
    validate_settings(connection)?;
    validate_schema(connection)
}

fn connection_boundary(connection: &Connection) -> Result<(), AttemptJournalError> {
    let mut statement = connection
        .prepare("PRAGMA database_list")
        .map_err(sqlite_error)?;
    let mut rows = statement.query([]).map_err(sqlite_error)?;
    let row = rows
        .next()
        .map_err(sqlite_error)?
        .ok_or(AttemptJournalError::Invalid)?;
    let name: String = row.get(1).map_err(sqlite_error)?;
    let filename: String = row.get(2).map_err(sqlite_error)?;
    if name != "main" || filename.is_empty() || rows.next().map_err(sqlite_error)?.is_some() {
        return Err(AttemptJournalError::Invalid);
    }
    Ok(())
}

fn validate_settings(connection: &Connection) -> Result<(), AttemptJournalError> {
    let mode: String = connection
        .pragma_query_value(None, "journal_mode", |row| row.get(0))
        .map_err(sqlite_error)?;
    if mode != "wal"
        || integer(connection, "synchronous")? != 2
        || integer(connection, "foreign_keys")? != 1
        || integer(connection, "ignore_check_constraints")? != 0
        || integer(connection, "writable_schema")? != 0
        || integer(connection, "trusted_schema")? != 0
        || integer(connection, "busy_timeout")? != 5_000
    {
        return Err(AttemptJournalError::Corrupt);
    }
    Ok(())
}

fn empty(connection: &Connection) -> Result<bool, AttemptJournalError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM (SELECT 1 FROM sqlite_schema LIMIT 1)",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite_error)?;
    Ok(count == 0
        && integer(connection, "application_id")? == 0
        && integer(connection, "user_version")? == 0)
}

fn validate_schema(connection: &Connection) -> Result<(), AttemptJournalError> {
    if integer(connection, "application_id")? != schema_sql::APPLICATION_ID
        || integer(connection, "user_version")? != 1
    {
        return Err(AttemptJournalError::Corrupt);
    }
    let mut statement = connection
        .prepare(
            "SELECT substr(type,1,16),substr(name,1,128),substr(tbl_name,1,128),substr(sql,1,2049)
         FROM sqlite_schema LIMIT 9",
        )
        .map_err(sqlite_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SchemaObject {
                kind: row.get(0)?,
                name: row.get(1)?,
                table: row.get(2)?,
                sql: row.get(3)?,
            })
        })
        .map_err(sqlite_error)?;
    let objects = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| AttemptJournalError::Corrupt)?;
    if objects.len() != schema_sql::TABLES.len() + schema_sql::INDEXES.len()
        || objects.iter().any(|object| {
            !expected_object(object)
                || objects
                    .iter()
                    .filter(|other| other.name == object.name)
                    .count()
                    != 1
        })
    {
        return Err(AttemptJournalError::Corrupt);
    }
    Ok(())
}

fn expected_object(object: &SchemaObject) -> bool {
    schema_sql::TABLES.iter().any(|(name, sql)| {
        object.kind == "table"
            && object.name == *name
            && object.table == *name
            && object.sql.as_deref() == Some(*sql)
    }) || schema_sql::INDEXES.iter().any(|(name, table)| {
        object.kind == "index"
            && object.name == *name
            && object.table == *table
            && object.sql.is_none()
    })
}

fn integer(connection: &Connection, pragma: &str) -> Result<i64, AttemptJournalError> {
    connection
        .pragma_query_value(None, pragma, |row| row.get(0))
        .map_err(sqlite_error)
}

struct SchemaObject {
    kind: String,
    name: String,
    table: String,
    sql: Option<String>,
}
