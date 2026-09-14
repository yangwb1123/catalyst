use rusqlite::{Connection, Error as SqliteError, ErrorCode, OpenFlags};
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};
use url::Url;

use super::{
    CONNECTION_BUSY_TIMEOUT, HubStoreError, OpenAttemptError, SCHEMA_VERSION, contract, location,
    migrate_or_validate, read_only_schema_required, schema_version, unsupported_schema,
};

const OPEN_RETRY_TIMEOUT: Duration = Duration::from_secs(15);
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(10);
const OPEN_RETRY_MAX_DELAY: Duration = Duration::from_millis(100);

pub(in crate::sqlite_hub) fn open_database(path: &Path) -> Result<Connection, HubStoreError> {
    let deadline = Instant::now() + OPEN_RETRY_TIMEOUT;
    let mut retry_delay = OPEN_RETRY_DELAY;
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
                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(retry_delay.min(remaining));
                if Instant::now() >= deadline {
                    return Err(contract::sqlite_error(error));
                }
                retry_delay = retry_delay.saturating_mul(2).min(OPEN_RETRY_MAX_DELAY);
            }
            Err(OpenAttemptError::Sqlite(error)) => return Err(contract::sqlite_error(error)),
            Err(OpenAttemptError::Store(error)) => return Err(error),
        }
    }
}
pub(in crate::sqlite_hub) fn open_existing_current_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    open_existing_validated_read_only_database(path, &[SCHEMA_VERSION], "current schema version 34")
}
pub(in crate::sqlite_hub) fn open_existing_dispatch_preflight_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    open_existing_validated_read_only_database(
        path,
        &[
            11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
        ],
        "schema version 11..=32",
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
    contract::validate_migration_source(&connection, version)?;
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
fn open_database_once(path: &Path) -> Result<Connection, OpenAttemptError> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(CONNECTION_BUSY_TIMEOUT)?;
    reject_unsupported_schema(&connection)?;
    let version = schema_version(&connection)?;
    if version > 0 && version < SCHEMA_VERSION {
        // Production-readiness condition: a migration is irreversible, so
        // snapshot an EXISTING hub before the first upgrade opens it
        // (review stage-06 High). Fresh hubs (version 0) need no backup.
        location::backup_before_upgrade(&connection, path, version)?;
    }
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
