use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

use rusqlite::{Connection, Error as SqliteError, ErrorCode, OpenFlags};
use url::Url;

#[path = "schema_contract.rs"]
mod contract;
#[path = "schema_location.rs"]
mod location;
#[path = "schema_contract/reentry.rs"]
mod reentry;

pub(super) fn open_existing_dispatch_reentry_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    reentry::open_existing_dispatch_reentry_read_only_database(path)
}

use super::{
    HubStoreError,
    schema_sql::{
        CREATE_V1_SCHEMA_SQL, MIGRATE_V1_TO_V2_SQL, MIGRATE_V2_TO_V3_SQL, MIGRATE_V3_TO_V4_SQL,
        MIGRATE_V4_TO_V5_SQL, MIGRATE_V5_TO_V6_SQL, MIGRATE_V6_TO_V7_SQL, MIGRATE_V7_TO_V8_SQL,
    },
    schema_v9_sql::MIGRATE_V8_TO_V9_SQL,
    schema_v10_sql::MIGRATE_V9_TO_V10_SQL,
    schema_v11_sql::MIGRATE_V10_TO_V11_SQL,
    schema_v12_sql::MIGRATE_V11_TO_V12_SQL,
    schema_v13_sql::MIGRATE_V12_TO_V13_SQL,
    unavailable,
};

const SCHEMA_VERSION: i64 = 13;
const CONNECTION_BUSY_TIMEOUT: Duration = Duration::from_millis(250);
const OPEN_RETRY_TIMEOUT: Duration = Duration::from_secs(5);
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(10);

pub(super) fn sqlite_error(error: SqliteError) -> HubStoreError {
    contract::sqlite_error(error)
}

pub(super) fn open_database(path: &Path) -> Result<Connection, HubStoreError> {
    let deadline = Instant::now() + OPEN_RETRY_TIMEOUT;
    loop {
        location::prepare(path)?;
        match open_database_once(path) {
            Ok(connection) => {
                location::restrict(path)?;
                return Ok(connection);
            }
            Err(OpenAttemptError::Sqlite(error))
                if is_lock_contention(&error) && Instant::now() < deadline =>
            {
                thread::sleep(OPEN_RETRY_DELAY);
            }
            Err(OpenAttemptError::Sqlite(error)) => return Err(contract::sqlite_error(error)),
            Err(OpenAttemptError::Store(error)) => return Err(error),
        }
    }
}

pub(super) fn open_existing_current_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    open_existing_validated_read_only_database(path, &[SCHEMA_VERSION], "current schema version 13")
}

pub(super) fn open_existing_dispatch_preflight_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    open_existing_validated_read_only_database(
        path,
        &[11, 12, SCHEMA_VERSION],
        "schema version 11, 12, or 13",
    )
}

fn open_existing_validated_read_only_database(
    path: &Path,
    accepted_versions: &[i64],
    requirement: &str,
) -> Result<Connection, HubStoreError> {
    let before = location::inspect_existing_read_only(path)?;
    let uri = immutable_file_uri(before.canonical_path())?;
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let connection =
        Connection::open_with_flags(uri.as_str(), flags).map_err(contract::sqlite_error)?;
    connection
        .busy_timeout(CONNECTION_BUSY_TIMEOUT)
        .map_err(contract::sqlite_error)?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(contract::sqlite_error)?;
    let version = schema_version(&connection).map_err(contract::sqlite_error)?;
    if !accepted_versions.contains(&version) {
        return Err(read_only_schema_required(version, requirement));
    }
    contract::validate_version(&connection, version)?;
    let after = location::inspect_existing_read_only(path)?;
    if before != after {
        return Err(HubStoreError::Unavailable {
            message: "Hub database changed during effect-free read-only open".into(),
        });
    }
    Ok(connection)
}

fn immutable_file_uri(path: &Path) -> Result<Url, HubStoreError> {
    let mut uri = Url::from_file_path(path).map_err(|()| HubStoreError::Unavailable {
        message: format!(
            "Hub database path cannot be represented as a file URI: {}",
            path.display()
        ),
    })?;
    uri.query_pairs_mut()
        .append_pair("mode", "ro")
        .append_pair("immutable", "1");
    Ok(uri)
}

fn read_only_schema_required(version: i64, requirement: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: format!("effect-free Hub reads require {requirement}; found {version}"),
    }
}

fn open_database_once(path: &Path) -> Result<Connection, OpenAttemptError> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(CONNECTION_BUSY_TIMEOUT)?;
    reject_unsupported_schema(&connection)?;
    configure(&connection)?;
    migrate_or_validate(&connection)?;
    Ok(connection)
}

fn configure(connection: &Connection) -> Result<(), SqliteError> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;
             PRAGMA synchronous = FULL;
             PRAGMA secure_delete = ON;",
    )
}

fn reject_unsupported_schema(connection: &Connection) -> Result<(), OpenAttemptError> {
    let version = schema_version(connection)?;
    if (0..=SCHEMA_VERSION).contains(&version) {
        return Ok(());
    }
    Err(unsupported_schema(version))
}

fn is_lock_contention(error: &SqliteError) -> bool {
    matches!(
        error,
        SqliteError::SqliteFailure(problem, _)
            if matches!(
                problem.code,
                ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked
            )
    )
}

fn migrate_or_validate(connection: &Connection) -> Result<(), OpenAttemptError> {
    migrate_or_validate_with_before_final(connection, |_| Ok(()))
}

fn migrate_or_validate_with_before_final(
    connection: &Connection,
    before_final: impl FnOnce(&Connection) -> Result<(), OpenAttemptError>,
) -> Result<(), OpenAttemptError> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = validate_locked_schema(connection, before_final);
    match result {
        Ok(()) => {
            connection.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(error) => {
            let _ = connection.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

fn validate_locked_schema(
    connection: &Connection,
    before_final: impl FnOnce(&Connection) -> Result<(), OpenAttemptError>,
) -> Result<(), OpenAttemptError> {
    let version = schema_version(connection)?;
    contract::validate_version(connection, version)?;
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    migrate_to_current(connection, version)?;
    before_final(connection)?;
    contract::validate_version(connection, SCHEMA_VERSION).map_err(Into::into)
}

fn migrate_to_current(connection: &Connection, version: i64) -> Result<(), OpenAttemptError> {
    if version == 0 {
        create_v1_schema(connection)?;
    }
    if version <= 1 {
        migrate_v1_to_v2(connection)?;
    }
    if version <= 2 {
        migrate_v2_to_v3(connection)?;
    }
    if version <= 3 {
        migrate_v3_to_v4(connection)?;
    }
    if version <= 4 {
        migrate_v4_to_v5(connection)?;
    }
    if version <= 5 {
        migrate_v5_to_v6(connection)?;
    }
    if version <= 6 {
        migrate_v6_to_v7(connection)?;
    }
    if version <= 7 {
        migrate_v7_to_v8(connection)?;
    }
    if version <= 8 {
        migrate_v8_to_v9(connection)?;
    }
    if version <= 9 {
        migrate_v9_to_v10(connection)?;
    }
    if version <= 10 {
        migrate_v10_to_v11(connection)?;
    }
    if version <= 11 {
        migrate_v11_to_v12(connection)?;
    }
    if version <= 12 {
        migrate_v12_to_v13(connection)?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn migrate_with_before_final_fault_for_test(
    connection: &Connection,
    fault: impl FnOnce(&Connection) -> Result<(), SqliteError>,
) -> Result<(), HubStoreError> {
    migrate_or_validate_with_before_final(connection, |connection| {
        fault(connection).map_err(Into::into)
    })
    .map_err(|error| match error {
        OpenAttemptError::Sqlite(error) => contract::sqlite_error(error),
        OpenAttemptError::Store(error) => error,
    })
}

fn schema_version(connection: &Connection) -> Result<i64, SqliteError> {
    connection.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn unsupported_schema(version: i64) -> OpenAttemptError {
    HubStoreError::Corrupt {
        message: format!("unsupported Hub schema version {version}; expected {SCHEMA_VERSION}"),
    }
    .into()
}

fn create_v1_schema(connection: &Connection) -> Result<(), SqliteError> {
    connection.execute_batch(CREATE_V1_SCHEMA_SQL)
}

fn migrate_v1_to_v2(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V1_TO_V2_SQL)?;
    Ok(())
}

fn migrate_v2_to_v3(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V2_TO_V3_SQL)?;
    Ok(())
}

fn migrate_v3_to_v4(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V3_TO_V4_SQL)?;
    Ok(())
}

fn migrate_v4_to_v5(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V4_TO_V5_SQL)?;
    Ok(())
}

fn migrate_v5_to_v6(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V5_TO_V6_SQL)?;
    Ok(())
}

fn migrate_v6_to_v7(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V6_TO_V7_SQL)?;
    Ok(())
}

fn migrate_v7_to_v8(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V7_TO_V8_SQL)?;
    Ok(())
}

fn migrate_v8_to_v9(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V8_TO_V9_SQL)?;
    Ok(())
}

fn migrate_v9_to_v10(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V9_TO_V10_SQL)?;
    Ok(())
}

fn migrate_v10_to_v11(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V10_TO_V11_SQL)?;
    Ok(())
}

fn migrate_v11_to_v12(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V11_TO_V12_SQL)?;
    Ok(())
}

fn migrate_v12_to_v13(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V12_TO_V13_SQL)?;
    Ok(())
}

enum OpenAttemptError {
    Sqlite(SqliteError),
    Store(HubStoreError),
}

impl From<SqliteError> for OpenAttemptError {
    fn from(error: SqliteError) -> Self {
        Self::Sqlite(error)
    }
}

impl From<HubStoreError> for OpenAttemptError {
    fn from(error: HubStoreError) -> Self {
        Self::Store(error)
    }
}

#[cfg(all(test, unix))]
use location::verify_private_directory_permissions;

#[cfg(all(test, unix))]
#[path = "tests/schema_permissions.rs"]
mod tests;
