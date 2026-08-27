use std::{
    fs::{self, DirBuilder, File},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::Path,
};

use super::{
    ScheduledExecutorSidecarError, file_identity, io_error, map_not_found, sync_directory,
    sync_parent_directory, validate_directory,
};

pub(super) const MAX_OWNER_FILES: usize = 1_024;

pub(super) struct DirectoryCapacityGuard {
    lock: File,
}

impl Drop for DirectoryCapacityGuard {
    fn drop(&mut self) {
        let _ = self.lock.unlock();
    }
}

pub(super) fn prepare_directory(directory: &Path) -> Result<(), ScheduledExecutorSidecarError> {
    let created = match DirBuilder::new().mode(0o700).create(directory) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(_) => return Err(ScheduledExecutorSidecarError::Io),
    };
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| map_not_found(&error, ScheduledExecutorSidecarError::UnsafeDirectory))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ScheduledExecutorSidecarError::UnsafeDirectory);
    }
    if created {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(io_error)?;
    }
    validate_directory(directory)?;
    sync_directory(directory)?;
    if created {
        sync_parent_directory(directory)?;
    }
    Ok(())
}

pub(super) fn acquire_capacity_guard(
    directory: &Path,
) -> Result<DirectoryCapacityGuard, ScheduledExecutorSidecarError> {
    let lock = File::open(directory).map_err(io_error)?;
    lock.lock().map_err(io_error)?;
    validate_locked_directory(directory, &lock)?;
    ensure_available_slot(directory)?;
    Ok(DirectoryCapacityGuard { lock })
}

fn validate_locked_directory(
    directory: &Path,
    lock: &File,
) -> Result<(), ScheduledExecutorSidecarError> {
    validate_directory(directory)?;
    let locked = lock.metadata().map_err(io_error)?;
    let path = fs::symlink_metadata(directory).map_err(io_error)?;
    (locked.is_dir() && file_identity(&locked) == file_identity(&path))
        .then_some(())
        .ok_or(ScheduledExecutorSidecarError::UnsafeDirectory)
}

fn ensure_available_slot(directory: &Path) -> Result<(), ScheduledExecutorSidecarError> {
    let mut count = 0_usize;
    for entry in fs::read_dir(directory).map_err(io_error)? {
        entry.map_err(io_error)?;
        count = count
            .checked_add(1)
            .ok_or(ScheduledExecutorSidecarError::CapacityExceeded)?;
        if count >= MAX_OWNER_FILES {
            return Err(ScheduledExecutorSidecarError::CapacityExceeded);
        }
    }
    Ok(())
}
