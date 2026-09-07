use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use cap_std::fs::{Dir, OpenOptions};

use crate::runtime_domain::{TOOL_EFFECT_UNCERTAIN_CODE, ToolError};

use super::commit::{identity_from_cap, identity_matches_cap, identity_matches_std};
use super::metadata::{ensure_parent_acl_safe, ensure_staged_metadata_safe};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn stage_file(
    parent: &Dir,
    contents: &[u8],
) -> Result<(PathBuf, cap_std::fs::File), ToolError> {
    let (temp, mut file) = create_temp(parent)?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        cleanup_if_owned(parent, &temp, &file)?;
        return Err(write_failed(&error));
    }
    Ok((temp, file))
}

pub(super) fn create_temp(parent: &Dir) -> Result<(PathBuf, cap_std::fs::File), ToolError> {
    ensure_parent_acl_safe(parent)?;
    for _ in 0..64 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let nonce = temp_nonce(sequence);
        let name = PathBuf::from(format!(
            ".forge-edit-{}-{sequence}-{nonce:016x}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        match parent.open_with(&name, &options) {
            Ok(file) => {
                if let Err(error) = validate_created_temp(parent, &file) {
                    cleanup_if_owned(parent, &name, &file)?;
                    return Err(error);
                }
                return Ok((name, file));
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(write_failed(&error)),
        }
    }
    Err(ToolError::new(
        "write_failed",
        "could not allocate a unique temporary file",
    ))
}

fn validate_created_temp(parent: &Dir, file: &cap_std::fs::File) -> Result<(), ToolError> {
    harden_created_temp(file)?;
    ensure_staged_metadata_safe(file)?;
    ensure_parent_acl_safe(parent)
}

#[cfg(unix)]
#[allow(
    clippy::verbose_bit_mask,
    reason = "the exact Unix permission mask is clearest in octal"
)]
fn harden_created_temp(file: &cap_std::fs::File) -> Result<(), ToolError> {
    use cap_std::fs::MetadataExt as _;
    use rustix::fs::{Mode, fchmod};

    fchmod(file, Mode::RUSR | Mode::WUSR)
        .map_err(|error| ToolError::new("write_failed", error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| ToolError::new("write_failed", error.to_string()))?;
    if metadata.mode() & 0o7777 != 0o600
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.nlink() != 1
    {
        return Err(ToolError::new(
            "write_failed",
            "temporary edit file did not retain private ownership and mode",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn harden_created_temp(_file: &cap_std::fs::File) -> Result<(), ToolError> {
    Ok(())
}

fn temp_nonce(sequence: u64) -> u64 {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.write_u64(sequence);
    hasher.finish()
}

#[cfg(unix)]
pub(super) fn verify_staged_name(
    parent: &Dir,
    path: &Path,
    staged_file: &cap_std::fs::File,
) -> Result<(), ToolError> {
    let expected = staged_file.metadata().map_err(|_| staging_conflict())?;
    let descriptor = rustix::fs::openat(
        parent,
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|_| staging_conflict())?;
    let actual = std::fs::File::from(descriptor)
        .metadata()
        .map_err(|_| staging_conflict())?;
    if !actual.is_file() || !identity_matches_std(identity_from_cap(&expected), &actual) {
        return Err(staging_conflict());
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_staged_name(
    parent: &Dir,
    path: &Path,
    staged_file: &cap_std::fs::File,
) -> Result<(), ToolError> {
    verify_staged_metadata(parent, path, staged_file)
}

pub(super) fn verify_staged_metadata(
    parent: &Dir,
    path: &Path,
    staged_file: &cap_std::fs::File,
) -> Result<(), ToolError> {
    let expected = staged_file.metadata().map_err(|_| staging_conflict())?;
    let actual = parent
        .symlink_metadata(path)
        .map_err(|_| staging_conflict())?;
    if actual.is_symlink()
        || !actual.is_file()
        || !identity_matches_cap(identity_from_cap(&expected), &actual)
    {
        return Err(staging_conflict());
    }
    Ok(())
}

pub(super) fn cleanup_if_owned(
    parent: &Dir,
    path: &Path,
    staged_file: &cap_std::fs::File,
) -> Result<(), ToolError> {
    let expected = staged_file.metadata().map_err(|_| cleanup_uncertain())?;
    let actual = match parent.symlink_metadata(path) {
        Ok(actual) => actual,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(cleanup_uncertain()),
    };
    if actual.is_symlink()
        || !actual.is_file()
        || !identity_matches_cap(identity_from_cap(&expected), &actual)
    {
        return Err(cleanup_uncertain());
    }
    parent.remove_file(path).map_err(|_| cleanup_uncertain())
}

#[cfg(unix)]
pub(super) fn sync_parent(parent: &Dir) -> std::io::Result<()> {
    let descriptor = rustix::fs::openat(
        parent,
        ".",
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    rustix::fs::fsync(descriptor).map_err(Into::into)
}

#[cfg(not(unix))]
pub(super) fn sync_parent(_parent: &Dir) -> std::io::Result<()> {
    Ok(())
}

fn staging_conflict() -> ToolError {
    ToolError::new(
        "staging_conflict",
        "temporary edit path no longer identifies the staged regular file",
    )
}

fn cleanup_uncertain() -> ToolError {
    ToolError::new(
        TOOL_EFFECT_UNCERTAIN_CODE,
        "removal of the private staged edit file could not be confirmed",
    )
}

fn write_failed(error: &std::io::Error) -> ToolError {
    ToolError::new("write_failed", error.to_string())
}
