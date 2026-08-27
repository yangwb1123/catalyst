#![allow(clippy::missing_errors_doc)]

use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[path = "scheduled_executor_sidecar/directory_control.rs"]
mod directory_control;
#[path = "scheduled_executor_sidecar/linux_identity.rs"]
mod linux_identity;

use directory_control::{acquire_capacity_guard, prepare_directory};

const DOCUMENT_VERSION: u16 = 1;
const MAX_DOCUMENT_BYTES: usize = 4 * 1024;
const MAX_PROVIDER_REQUEST_ID_BYTES: usize = 128;
const MAX_LANE_OWNERSHIP_ID_BYTES: usize = 256;
const OWNER_PATH_DOMAIN: &[u8] = b"forge.scheduled-executor-sidecar-owner.v1\0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScheduledExecutorSidecarDocument {
    pub(crate) v: u16,
    pub(crate) provider_request_id: String,
    pub(crate) lane_ownership_id: String,
    pub(crate) pid: u32,
    pub(crate) linux_machine_id: String,
    pub(crate) linux_boot_id: String,
    pub(crate) linux_pid_namespace_id: String,
    pub(crate) linux_time_namespace_id: String,
    pub(crate) proc_start_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScheduledExecutorLiveness {
    Live,
    Dead,
    PidReused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScheduledExecutorSidecarError {
    InvalidInput,
    AlreadyExists,
    NotFound,
    UnsafeDirectory,
    UnsafeFile,
    InvalidDocument,
    OwnershipChanged,
    ForeignMachine,
    ForeignPidNamespace,
    ForeignTimeNamespace,
    UnsafeProcfsView,
    CapacityExceeded,
    Io,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

/// An opened exact owner sidecar. Handles returned by `create` clean up on
/// drop; handles returned by `open` are observations and require explicit
/// cleanup after the durable adjudication or terminal transaction commits.
pub(crate) struct ScheduledExecutorSidecar {
    directory: PathBuf,
    path: PathBuf,
    document: ScheduledExecutorSidecarDocument,
    file: File,
    identity: FileIdentity,
    cleanup_on_drop: bool,
    cleaned: bool,
}

impl ScheduledExecutorSidecar {
    pub(crate) fn create(
        directory: &Path,
        provider_request_id: &str,
        lane_ownership_id: &str,
    ) -> Result<Self, ScheduledExecutorSidecarError> {
        validate_owner_ids(provider_request_id, lane_ownership_id)?;
        prepare_directory(directory)?;
        let capacity_guard = acquire_capacity_guard(directory)?;
        let document =
            ScheduledExecutorSidecarDocument::current(provider_request_id, lane_ownership_id)?;
        let bytes = document.encode_exact()?;
        let path = sidecar_path(directory, provider_request_id, lane_ownership_id);
        let file = create_private_file(&path)?;
        let identity = file_identity(&file.metadata().map_err(io_error)?);
        let mut sidecar = Self {
            directory: directory.to_path_buf(),
            path,
            document,
            file,
            identity,
            cleanup_on_drop: true,
            cleaned: false,
        };
        sidecar.persist(&bytes)?;
        drop(capacity_guard);
        Ok(sidecar)
    }

    pub(crate) fn open(
        directory: &Path,
        provider_request_id: &str,
        lane_ownership_id: &str,
    ) -> Result<Self, ScheduledExecutorSidecarError> {
        validate_owner_ids(provider_request_id, lane_ownership_id)?;
        validate_directory(directory)?;
        let path = sidecar_path(directory, provider_request_id, lane_ownership_id);
        let (file, identity, bytes) = open_private_file(&path)?;
        let document = ScheduledExecutorSidecarDocument::decode_exact(&bytes)?;
        document.validate_owner(provider_request_id, lane_ownership_id)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            path,
            document,
            file,
            identity,
            cleanup_on_drop: false,
            cleaned: false,
        })
    }

    #[must_use]
    pub(crate) const fn document(&self) -> &ScheduledExecutorSidecarDocument {
        &self.document
    }

    #[must_use]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn liveness(
        &self,
    ) -> Result<ScheduledExecutorLiveness, ScheduledExecutorSidecarError> {
        linux_identity::classify(&self.document)
    }

    /// Keeps a freshly-created sidecar across drop. The effectful caller uses
    /// this before an operation whose commit result may be uncertain.
    pub(crate) const fn preserve_on_drop(&mut self) {
        self.cleanup_on_drop = false;
    }

    /// Removes only the still-present path whose device/inode match this open
    /// handle. A replacement at the same name is left untouched.
    pub(crate) fn cleanup(mut self) -> Result<(), ScheduledExecutorSidecarError> {
        self.remove_owned_path()
    }

    fn persist(&mut self, bytes: &[u8]) -> Result<(), ScheduledExecutorSidecarError> {
        self.file.write_all(bytes).map_err(io_error)?;
        self.file.sync_all().map_err(io_error)?;
        let metadata = self.file.metadata().map_err(io_error)?;
        if file_identity(&metadata) != self.identity || metadata.len() != bytes.len() as u64 {
            return Err(ScheduledExecutorSidecarError::UnsafeFile);
        }
        validate_path_identity(&self.path, self.identity, bytes.len())?;
        sync_directory(&self.directory)
    }

    fn remove_owned_path(&mut self) -> Result<(), ScheduledExecutorSidecarError> {
        if self.cleaned {
            return Ok(());
        }
        let expected = sidecar_path(
            &self.directory,
            &self.document.provider_request_id,
            &self.document.lane_ownership_id,
        );
        if expected != self.path
            || file_identity(&self.file.metadata().map_err(io_error)?) != self.identity
        {
            return Err(ScheduledExecutorSidecarError::OwnershipChanged);
        }
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                self.cleaned = true;
                return Ok(());
            }
            Err(_) => return Err(ScheduledExecutorSidecarError::Io),
        };
        if !regular_not_symlink(&metadata)
            || metadata.nlink() != 1
            || file_identity(&metadata) != self.identity
        {
            return Err(ScheduledExecutorSidecarError::OwnershipChanged);
        }
        fs::remove_file(&self.path).map_err(io_error)?;
        self.cleaned = true;
        sync_directory(&self.directory)
    }
}

impl Drop for ScheduledExecutorSidecar {
    fn drop(&mut self) {
        if self.cleanup_on_drop && !self.cleaned {
            let _ = self.remove_owned_path();
        }
    }
}

impl ScheduledExecutorSidecarDocument {
    fn current(
        provider_request_id: &str,
        lane_ownership_id: &str,
    ) -> Result<Self, ScheduledExecutorSidecarError> {
        let identity = linux_identity::capture_current()?;
        let value = Self {
            v: DOCUMENT_VERSION,
            provider_request_id: provider_request_id.to_owned(),
            lane_ownership_id: lane_ownership_id.to_owned(),
            pid: std::process::id(),
            linux_machine_id: identity.machine_id,
            linux_boot_id: identity.boot_id,
            linux_pid_namespace_id: identity.pid_namespace_id,
            linux_time_namespace_id: identity.time_namespace_id,
            proc_start_ticks: identity.proc_start_ticks,
        };
        value.validate()?;
        Ok(value)
    }

    fn decode_exact(bytes: &[u8]) -> Result<Self, ScheduledExecutorSidecarError> {
        if !(1..=MAX_DOCUMENT_BYTES).contains(&bytes.len()) {
            return Err(ScheduledExecutorSidecarError::InvalidDocument);
        }
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|_| ScheduledExecutorSidecarError::InvalidDocument)?;
        value.validate()?;
        if value.encode_exact()?.as_slice() != bytes {
            return Err(ScheduledExecutorSidecarError::InvalidDocument);
        }
        Ok(value)
    }

    fn encode_exact(&self) -> Result<Vec<u8>, ScheduledExecutorSidecarError> {
        let bytes =
            serde_json::to_vec(self).map_err(|_| ScheduledExecutorSidecarError::InvalidDocument)?;
        (1..=MAX_DOCUMENT_BYTES)
            .contains(&bytes.len())
            .then_some(bytes)
            .ok_or(ScheduledExecutorSidecarError::InvalidDocument)
    }

    fn validate(&self) -> Result<(), ScheduledExecutorSidecarError> {
        let valid = self.v == DOCUMENT_VERSION
            && valid_identifier(&self.provider_request_id, MAX_PROVIDER_REQUEST_ID_BYTES)
            && valid_identifier(&self.lane_ownership_id, MAX_LANE_OWNERSHIP_ID_BYTES)
            && self.pid > 0
            && linux_identity::valid_machine_id(&self.linux_machine_id)
            && linux_identity::valid_boot_id(&self.linux_boot_id)
            && linux_identity::valid_pid_namespace_id(&self.linux_pid_namespace_id)
            && linux_identity::valid_time_namespace_id(&self.linux_time_namespace_id)
            && self.proc_start_ticks > 0;
        valid
            .then_some(())
            .ok_or(ScheduledExecutorSidecarError::InvalidDocument)
    }

    fn validate_owner(
        &self,
        provider_request_id: &str,
        lane_ownership_id: &str,
    ) -> Result<(), ScheduledExecutorSidecarError> {
        (self.provider_request_id == provider_request_id
            && self.lane_ownership_id == lane_ownership_id)
            .then_some(())
            .ok_or(ScheduledExecutorSidecarError::InvalidDocument)
    }
}

fn validate_directory(directory: &Path) -> Result<(), ScheduledExecutorSidecarError> {
    let path_metadata = fs::symlink_metadata(directory)
        .map_err(|error| map_not_found(&error, ScheduledExecutorSidecarError::UnsafeDirectory))?;
    if path_metadata.file_type().is_symlink() || !path_metadata.is_dir() {
        return Err(ScheduledExecutorSidecarError::UnsafeDirectory);
    }
    let opened = File::open(directory).map_err(io_error)?;
    let opened_metadata = opened.metadata().map_err(io_error)?;
    let exact = opened_metadata.is_dir()
        && file_identity(&opened_metadata) == file_identity(&path_metadata)
        && opened_metadata.mode() & 0o777 == 0o700;
    exact
        .then_some(())
        .ok_or(ScheduledExecutorSidecarError::UnsafeDirectory)
}

fn create_private_file(path: &Path) -> Result<File, ScheduledExecutorSidecarError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                ScheduledExecutorSidecarError::AlreadyExists
            } else {
                ScheduledExecutorSidecarError::Io
            }
        })?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(io_error)?;
    validate_opened_file(&file, path, true)?;
    Ok(file)
}

fn open_private_file(
    path: &Path,
) -> Result<(File, FileIdentity, Vec<u8>), ScheduledExecutorSidecarError> {
    let before = checked_path_metadata(path, false)?;
    let mut file = OpenOptions::new().read(true).open(path).map_err(io_error)?;
    validate_opened_file(&file, path, false)?;
    let identity = file_identity(&file.metadata().map_err(io_error)?);
    if identity != file_identity(&before) {
        return Err(ScheduledExecutorSidecarError::UnsafeFile);
    }
    let capacity = usize::try_from(before.len())
        .unwrap_or(MAX_DOCUMENT_BYTES)
        .min(MAX_DOCUMENT_BYTES);
    let mut bytes = Vec::with_capacity(capacity);
    Read::by_ref(&mut file)
        .take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    validate_path_identity(path, identity, bytes.len())?;
    Ok((file, identity, bytes))
}

fn validate_opened_file(
    file: &File,
    path: &Path,
    allow_empty: bool,
) -> Result<(), ScheduledExecutorSidecarError> {
    let opened = file.metadata().map_err(io_error)?;
    let path_metadata = checked_path_metadata(path, allow_empty)?;
    let exact = safe_regular_file(&opened)
        && file_identity(&opened) == file_identity(&path_metadata)
        && (allow_empty || opened.len() > 0)
        && opened.len() <= MAX_DOCUMENT_BYTES as u64;
    exact
        .then_some(())
        .ok_or(ScheduledExecutorSidecarError::UnsafeFile)
}

fn checked_path_metadata(
    path: &Path,
    allow_empty: bool,
) -> Result<Metadata, ScheduledExecutorSidecarError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| map_not_found(&error, ScheduledExecutorSidecarError::NotFound))?;
    let sized = (allow_empty || metadata.len() > 0) && metadata.len() <= MAX_DOCUMENT_BYTES as u64;
    (safe_regular_file(&metadata) && sized)
        .then_some(metadata)
        .ok_or(ScheduledExecutorSidecarError::UnsafeFile)
}

fn validate_path_identity(
    path: &Path,
    identity: FileIdentity,
    bytes: usize,
) -> Result<(), ScheduledExecutorSidecarError> {
    let metadata = checked_path_metadata(path, false)?;
    (file_identity(&metadata) == identity && metadata.len() == bytes as u64)
        .then_some(())
        .ok_or(ScheduledExecutorSidecarError::UnsafeFile)
}

fn safe_regular_file(metadata: &Metadata) -> bool {
    regular_not_symlink(metadata) && metadata.mode() & 0o777 == 0o600 && metadata.nlink() == 1
}

fn regular_not_symlink(metadata: &Metadata) -> bool {
    metadata.file_type().is_file() && !metadata.file_type().is_symlink()
}

fn file_identity(metadata: &Metadata) -> FileIdentity {
    FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

fn sync_directory(directory: &Path) -> Result<(), ScheduledExecutorSidecarError> {
    let opened = File::open(directory).map_err(io_error)?;
    opened.sync_all().map_err(io_error)
}

fn sync_parent_directory(directory: &Path) -> Result<(), ScheduledExecutorSidecarError> {
    let parent = directory
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    sync_directory(parent)
}

fn sidecar_path(directory: &Path, provider_request_id: &str, lane_ownership_id: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(OWNER_PATH_DOMAIN);
    hasher.update(provider_request_id.as_bytes());
    hasher.update([0]);
    hasher.update(lane_ownership_id.as_bytes());
    directory.join(format!(
        "scheduled-executor-owner-{:x}.json",
        hasher.finalize()
    ))
}

fn validate_owner_ids(
    provider_request_id: &str,
    lane_ownership_id: &str,
) -> Result<(), ScheduledExecutorSidecarError> {
    (valid_identifier(provider_request_id, MAX_PROVIDER_REQUEST_ID_BYTES)
        && valid_identifier(lane_ownership_id, MAX_LANE_OWNERSHIP_ID_BYTES))
    .then_some(())
    .ok_or(ScheduledExecutorSidecarError::InvalidInput)
}

fn valid_identifier(value: &str, maximum: usize) -> bool {
    (1..=maximum).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && byte != b' ')
}

fn map_not_found(
    error: &std::io::Error,
    fallback: ScheduledExecutorSidecarError,
) -> ScheduledExecutorSidecarError {
    if error.kind() == std::io::ErrorKind::NotFound {
        ScheduledExecutorSidecarError::NotFound
    } else {
        fallback
    }
}

fn io_error(_: std::io::Error) -> ScheduledExecutorSidecarError {
    ScheduledExecutorSidecarError::Io
}

impl std::fmt::Display for ScheduledExecutorSidecarError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidInput => "scheduled executor sidecar input is invalid",
            Self::AlreadyExists => "scheduled executor sidecar owner already exists",
            Self::NotFound => "scheduled executor sidecar was not found",
            Self::UnsafeDirectory => "scheduled executor sidecar directory is unsafe",
            Self::UnsafeFile => "scheduled executor sidecar file is unsafe",
            Self::InvalidDocument => "scheduled executor sidecar document is invalid",
            Self::OwnershipChanged => "scheduled executor sidecar ownership changed",
            Self::ForeignMachine => "scheduled executor sidecar belongs to another machine",
            Self::ForeignPidNamespace => {
                "scheduled executor sidecar belongs to another PID namespace"
            }
            Self::ForeignTimeNamespace => {
                "scheduled executor sidecar belongs to another time namespace"
            }
            Self::UnsafeProcfsView => "scheduled executor procfs PID view is unsafe",
            Self::CapacityExceeded => "scheduled executor sidecar directory capacity was reached",
            Self::Io => "scheduled executor sidecar storage is unavailable",
        })
    }
}

impl std::error::Error for ScheduledExecutorSidecarError {}

#[cfg(test)]
#[path = "scheduled_executor_sidecar/tests.rs"]
mod tests;
