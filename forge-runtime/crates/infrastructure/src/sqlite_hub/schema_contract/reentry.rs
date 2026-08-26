//! Validates exact live Hub files for read-only snapshot consumers.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

use rusqlite::{Connection, OpenFlags};
use url::Url;

use super::{
    CONNECTION_BUSY_TIMEOUT, HubStoreError, contract, location, read_only_schema_required,
    schema_version, unavailable,
};

const WAL_HEADER_BYTES: usize = 32;
const WAL_MAGIC_BIG_ENDIAN_CHECKSUM: [u8; 4] = [0x37, 0x7f, 0x06, 0x82];
const WAL_MAGIC_LITTLE_ENDIAN_CHECKSUM: [u8; 4] = [0x37, 0x7f, 0x06, 0x83];
const SHM_MINIMUM_BYTES: u64 = 32_768;
const DISPATCH_REENTRY_VERSIONS: &[i64] = &[
    12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28,
];

pub(in crate::sqlite_hub) fn open_existing_current_live_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    open_existing_live_read_only_database(path, &[28], "current schema version 28", true)
}

pub(in crate::sqlite_hub) fn open_existing_dispatch_reentry_read_only_database(
    path: &Path,
) -> Result<Connection, HubStoreError> {
    open_existing_live_read_only_database(
        path,
        DISPATCH_REENTRY_VERSIONS,
        "dispatch re-entry schema version 12..=28",
        false,
    )
}

fn open_existing_live_read_only_database(
    path: &Path,
    accepted_versions: &[i64],
    requirement: &str,
    upgrade_supported: bool,
) -> Result<Connection, HubStoreError> {
    let before = LiveDatabaseIdentity::inspect(path)?;
    let uri = read_only_file_uri(&before.canonical_path)?;
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
    validate_live_schema(
        &connection,
        version,
        accepted_versions,
        requirement,
        upgrade_supported,
    )?;
    let clean_before = before.wal.as_ref().is_none_or(|wal| wal.length == 0);
    let after = LiveDatabaseIdentity::inspect_after_open(path, clean_before)?;
    if !before.stable_after_open(&after) {
        return Err(HubStoreError::Unavailable {
            message: "Hub main database or WAL changed during live read-only inspection".into(),
        });
    }
    Ok(connection)
}

fn validate_live_schema(
    connection: &Connection,
    version: i64,
    accepted_versions: &[i64],
    requirement: &str,
    upgrade_supported: bool,
) -> Result<(), HubStoreError> {
    if accepted_versions.contains(&version) {
        contract::validate_migration_source(connection, version)?;
        return Ok(());
    }
    if upgrade_supported && (1..28).contains(&version) {
        contract::validate_migration_source(connection, version)?;
        return Err(HubStoreError::Unavailable {
            message: format!(
                "live Hub read requires {requirement}; found valid v{version}; upgrade required"
            ),
        });
    }
    Err(read_only_schema_required(version, requirement))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LiveDatabaseIdentity {
    canonical_path: PathBuf,
    database: StableFileIdentity,
    wal: Option<StableFileIdentity>,
    shm: Option<FileObjectIdentity>,
}

impl LiveDatabaseIdentity {
    fn inspect(path: &Path) -> Result<Self, HubStoreError> {
        Self::inspect_with_empty_wal(path, true)
    }

    fn inspect_after_open(path: &Path, clean_before: bool) -> Result<Self, HubStoreError> {
        Self::inspect_with_empty_wal(path, clean_before)
    }

    fn inspect_with_empty_wal(path: &Path, allow_empty_wal: bool) -> Result<Self, HubStoreError> {
        location::verify_existing_private_parent(path)?;
        let metadata = location::checked_database_metadata(path)?;
        location::verify_private_file_permissions(path, &metadata)?;
        location::validate_persistent_wal_header(path)?;
        let canonical_path = fs::canonicalize(path).map_err(unavailable)?;
        let canonical_metadata = location::checked_database_metadata(&canonical_path)?;
        if !location::same_database_file(&metadata, &canonical_metadata) {
            return Err(changed(path));
        }
        let database = StableFileIdentity::from_metadata(&canonical_metadata)?;
        reject_rollback_journal(path, &canonical_path)?;
        let wal = matching_sidecar(path, &canonical_path, "-wal")?;
        let shm = matching_sidecar(path, &canonical_path, "-shm")?;
        match (&wal, &shm) {
            (Some(wal), Some(shm)) => {
                validate_wal(
                    &location::auxiliary_path(&canonical_path, "-wal"),
                    wal,
                    allow_empty_wal,
                )?;
                if shm.length < SHM_MINIMUM_BYTES {
                    return Err(invalid_sidecar("SQLite SHM is truncated"));
                }
            }
            (None, None) => {}
            _ => {
                return Err(invalid_sidecar(
                    "SQLite live read-only inspection requires WAL and SHM sidecars together",
                ));
            }
        }
        Ok(Self {
            canonical_path,
            database,
            wal,
            shm: shm.map(StableFileIdentity::object),
        })
    }

    fn stable_after_open(&self, after: &Self) -> bool {
        if self == after {
            return true;
        }
        self.canonical_path == after.canonical_path
            && self.database == after.database
            && self.wal.is_none()
            && self.shm.is_none()
            && after.wal.as_ref().is_some_and(|wal| wal.length == 0)
            && after.shm.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StableFileIdentity {
    length: u64,
    modified: SystemTime,
    object: FileObjectIdentity,
}

impl StableFileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Result<Self, HubStoreError> {
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified().map_err(unavailable)?,
            object: FileObjectIdentity::from_metadata(metadata),
        })
    }

    fn object(self) -> FileObjectIdentity {
        self.object
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileObjectIdentity {
    length: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl FileObjectIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        Self {
            length: metadata.len(),
            #[cfg(unix)]
            device: device(metadata),
            #[cfg(unix)]
            inode: inode(metadata),
        }
    }
}

fn matching_sidecar(
    path: &Path,
    canonical_path: &Path,
    suffix: &str,
) -> Result<Option<StableFileIdentity>, HubStoreError> {
    let requested = location::auxiliary_path(path, suffix);
    let canonical = location::auxiliary_path(canonical_path, suffix);
    let Some(requested_metadata) = optional_private_file(&requested)? else {
        if optional_private_file(&canonical)?.is_some() {
            return Err(changed(path));
        }
        return Ok(None);
    };
    let canonical_metadata = optional_private_file(&canonical)?.ok_or_else(|| changed(path))?;
    if !location::same_database_file(&requested_metadata, &canonical_metadata) {
        return Err(changed(path));
    }
    StableFileIdentity::from_metadata(&canonical_metadata).map(Some)
}

fn optional_private_file(path: &Path) -> Result<Option<fs::Metadata>, HubStoreError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            let metadata = location::checked_database_metadata(path)?;
            location::verify_private_file_permissions(path, &metadata)?;
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(unavailable(error)),
    }
}

fn reject_rollback_journal(path: &Path, canonical_path: &Path) -> Result<(), HubStoreError> {
    for database in [path, canonical_path] {
        let journal = location::auxiliary_path(database, "-journal");
        match fs::symlink_metadata(&journal) {
            Ok(_) => {
                return Err(invalid_sidecar(
                    "SQLite rollback journal is not valid for live WAL inspection",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(unavailable(error)),
        }
    }
    Ok(())
}

fn validate_wal(
    path: &Path,
    identity: &StableFileIdentity,
    allow_empty: bool,
) -> Result<(), HubStoreError> {
    // A read-only WAL connection can leave a zero-length WAL beside its full SHM.
    // The pair is a clean coordination placeholder; non-zero WALs need a real header.
    if allow_empty && identity.length == 0 {
        return Ok(());
    }
    if identity.length < WAL_HEADER_BYTES as u64 {
        return Err(invalid_sidecar("SQLite WAL is truncated"));
    }
    let mut magic = [0_u8; 4];
    fs::File::open(path)
        .map_err(unavailable)?
        .read_exact(&mut magic)
        .map_err(unavailable)?;
    if !matches!(
        magic,
        WAL_MAGIC_BIG_ENDIAN_CHECKSUM | WAL_MAGIC_LITTLE_ENDIAN_CHECKSUM
    ) {
        return Err(invalid_sidecar("SQLite WAL magic is invalid"));
    }
    Ok(())
}

fn read_only_file_uri(path: &Path) -> Result<Url, HubStoreError> {
    let mut uri = Url::from_file_path(path).map_err(|()| HubStoreError::Unavailable {
        message: format!(
            "Hub database path cannot be represented as a file URI: {}",
            path.display()
        ),
    })?;
    uri.query_pairs_mut().append_pair("mode", "ro");
    Ok(uri)
}

fn changed(path: &Path) -> HubStoreError {
    HubStoreError::Unavailable {
        message: format!(
            "Hub live read-only files changed while their identity was checked: {}",
            path.display()
        ),
    }
}

fn invalid_sidecar(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

#[cfg(unix)]
fn device(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;

    metadata.dev()
}

#[cfg(unix)]
fn inode(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;

    metadata.ino()
}
