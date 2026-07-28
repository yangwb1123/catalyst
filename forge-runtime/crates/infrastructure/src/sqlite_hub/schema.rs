use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use rusqlite::{Connection, Error as SqliteError, ErrorCode};

use super::{
    HubStoreError,
    schema_sql::{CREATE_V1_SCHEMA_SQL, MIGRATE_V1_TO_V2_SQL},
    unavailable,
};

const SCHEMA_VERSION: i64 = 2;
const LEGACY_SCHEMA_VERSION: i64 = 1;
const CONNECTION_BUSY_TIMEOUT: Duration = Duration::from_millis(250);
const OPEN_RETRY_TIMEOUT: Duration = Duration::from_secs(5);
const OPEN_RETRY_DELAY: Duration = Duration::from_millis(10);

pub(super) fn open_database(path: &Path) -> Result<Connection, HubStoreError> {
    let deadline = Instant::now() + OPEN_RETRY_TIMEOUT;
    loop {
        prepare_location(path)?;
        match open_database_once(path) {
            Ok(connection) => {
                restrict_file_permissions(path)?;
                restrict_auxiliary_permissions(path)?;
                return Ok(connection);
            }
            Err(OpenAttemptError::Sqlite(error))
                if is_lock_contention(&error) && Instant::now() < deadline =>
            {
                thread::sleep(OPEN_RETRY_DELAY);
            }
            Err(OpenAttemptError::Sqlite(error)) => return Err(unavailable(error)),
            Err(OpenAttemptError::Store(error)) => return Err(error),
        }
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

fn prepare_location(path: &Path) -> Result<(), HubStoreError> {
    let parent = path.parent().ok_or_else(|| HubStoreError::Unavailable {
        message: "Hub database path has no parent directory".into(),
    })?;
    prepare_private_directory(parent)?;
    reject_symlink(path)?;
    Ok(())
}

fn prepare_private_directory(path: &Path) -> Result<(), HubStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(HubStoreError::Unavailable {
            message: format!(
                "Hub state directory cannot be a symbolic link: {}",
                path.display()
            ),
        }),
        Ok(metadata) if !metadata.is_dir() => Err(HubStoreError::Unavailable {
            message: format!("Hub state path is not a directory: {}", path.display()),
        }),
        Ok(metadata) => verify_private_directory_permissions(path, &metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path).map_err(unavailable)?;
            restrict_directory_permissions(path)
        }
        Err(error) => Err(unavailable(error)),
    }
}

fn reject_symlink(path: &Path) -> Result<(), HubStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(HubStoreError::Unavailable {
            message: format!("Hub database cannot be a symbolic link: {}", path.display()),
        }),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(unavailable(error)),
    }
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
    if matches!(version, 0 | LEGACY_SCHEMA_VERSION | SCHEMA_VERSION) {
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
    connection.execute_batch("BEGIN IMMEDIATE")?;
    let result = validate_locked_schema(connection);
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

fn validate_locked_schema(connection: &Connection) -> Result<(), OpenAttemptError> {
    let version = schema_version(connection)?;
    match version {
        0 => {
            create_v1_schema(connection)?;
            migrate_v1_to_v2(connection)
        }
        LEGACY_SCHEMA_VERSION => migrate_v1_to_v2(connection),
        SCHEMA_VERSION => Ok(()),
        other => Err(unsupported_schema(other)),
    }
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

#[cfg(unix)]
fn restrict_directory_permissions(path: &Path) -> Result<(), HubStoreError> {
    let expected = checked_directory_metadata(path)?;
    restrict_directory_permissions_if_unchanged(path, &expected)
}

#[cfg(unix)]
fn restrict_directory_permissions_if_unchanged(
    path: &Path,
    expected: &fs::Metadata,
) -> Result<(), HubStoreError> {
    use std::os::unix::fs::PermissionsExt;

    let directory = fs::File::open(path).map_err(unavailable)?;
    let opened = directory.metadata().map_err(unavailable)?;
    if !same_file(expected, &opened) {
        return Err(changed_directory_error(path));
    }
    directory
        .set_permissions(fs::Permissions::from_mode(0o700))
        .map_err(unavailable)?;
    let current = checked_directory_metadata(path)?;
    if !same_file(expected, &current) {
        return Err(changed_directory_error(path));
    }
    Ok(())
}

#[cfg(not(unix))]
fn restrict_directory_permissions(_path: &Path) -> Result<(), HubStoreError> {
    Ok(())
}

#[cfg(unix)]
fn verify_private_directory_permissions(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
    // Another first opener may have observed the same empty directory, made it
    // private, and created the database after our initial metadata read. Check
    // the current object before inspecting or changing it based on stale data.
    let current = checked_directory_metadata(path)?;
    if !same_file(metadata, &current) {
        return Err(changed_directory_error(path));
    }
    let mode = directory_mode(&current);
    #[allow(
        clippy::verbose_bit_mask,
        reason = "the Unix group/other permission mask is clearest in octal"
    )]
    if mode & 0o077 == 0 {
        return Ok(());
    }

    let is_empty = fs::read_dir(path)
        .map_err(unavailable)?
        .next()
        .transpose()
        .map_err(unavailable)?
        .is_none();
    let inspected = checked_directory_metadata(path)?;
    if !same_file(&current, &inspected) {
        return Err(changed_directory_error(path));
    }
    let mode = directory_mode(&inspected);
    #[allow(
        clippy::verbose_bit_mask,
        reason = "the Unix group/other permission mask is clearest in octal"
    )]
    if mode & 0o077 == 0 {
        return Ok(());
    }
    if is_empty {
        return restrict_directory_permissions_if_unchanged(path, &inspected);
    }
    Err(HubStoreError::Unavailable {
        message: format!(
            "existing Hub state directory is accessible by group or others \
             (mode {mode:o}); choose a dedicated private directory or run chmod 700 {}",
            path.display()
        ),
    })
}

#[cfg(unix)]
fn checked_directory_metadata(path: &Path) -> Result<fs::Metadata, HubStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(unavailable)?;
    if metadata.file_type().is_symlink() {
        return Err(HubStoreError::Unavailable {
            message: format!(
                "Hub state directory cannot be a symbolic link: {}",
                path.display()
            ),
        });
    }
    if !metadata.is_dir() {
        return Err(HubStoreError::Unavailable {
            message: format!("Hub state path is not a directory: {}", path.display()),
        });
    }
    Ok(metadata)
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(unix)]
fn directory_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o777
}

#[cfg(unix)]
fn changed_directory_error(path: &Path) -> HubStoreError {
    HubStoreError::Unavailable {
        message: format!(
            "Hub state directory changed while its permissions were being verified: {}",
            path.display()
        ),
    }
}

#[cfg(not(unix))]
fn verify_private_directory_permissions(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
    Ok(())
}

#[cfg(unix)]
fn restrict_file_permissions(path: &Path) -> Result<(), HubStoreError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(unavailable)
}

#[cfg(not(unix))]
fn restrict_file_permissions(_path: &Path) -> Result<(), HubStoreError> {
    Ok(())
}

#[cfg(unix)]
fn restrict_auxiliary_permissions(path: &Path) -> Result<(), HubStoreError> {
    for suffix in ["-wal", "-shm"] {
        let mut auxiliary = path.as_os_str().to_os_string();
        auxiliary.push(suffix);
        let auxiliary = std::path::PathBuf::from(auxiliary);
        if auxiliary.exists() {
            restrict_file_permissions(&auxiliary)?;
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn restrict_auxiliary_permissions(_path: &Path) -> Result<(), HubStoreError> {
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
        path::{Path, PathBuf},
    };

    use super::{open_database, verify_private_directory_permissions};

    #[test]
    fn generated_wal_and_shm_files_are_private() {
        let root = tempfile::tempdir().expect("private state directory");
        let database = root.path().join("hub.sqlite3");
        let connection = open_database(&database).expect("open Hub database");
        let journal_mode: String = connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .expect("query journal mode");

        assert_eq!(journal_mode, "wal");
        for suffix in ["-wal", "-shm"] {
            let auxiliary = auxiliary_path(&database, suffix);
            let mode = std::fs::metadata(&auxiliary)
                .unwrap_or_else(|error| panic!("{} unavailable: {error}", auxiliary.display()))
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{} must be private", auxiliary.display());
        }
    }

    #[test]
    fn stale_shared_metadata_accepts_directory_restricted_by_concurrent_opener() {
        let root = tempfile::tempdir().expect("temporary root");
        let state = root.path().join("state");
        fs::create_dir(&state).expect("state directory");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o775))
            .expect("initial shared permissions");
        let stale_metadata = fs::symlink_metadata(&state).expect("shared metadata");

        fs::set_permissions(&state, fs::Permissions::from_mode(0o700))
            .expect("concurrent opener restricts permissions");
        fs::write(state.join("hub.sqlite3"), []).expect("concurrent opener creates database");

        verify_private_directory_permissions(&state, &stale_metadata)
            .expect("current private permissions supersede stale metadata");
        assert_eq!(
            fs::symlink_metadata(&state)
                .expect("current metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }

    #[test]
    fn stale_shared_metadata_rejects_symlink_replacement_without_chmod_target() {
        let root = tempfile::tempdir().expect("temporary root");
        let state = root.path().join("state");
        fs::create_dir(&state).expect("state directory");
        fs::set_permissions(&state, fs::Permissions::from_mode(0o775))
            .expect("initial shared permissions");
        let stale_metadata = fs::symlink_metadata(&state).expect("shared metadata");

        fs::remove_dir(&state).expect("remove original state directory");
        let replacement = root.path().join("replacement");
        fs::create_dir(&replacement).expect("replacement directory");
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o755))
            .expect("replacement permissions");
        symlink(&replacement, &state).expect("replace state path with symlink");

        verify_private_directory_permissions(&state, &stale_metadata)
            .expect_err("symlink replacement must be rejected");
        assert_eq!(
            fs::symlink_metadata(&replacement)
                .expect("replacement metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755,
            "symlink target permissions must remain unchanged"
        );
    }

    fn auxiliary_path(database: &Path, suffix: &str) -> PathBuf {
        let mut path = database.as_os_str().to_os_string();
        path.push(suffix);
        path.into()
    }
}
