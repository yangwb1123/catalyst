use std::{
    io::Read,
    path::{Component, Path},
    sync::Arc,
};

use cap_std::{ambient_authority, fs::Dir};
use forge_runtime_domain::{
    ToolError, ToolOutput, WorkspaceOpenError, WorkspaceReadCapability, WorkspaceReadFactory,
    WorkspaceReader,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct CapStdWorkspaceFactory;

impl WorkspaceReadFactory for CapStdWorkspaceFactory {
    fn open(&self, workspace: &Path) -> Result<WorkspaceReadCapability, WorkspaceOpenError> {
        let directory = Dir::open_ambient_dir(workspace, ambient_authority())
            .map_err(|error| WorkspaceOpenError::new(error.to_string()))?;
        Ok(WorkspaceReadCapability::new(Arc::new(
            CapStdWorkspaceReader { directory },
        )))
    }
}

struct CapStdWorkspaceReader {
    directory: Dir,
}

impl WorkspaceReader for CapStdWorkspaceReader {
    fn read_file(&self, relative: &Path, max_bytes: usize) -> Result<ToolOutput, ToolError> {
        validate_relative(relative)?;
        read_from_directory(&self.directory, relative, max_bytes)
    }
}

fn validate_relative(path: &Path) -> Result<(), ToolError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(ToolError::new(
            "invalid_path",
            "path must be a non-empty workspace-relative path",
        ));
    }
    let forbidden = path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    });
    if forbidden {
        return Err(ToolError::new(
            "invalid_path",
            "path traversal is not allowed",
        ));
    }
    Ok(())
}

fn read_from_directory(
    directory: &Dir,
    relative: &Path,
    max_bytes: usize,
) -> Result<ToolOutput, ToolError> {
    let file = directory
        .open(relative)
        .map_err(|error| ToolError::new("path_denied", error.to_string()))?;
    let metadata = file
        .metadata()
        .map_err(|error| ToolError::new("file_unavailable", error.to_string()))?;
    if !metadata.is_file() {
        return Err(ToolError::new("not_a_file", "path is not a regular file"));
    }
    let byte_limit = u64::try_from(max_bytes).unwrap_or(u64::MAX);
    if metadata.len() > byte_limit {
        return Err(ToolError::new(
            "output_limit",
            format!("file exceeds the {max_bytes}-byte output limit"),
        ));
    }
    let read_limit = byte_limit.saturating_add(1);
    let mut reader = file.take(read_limit);
    let mut bytes = Vec::with_capacity(max_bytes.min(8 * 1024).saturating_add(1));
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| ToolError::new("read_failed", error.to_string()))?;
    if bytes.len() > max_bytes {
        return Err(ToolError::new(
            "output_limit",
            format!("file exceeds the {max_bytes}-byte output limit"),
        ));
    }
    let content = String::from_utf8(bytes)
        .map_err(|error| ToolError::new("invalid_encoding", error.to_string()))?;
    Ok(ToolOutput {
        content,
        truncated: false,
    })
}
