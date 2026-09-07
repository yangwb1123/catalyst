use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::runtime_domain::{RunEntity, RunStoreError};

use super::{SqliteHubStore, SqliteHubStoreOpenMode};

/// Holds cooperative, process-wide execution ownership for one local Hub.
#[must_use = "dropping the guard releases local Run execution ownership"]
pub struct RunExecutionGuard {
    _lock_file: fs::File,
}

impl SqliteHubStore {
    /// Tries to claim the single local Run execution slot for this Hub.
    ///
    /// The operating system releases the claim if the process exits. This is
    /// cooperative serialization between Forge processes, not a security lock
    /// against another same-user program.
    ///
    /// # Errors
    ///
    /// Returns a conflict when another Forge process owns the slot, or fails
    /// closed when the private lock path or lock cannot be safely verified.
    pub fn try_acquire_run_execution_guard(&self) -> Result<RunExecutionGuard, RunStoreError> {
        if !matches!(self.open_mode, SqliteHubStoreOpenMode::ReadWrite) {
            return Err(unavailable("Run execution requires a read-write Hub"));
        }
        verify_unique_database_path(&self.database_path)?;
        let lock_path = execution_lock_path(&self.database_path)?;
        verify_private_parent(&lock_path)?;
        let lock_file = open_lock_file(&lock_path).map_err(unavailable)?;
        let opened = prepare_opened_lock(&lock_file)?;
        lock_file.try_lock().map_err(lock_error)?;
        let named = fs::symlink_metadata(&lock_path).map_err(unavailable)?;
        verify_lock_metadata(&named)?;
        if !same_file(&opened, &named) {
            return Err(changed_path());
        }
        Ok(RunExecutionGuard {
            _lock_file: lock_file,
        })
    }
}

fn verify_unique_database_path(path: &Path) -> Result<(), RunStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(unavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(unavailable(
            "Run execution requires a regular non-symbolic Hub database",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        if metadata.nlink() != 1 {
            return Err(unavailable(
                "Run execution rejects a multiply linked Hub database",
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn prepare_opened_lock(lock_file: &fs::File) -> Result<fs::Metadata, RunStoreError> {
    use rustix::fs::{Mode, fchmod};

    let initial = lock_file.metadata().map_err(unavailable)?;
    verify_lock_shape(&initial)?;
    verify_lock_owner(&initial)?;
    fchmod(lock_file, Mode::RUSR | Mode::WUSR).map_err(unavailable)?;
    let hardened = lock_file.metadata().map_err(unavailable)?;
    verify_lock_metadata(&hardened)?;
    Ok(hardened)
}

#[cfg(not(unix))]
fn prepare_opened_lock(lock_file: &fs::File) -> Result<fs::Metadata, RunStoreError> {
    let metadata = lock_file.metadata().map_err(unavailable)?;
    verify_lock_metadata(&metadata)?;
    Ok(metadata)
}

fn execution_lock_path(database: &Path) -> Result<PathBuf, RunStoreError> {
    let mut name = database
        .file_name()
        .ok_or_else(|| unavailable("Hub database lock path has no file name"))?
        .to_os_string();
    name.push(".run-execution.lock");
    Ok(database.with_file_name(name))
}

fn verify_private_parent(path: &Path) -> Result<(), RunStoreError> {
    let parent = path
        .parent()
        .ok_or_else(|| unavailable("Hub execution lock has no parent directory"))?;
    let metadata = fs::symlink_metadata(parent).map_err(unavailable)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(unavailable("Hub execution lock parent is not a directory"));
    }
    #[cfg(unix)]
    verify_private_mode(&metadata)?;
    Ok(())
}

fn verify_lock_metadata(metadata: &fs::Metadata) -> Result<(), RunStoreError> {
    verify_lock_shape(metadata)?;
    #[cfg(unix)]
    {
        verify_lock_owner(metadata)?;
        verify_lock_mode(metadata)?;
    }
    Ok(())
}

fn verify_lock_shape(metadata: &fs::Metadata) -> Result<(), RunStoreError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() != 0 {
        return Err(unavailable(
            "Hub execution lock must be an empty regular file",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        verify_private_mode(metadata)?;
        if metadata.nlink() != 1 {
            return Err(unavailable("Hub execution lock has multiple hard links"));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn verify_lock_owner(metadata: &fs::Metadata) -> Result<(), RunStoreError> {
    use std::os::unix::fs::MetadataExt as _;

    if metadata.uid() == rustix::process::geteuid().as_raw() {
        Ok(())
    } else {
        Err(unavailable(
            "Hub execution lock is not owned by the current user",
        ))
    }
}

#[cfg(unix)]
#[allow(
    clippy::verbose_bit_mask,
    reason = "the exact Unix permission mask is clearest in octal"
)]
fn verify_lock_mode(metadata: &fs::Metadata) -> Result<(), RunStoreError> {
    use std::os::unix::fs::PermissionsExt as _;

    if metadata.permissions().mode() & 0o7777 == 0o600 {
        Ok(())
    } else {
        Err(unavailable("Hub execution lock mode is not exactly 0600"))
    }
}

#[cfg(unix)]
#[allow(
    clippy::verbose_bit_mask,
    reason = "the Unix group/other permission mask is clearest in octal"
)]
fn verify_private_mode(metadata: &fs::Metadata) -> Result<(), RunStoreError> {
    use std::os::unix::fs::PermissionsExt as _;

    if metadata.permissions().mode() & 0o077 == 0 {
        Ok(())
    } else {
        Err(unavailable("Hub execution lock path is not private"))
    }
}

#[cfg(unix)]
fn open_lock_file(path: &Path) -> io::Result<fs::File> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .map(fs::File::from)
    .map_err(Into::into)
}

#[cfg(not(unix))]
fn open_lock_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

#[cfg(unix)]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

fn lock_error(error: fs::TryLockError) -> RunStoreError {
    match error {
        fs::TryLockError::WouldBlock => RunStoreError::Conflict {
            entity: RunEntity::Run,
            message: "another local Run execution is active for this Hub".into(),
        },
        fs::TryLockError::Error(error) => unavailable(error),
    }
}

fn changed_path() -> RunStoreError {
    unavailable("Hub execution lock changed while Run ownership was acquired")
}

fn unavailable(error: impl std::fmt::Display) -> RunStoreError {
    RunStoreError::Unavailable {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_is_exclusive_and_released_on_drop() {
        let state = tempfile::TempDir::new().expect("state");
        let store = SqliteHubStore::open(state.path().join("hub.sqlite3")).expect("open Hub");
        let guard = store
            .try_acquire_run_execution_guard()
            .expect("first execution owner");
        let lock_path = state.path().join("hub.sqlite3.run-execution.lock");
        assert_eq!(fs::metadata(&lock_path).expect("lock metadata").len(), 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

            let metadata = fs::metadata(&lock_path).expect("private lock metadata");
            assert_eq!(metadata.permissions().mode() & 0o077, 0);
            assert_eq!(metadata.nlink(), 1);
        }
        assert!(matches!(
            store.try_acquire_run_execution_guard(),
            Err(RunStoreError::Conflict {
                entity: RunEntity::Run,
                ..
            })
        ));
        drop(guard);
        let _released = store
            .try_acquire_run_execution_guard()
            .expect("ownership released");
    }

    #[cfg(unix)]
    #[test]
    fn multiply_linked_coordination_file_is_rejected() {
        use std::os::unix::fs::PermissionsExt as _;

        let state = tempfile::TempDir::new().expect("state");
        let store = SqliteHubStore::open(state.path().join("hub.sqlite3")).expect("open Hub");
        let lock_path = state.path().join("hub.sqlite3.run-execution.lock");
        fs::write(&lock_path, []).expect("coordination file");
        fs::set_permissions(&lock_path, fs::Permissions::from_mode(0o600)).expect("private mode");
        fs::hard_link(&lock_path, state.path().join("alias")).expect("hard link");

        assert!(matches!(
            store.try_acquire_run_execution_guard(),
            Err(RunStoreError::Unavailable { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn hard_linked_database_cannot_split_execution_ownership_by_path() {
        let first_state = tempfile::TempDir::new().expect("first state");
        let second_state = tempfile::TempDir::new().expect("second state");
        let first_path = first_state.path().join("hub.sqlite3");
        let second_path = second_state.path().join("hub.sqlite3");
        let first = SqliteHubStore::open(&first_path).expect("open first Hub");
        fs::hard_link(&first_path, &second_path).expect("database alias");

        assert!(matches!(
            first.try_acquire_run_execution_guard(),
            Err(RunStoreError::Unavailable { .. })
        ));
        assert!(matches!(
            SqliteHubStore::open(second_path),
            Err(crate::runtime_domain::HubStoreError::Unavailable { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn strict_umask_does_not_poison_new_coordination_file() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::process::Command;

        const CHILD_PATH: &str = "FORGE_RUN_LOCK_UMASK_CHILD_PATH";
        const TEST_NAME: &str = concat!(
            "sqlite_hub::run_execution_lock::tests::",
            "strict_umask_does_not_poison_new_coordination_file"
        );
        if let Some(path) = std::env::var_os(CHILD_PATH) {
            let lock_file =
                open_lock_file(Path::new(&path)).expect("create lock under strict umask");
            prepare_opened_lock(&lock_file).expect("harden opened lock");
            lock_file.try_lock().expect("lock hardened file");
            return;
        }

        let state = tempfile::TempDir::new().expect("state");
        let lock_path = state.path().join("hub.sqlite3.run-execution.lock");
        let executable = std::env::current_exe().expect("test executable");
        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("umask 777; exec \"$@\"")
            .arg("forge-run-lock-umask-test")
            .arg(executable)
            .arg("--exact")
            .arg(TEST_NAME)
            .arg("--nocapture")
            .env(CHILD_PATH, &lock_path);
        let output = command.output().expect("strict-umask child");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata = fs::metadata(lock_path).expect("hardened lock metadata");
        assert_eq!(metadata.permissions().mode() & 0o7777, 0o600);
    }
}
