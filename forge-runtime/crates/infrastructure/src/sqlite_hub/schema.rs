use rusqlite::{Connection, Error as SqliteError, ErrorCode, OpenFlags};
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};
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

pub(super) fn open_existing_current_live_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    reentry::open_existing_current_live_read_only_database(path)
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
    schema_v14_sql::MIGRATE_V13_TO_V14_SQL,
    schema_v15_sql::MIGRATE_V14_TO_V15_SQL,
    schema_v16_sql::MIGRATE_V15_TO_V16_SQL,
    schema_v17_sql::MIGRATE_V16_TO_V17_SQL,
    schema_v18_sql::MIGRATE_V17_TO_V18_SQL,
    schema_v19_sql::MIGRATE_V18_TO_V19_SQL,
    schema_v20_sql::MIGRATE_V19_TO_V20_SQL,
    schema_v21_sql::MIGRATE_V20_TO_V21_SQL,
    schema_v22_sql::MIGRATE_V21_TO_V22_SQL,
    schema_v23_sql::MIGRATE_V22_TO_V23_SQL,
    schema_v24_sql::MIGRATE_V23_TO_V24_SQL,
    schema_v25_sql::MIGRATE_V24_TO_V25_SQL,
    schema_v26_sql::MIGRATE_V25_TO_V26_SQL,
    schema_v27_sql::MIGRATE_V26_TO_V27_SQL,
    schema_v28_sql::MIGRATE_V27_TO_V28_SQL,
    schema_v29_sql::MIGRATE_V28_TO_V29_SQL,
    unavailable,
};

pub(super) const SCHEMA_VERSION: i64 = 29;
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
    open_existing_validated_read_only_database(path, &[SCHEMA_VERSION], "current schema version 29")
}
pub(super) fn open_existing_dispatch_preflight_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    open_existing_validated_read_only_database(
        path,
        &[
            11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
        ],
        "schema version 11..=29",
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
fn read_only_schema_required(version: i64, requirement: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: format!("effect-free Hub reads require {requirement}; found {version}"),
    }
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
fn migrate_or_validate(connection: &Connection) -> Result<(), OpenAttemptError> {
    migrate_or_validate_with_before_final(connection, |_| Ok(()))
}
fn migrate_or_validate_with_before_final(
    connection: &Connection,
    before_final: impl FnOnce(&Connection) -> Result<(), OpenAttemptError>,
) -> Result<(), OpenAttemptError> {
    connection.execute_batch("BEGIN IMMEDIATE")?;
    // Data-bearing rebuilds DROP parent tables whose children still carry
    // rows; under FK enforcement the implicit DELETE violates ON DELETE
    // RESTRICT mid-batch. Deferring re-checks every FK against the final
    // (consistent) state at COMMIT (Stage-02 Finding 1).
    connection.execute_batch("PRAGMA defer_foreign_keys = ON")?;
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
    let endpoint_only_v25 = contract::validate_migration_source(connection, version)?;
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    if endpoint_only_v25 {
        migrate_endpoint_only_v25_to_current(connection)?;
    } else {
        migrate_to_current(connection, version)?;
    }
    before_final(connection)?;
    contract::validate_version(connection, SCHEMA_VERSION).map_err(Into::into)
}

fn migrate_endpoint_only_v25_to_current(connection: &Connection) -> Result<(), OpenAttemptError> {
    connection.execute_batch(MIGRATE_V24_TO_V25_SQL)?;
    connection.execute_batch("PRAGMA user_version = 26")?;
    connection.execute_batch(MIGRATE_V26_TO_V27_SQL)?;
    super::governance_record_journal::semantic::rebuild_locked(connection)
        .map_err(OpenAttemptError::Store)?;
    connection.execute_batch(MIGRATE_V27_TO_V28_SQL)?;
    connection.execute_batch(MIGRATE_V28_TO_V29_SQL)?;
    Ok(())
}

fn migrate_to_current(connection: &Connection, version: i64) -> Result<(), OpenAttemptError> {
    create_if_empty(connection, version)?;
    migrate_early(connection, version)?;
    migrate_late(connection, version)?;
    migrate_latest(connection, version)
}

/// Upgrades legacy v1 through v8 sources to v9.
fn migrate_early(connection: &Connection, version: i64) -> Result<(), OpenAttemptError> {
    if version <= 1 {
        connection.execute_batch(MIGRATE_V1_TO_V2_SQL)?;
    }
    if version <= 2 {
        connection.execute_batch(MIGRATE_V2_TO_V3_SQL)?;
    }
    if version <= 3 {
        connection.execute_batch(MIGRATE_V3_TO_V4_SQL)?;
    }
    if version <= 4 {
        connection.execute_batch(MIGRATE_V4_TO_V5_SQL)?;
    }
    if version <= 5 {
        connection.execute_batch(MIGRATE_V5_TO_V6_SQL)?;
    }
    if version <= 6 {
        connection.execute_batch(MIGRATE_V6_TO_V7_SQL)?;
    }
    if version <= 7 {
        connection.execute_batch(MIGRATE_V7_TO_V8_SQL)?;
    }
    if version <= 8 {
        connection.execute_batch(MIGRATE_V8_TO_V9_SQL)?;
    }
    Ok(())
}

/// Upgrades Graph/Agent v9 through v16 sources to v17.
fn migrate_late(connection: &Connection, version: i64) -> Result<(), OpenAttemptError> {
    if version <= 9 {
        connection.execute_batch(MIGRATE_V9_TO_V10_SQL)?;
    }
    if version <= 10 {
        connection.execute_batch(MIGRATE_V10_TO_V11_SQL)?;
    }
    if version <= 11 {
        connection.execute_batch(MIGRATE_V11_TO_V12_SQL)?;
    }
    if version <= 12 {
        connection.execute_batch(MIGRATE_V12_TO_V13_SQL)?;
    }
    if version <= 13 {
        connection.execute_batch(MIGRATE_V13_TO_V14_SQL)?;
    }
    if version <= 14 {
        connection.execute_batch(MIGRATE_V14_TO_V15_SQL)?;
    }
    if version <= 15 {
        connection.execute_batch(MIGRATE_V15_TO_V16_SQL)?;
    }
    if version <= 16 {
        connection.execute_batch(MIGRATE_V16_TO_V17_SQL)?;
    }
    Ok(())
}

/// Upgrades execution/governance v17 through v28 sources to current v29.
fn migrate_latest(connection: &Connection, version: i64) -> Result<(), OpenAttemptError> {
    if version <= 17 {
        connection.execute_batch(MIGRATE_V17_TO_V18_SQL)?;
    }
    if version <= 18 {
        connection.execute_batch(MIGRATE_V18_TO_V19_SQL)?;
    }
    if version <= 19 {
        connection.execute_batch(MIGRATE_V19_TO_V20_SQL)?;
    }
    if version <= 20 {
        connection.execute_batch(MIGRATE_V20_TO_V21_SQL)?;
    }
    if version <= 21 {
        connection.execute_batch(MIGRATE_V21_TO_V22_SQL)?;
    }
    if version <= 22 {
        connection.execute_batch(MIGRATE_V22_TO_V23_SQL)?;
    }
    if version <= 23 {
        connection.execute_batch(MIGRATE_V23_TO_V24_SQL)?;
    }
    if version <= 24 {
        connection.execute_batch(MIGRATE_V24_TO_V25_SQL)?;
    }
    if version <= 25 {
        connection.execute_batch(MIGRATE_V25_TO_V26_SQL)?;
    }
    if version <= 26 {
        connection.execute_batch(MIGRATE_V26_TO_V27_SQL)?;
        super::governance_record_journal::semantic::rebuild_locked(connection)
            .map_err(OpenAttemptError::Store)?;
    }
    if version <= 27 {
        connection.execute_batch(MIGRATE_V27_TO_V28_SQL)?;
    }
    if version <= 28 {
        connection.execute_batch(MIGRATE_V28_TO_V29_SQL)?;
    }
    Ok(())
}

/// Creates the v1 schema when opening a fresh Hub; existing Hubs skip this.
fn create_if_empty(connection: &Connection, version: i64) -> Result<(), OpenAttemptError> {
    if version == 0 {
        create_v1_schema(connection)?;
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

pub(super) fn schema_version(connection: &Connection) -> Result<i64, SqliteError> {
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
