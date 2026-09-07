use std::{io::Read, path::Path};

use cap_std::fs::Dir;

use crate::runtime_domain::ToolError;

use super::{
    commit::{identity_from_cap, identity_matches_std},
    ensure_active, validate_result_size,
};

pub(super) fn read_text(
    parent: &Dir,
    target: &Path,
    expected: &cap_std::fs::Metadata,
    max_bytes: usize,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<String, ToolError> {
    let limit = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if expected.len() > limit {
        return Err(ToolError::new(
            "file_too_large",
            format!("file exceeds the {max_bytes}-byte edit limit"),
        ));
    }
    let file = open_observed_target(parent, target, expected)?;
    super::metadata::ensure_target_metadata_safe(&file)?;
    let bytes = read_bounded(file, max_bytes, cancellation)?;
    validate_result_size(bytes.len(), max_bytes)?;
    String::from_utf8(bytes).map_err(|error| ToolError::new("invalid_encoding", error.to_string()))
}

#[cfg(unix)]
fn open_observed_target(
    parent: &Dir,
    target: &Path,
    expected: &cap_std::fs::Metadata,
) -> Result<std::fs::File, ToolError> {
    let descriptor = rustix::fs::openat(
        parent,
        target,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| ToolError::new("path_denied", error.to_string()))?;
    verify_opened_target(std::fs::File::from(descriptor), expected)
}

#[cfg(not(unix))]
fn open_observed_target(
    parent: &Dir,
    target: &Path,
    expected: &cap_std::fs::Metadata,
) -> Result<std::fs::File, ToolError> {
    let file = parent
        .open(target)
        .map_err(|error| ToolError::new("path_denied", error.to_string()))?;
    verify_opened_target(file.into_std(), expected)
}

fn verify_opened_target(
    file: std::fs::File,
    expected: &cap_std::fs::Metadata,
) -> Result<std::fs::File, ToolError> {
    let actual = file
        .metadata()
        .map_err(|error| ToolError::new("path_denied", error.to_string()))?;
    if !actual.is_file() || !identity_matches_std(identity_from_cap(expected), &actual) {
        return Err(ToolError::new(
            "path_denied",
            "target changed or is no longer a regular file",
        ));
    }
    Ok(file)
}

pub(super) fn read_bounded(
    mut reader: impl Read,
    max_bytes: usize,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<Vec<u8>, ToolError> {
    let target = max_bytes.saturating_add(1);
    let mut bytes = Vec::with_capacity(target.min(8 * 1024));
    let mut buffer = [0_u8; 8 * 1024];
    while bytes.len() < target {
        ensure_active(cancellation)?;
        let length = (target - bytes.len()).min(buffer.len());
        let read = reader
            .read(&mut buffer[..length])
            .map_err(|error| ToolError::new("read_failed", error.to_string()))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
    ensure_active(cancellation)?;
    Ok(bytes)
}
