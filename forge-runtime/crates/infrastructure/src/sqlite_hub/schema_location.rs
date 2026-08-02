use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

use super::{HubStoreError, unavailable};

const SQLITE_HEADER_BYTES: usize = 20;
const SQLITE_HEADER_MAGIC: &[u8; 16] = b"SQLite format 3\0";
const SQLITE_WAL_FORMAT: u8 = 2;

pub(super) fn prepare(path: &Path) -> Result<(), HubStoreError> {
    let parent = path.parent().ok_or_else(|| HubStoreError::Unavailable {
        message: "Hub database path has no parent directory".into(),
    })?;
    prepare_private_directory(parent)?;
    reject_symlink(path)
}

pub(super) fn restrict(path: &Path) -> Result<(), HubStoreError> {
    restrict_file_permissions(path)?;
    restrict_auxiliary_permissions(path)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExistingDatabaseIdentity {
    canonical_path: PathBuf,
    length: u64,
    modified: SystemTime,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl ExistingDatabaseIdentity {
    pub(super) fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }
}

pub(super) fn inspect_existing_read_only(
    path: &Path,
) -> Result<ExistingDatabaseIdentity, HubStoreError> {
    verify_existing_private_parent(path)?;
    let metadata = checked_database_metadata(path)?;
    verify_private_file_permissions(path, &metadata)?;
    reject_auxiliary_files(path)?;
    validate_persistent_wal_header(path)?;
    let canonical_path = fs::canonicalize(path).map_err(unavailable)?;
    let current = checked_database_metadata(&canonical_path)?;
    if !same_database_file(&metadata, &current) {
        return Err(HubStoreError::Unavailable {
            message: format!(
                "Hub database changed while its read-only identity was checked: {}",
                path.display()
            ),
        });
    }
    reject_auxiliary_files(&canonical_path)?;
    validate_persistent_wal_header(&canonical_path)?;
    identity(canonical_path, &current)
}

pub(super) fn verify_existing_private_parent(path: &Path) -> Result<(), HubStoreError> {
    let parent = path.parent().ok_or_else(|| HubStoreError::Unavailable {
        message: "Hub database path has no parent directory".into(),
    })?;
    let metadata = checked_directory_metadata(parent)?;
    if private_directory(&metadata) {
        Ok(())
    } else {
        Err(HubStoreError::Unavailable {
            message: format!(
                "existing Hub state directory is accessible by group or others; run chmod 700 {}",
                parent.display()
            ),
        })
    }
}

pub(super) fn checked_database_metadata(path: &Path) -> Result<fs::Metadata, HubStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            HubStoreError::Unavailable {
                message: format!(
                    "effect-free Hub reads require an existing database: {}",
                    path.display()
                ),
            }
        } else {
            unavailable(error)
        }
    })?;
    if metadata.file_type().is_symlink() {
        return Err(HubStoreError::Unavailable {
            message: format!("Hub database cannot be a symbolic link: {}", path.display()),
        });
    }
    if !metadata.is_file() {
        return Err(HubStoreError::Unavailable {
            message: format!("Hub database path is not a file: {}", path.display()),
        });
    }
    Ok(metadata)
}

fn reject_auxiliary_files(path: &Path) -> Result<(), HubStoreError> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let auxiliary = auxiliary_path(path, suffix);
        match fs::symlink_metadata(&auxiliary) {
            Ok(_) => {
                return Err(HubStoreError::Unavailable {
                    message: format!(
                        "effect-free Hub reads reject SQLite sidecar files: {}",
                        auxiliary.display()
                    ),
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(unavailable(error)),
        }
    }
    Ok(())
}

pub(super) fn validate_persistent_wal_header(path: &Path) -> Result<(), HubStoreError> {
    let mut header = [0_u8; SQLITE_HEADER_BYTES];
    fs::File::open(path)
        .map_err(unavailable)?
        .read_exact(&mut header)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::UnexpectedEof {
                invalid_database_header("header is truncated")
            } else {
                unavailable(error)
            }
        })?;
    if &header[..SQLITE_HEADER_MAGIC.len()] != SQLITE_HEADER_MAGIC {
        return Err(invalid_database_header("magic is invalid"));
    }
    let write_format = header[18];
    let read_format = header[19];
    if write_format != SQLITE_WAL_FORMAT || read_format != SQLITE_WAL_FORMAT {
        return Err(invalid_database_header(&format!(
            "persistent journal format is {write_format}/{read_format}; expected WAL 2/2"
        )));
    }
    Ok(())
}

fn invalid_database_header(detail: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: format!("effect-free Hub read rejected SQLite database header: {detail}"),
    }
}

pub(super) fn auxiliary_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn identity(
    canonical_path: PathBuf,
    metadata: &fs::Metadata,
) -> Result<ExistingDatabaseIdentity, HubStoreError> {
    Ok(ExistingDatabaseIdentity {
        canonical_path,
        length: metadata.len(),
        modified: metadata.modified().map_err(unavailable)?,
        #[cfg(unix)]
        device: database_device(metadata),
        #[cfg(unix)]
        inode: database_inode(metadata),
    })
}

#[cfg(unix)]
pub(super) fn same_database_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    database_device(left) == database_device(right) && database_inode(left) == database_inode(right)
}

#[cfg(not(unix))]
pub(super) fn same_database_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(unix)]
fn database_device(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;

    metadata.dev()
}

#[cfg(unix)]
fn database_inode(metadata: &fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;

    metadata.ino()
}

#[cfg(unix)]
#[allow(
    clippy::verbose_bit_mask,
    reason = "the Unix group/other permission mask is clearest in octal"
)]
pub(super) fn verify_private_file_permissions(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o077 == 0 {
        Ok(())
    } else {
        Err(HubStoreError::Unavailable {
            message: format!(
                "existing Hub database is accessible by group or others; run chmod 600 {}",
                path.display()
            ),
        })
    }
}

#[cfg(not(unix))]
pub(super) fn verify_private_file_permissions(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
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
pub(super) fn verify_private_directory_permissions(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
    let current = checked_directory_metadata(path)?;
    if !same_file(metadata, &current) {
        return Err(changed_directory_error(path));
    }
    if private_directory(&current) {
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
    if private_directory(&inspected) {
        return Ok(());
    }
    if is_empty {
        return restrict_directory_permissions_if_unchanged(path, &inspected);
    }
    let mode = directory_mode(&inspected);
    Err(HubStoreError::Unavailable {
        message: format!(
            "existing Hub state directory is accessible by group or others \
             (mode {mode:o}); choose a dedicated private directory or run chmod 700 {}",
            path.display()
        ),
    })
}

#[cfg(unix)]
fn private_directory(metadata: &fs::Metadata) -> bool {
    #[allow(
        clippy::verbose_bit_mask,
        reason = "the Unix group/other permission mask is clearest in octal"
    )]
    {
        directory_mode(metadata) & 0o077 == 0
    }
}

#[cfg(not(unix))]
fn private_directory(_metadata: &fs::Metadata) -> bool {
    true
}

#[cfg(not(unix))]
pub(super) fn verify_private_directory_permissions(
    _path: &Path,
    _metadata: &fs::Metadata,
) -> Result<(), HubStoreError> {
    Ok(())
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

#[cfg(not(unix))]
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
