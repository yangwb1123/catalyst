use std::{fs, path::Path};

use super::{HubStoreError, unavailable};

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
