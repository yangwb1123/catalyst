use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use cap_std::{
    ambient_authority,
    fs::{Dir, File},
};
use forge_runtime_domain::{
    Cancellation, ToolError, ToolOutput, WorkspaceIdentity, WorkspaceOpenError,
    WorkspaceReadCapability, WorkspaceReadFactory, WorkspaceReader,
};

use crate::{
    edit_file::EditFileTool, exec_command::ExecCommandTool, list_files::ListFilesTool,
    search_text::SearchTextTool,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct CapStdWorkspaceFactory;

#[derive(Clone)]
pub struct CapStdAgentWorkspace {
    directory: Arc<Dir>,
    identity: WorkspaceIdentity,
    workspace_path: PathBuf,
}

impl CapStdAgentWorkspace {
    /// Opens one descriptor-anchored workspace bundle for a first-party Agent.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory cannot be opened or the host cannot
    /// provide a stable filesystem identity.
    pub fn open(workspace: &Path) -> Result<Self, WorkspaceOpenError> {
        Self::open_selected(workspace)
    }

    /// Opens and binds one selected path to a canonical, descriptor-anchored workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when the directory cannot be opened, its canonical path
    /// cannot be resolved, or the selection changes during the binding checks.
    pub fn open_selected(workspace: &Path) -> Result<Self, WorkspaceOpenError> {
        open_selected_with(workspace, || {})
    }

    /// Returns the canonical path proven to identify the anchored workspace.
    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.workspace_path
    }

    #[must_use]
    pub const fn workspace_identity(&self) -> &WorkspaceIdentity {
        &self.identity
    }

    #[must_use]
    pub fn edit_file_tool(&self) -> EditFileTool {
        EditFileTool::from_anchored(self.directory.clone())
    }

    #[must_use]
    pub fn exec_command_tool(&self) -> ExecCommandTool {
        ExecCommandTool::from_anchored(self.directory.clone(), self.workspace_path.clone())
    }

    #[must_use]
    pub fn list_files_tool(&self) -> ListFilesTool {
        ListFilesTool::from_anchored(self.directory.clone())
    }

    #[must_use]
    pub fn search_text_tool(&self) -> SearchTextTool {
        SearchTextTool::from_anchored(self.directory.clone())
    }
}

fn open_selected_with(
    workspace: &Path,
    after_descriptor_opened: impl FnOnce(),
) -> Result<CapStdAgentWorkspace, WorkspaceOpenError> {
    let directory = open_directory(workspace)?;
    let identity = required_identity(&directory)?;
    after_descriptor_opened();
    let canonical_path = canonical_workspace_path(workspace)?;
    verify_path_binding(workspace, &canonical_path, &identity)?;
    if required_identity(&directory)? != identity {
        return Err(selection_changed());
    }
    Ok(CapStdAgentWorkspace {
        directory,
        identity,
        workspace_path: canonical_path,
    })
}

fn canonical_workspace_path(workspace: &Path) -> Result<PathBuf, WorkspaceOpenError> {
    fs::canonicalize(workspace).map_err(|error| {
        WorkspaceOpenError::new(format!("workspace canonicalization failed: {error}"))
    })
}

fn verify_path_binding(
    selected: &Path,
    canonical: &Path,
    expected: &WorkspaceIdentity,
) -> Result<(), WorkspaceOpenError> {
    let canonical_directory = open_directory(canonical).map_err(|_| selection_changed())?;
    if required_identity(&canonical_directory)? != *expected {
        return Err(selection_changed());
    }
    let final_canonical = canonical_workspace_path(selected).map_err(|_| selection_changed())?;
    if final_canonical != canonical {
        return Err(selection_changed());
    }
    let selected_directory = open_directory(selected).map_err(|_| selection_changed())?;
    if required_identity(&selected_directory)? != *expected {
        return Err(selection_changed());
    }
    Ok(())
}

fn required_identity(directory: &Dir) -> Result<WorkspaceIdentity, WorkspaceOpenError> {
    directory_identity(directory)?.ok_or_else(|| {
        WorkspaceOpenError::new("stable workspace identity is unavailable on this host")
    })
}

fn selection_changed() -> WorkspaceOpenError {
    WorkspaceOpenError::new("workspace selection changed while it was being opened")
}

impl WorkspaceReadFactory for CapStdWorkspaceFactory {
    fn open(&self, workspace: &Path) -> Result<WorkspaceReadCapability, WorkspaceOpenError> {
        let directory = open_directory(workspace)?;
        let identity = directory_identity(&directory)?;
        Ok(read_capability(directory, identity))
    }
}

impl WorkspaceReadFactory for CapStdAgentWorkspace {
    fn open(&self, workspace: &Path) -> Result<WorkspaceReadCapability, WorkspaceOpenError> {
        if workspace != self.workspace_path {
            return Err(WorkspaceOpenError::new(
                "runtime workspace path does not match the anchored Agent workspace",
            ));
        }
        Ok(read_capability(
            self.directory.clone(),
            Some(self.identity.clone()),
        ))
    }
}

fn open_directory(workspace: &Path) -> Result<Arc<Dir>, WorkspaceOpenError> {
    Dir::open_ambient_dir(workspace, ambient_authority())
        .map(Arc::new)
        .map_err(|error| WorkspaceOpenError::new(error.to_string()))
}

fn directory_identity(directory: &Dir) -> Result<Option<WorkspaceIdentity>, WorkspaceOpenError> {
    let metadata = directory
        .dir_metadata()
        .map_err(|error| WorkspaceOpenError::new(error.to_string()))?;
    #[cfg(unix)]
    return Ok(Some(platform_identity(&metadata)));
    #[cfg(windows)]
    return Ok(platform_identity(&metadata));
    #[cfg(not(any(unix, windows)))]
    Ok(None)
}

fn read_capability(
    directory: Arc<Dir>,
    identity: Option<WorkspaceIdentity>,
) -> WorkspaceReadCapability {
    WorkspaceReadCapability::new(Arc::new(CapStdWorkspaceReader { directory }), identity)
}

#[cfg(unix)]
fn platform_identity(metadata: &cap_std::fs::Metadata) -> WorkspaceIdentity {
    use cap_std::fs::MetadataExt;

    WorkspaceIdentity::Unix {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}

#[cfg(windows)]
fn platform_identity(metadata: &cap_std::fs::Metadata) -> Option<WorkspaceIdentity> {
    use cap_std::fs::MetadataExt;

    Some(WorkspaceIdentity::Windows {
        volume_serial_number: metadata.volume_serial_number()?,
        file_index: metadata.file_index()?,
    })
}

struct CapStdWorkspaceReader {
    directory: Arc<Dir>,
}

impl WorkspaceReader for CapStdWorkspaceReader {
    fn read_file(
        &self,
        relative: &Path,
        max_bytes: usize,
        cancellation: &Cancellation,
    ) -> Result<ToolOutput, ToolError> {
        validate_relative(relative)?;
        read_from_directory(&self.directory, relative, max_bytes, cancellation)
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
    cancellation: &Cancellation,
) -> Result<ToolOutput, ToolError> {
    ensure_active(cancellation)?;
    let file = open_for_read(directory, relative)?;
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
    let bytes = read_bounded(file, max_bytes, cancellation)?;
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

#[cfg(unix)]
fn open_for_read(directory: &Dir, relative: &Path) -> Result<File, ToolError> {
    use cap_std::fs::{OpenOptions, OpenOptionsExt as _};

    let flags =
        rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let mut options = OpenOptions::new();
    options.read(true).custom_flags(flags.bits().cast_signed());
    directory
        .open_with(relative, &options)
        .map_err(|error| ToolError::new("path_denied", error.to_string()))
}

#[cfg(not(unix))]
fn open_for_read(directory: &Dir, relative: &Path) -> Result<File, ToolError> {
    directory
        .open(relative)
        .map_err(|error| ToolError::new("path_denied", error.to_string()))
}

fn read_bounded(
    mut reader: impl Read,
    max_bytes: usize,
    cancellation: &Cancellation,
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

fn ensure_active(cancellation: &Cancellation) -> Result<(), ToolError> {
    if cancellation.is_cancelled() {
        Err(ToolError::new("cancelled", "run was cancelled"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Read};

    use forge_runtime_domain::Cancellation;
    use tempfile::TempDir;

    struct CancellingReader {
        cancellation: Cancellation,
        remaining: usize,
    }

    impl Read for CancellingReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let length = self.remaining.min(buffer.len());
            buffer[..length].fill(b'x');
            self.remaining -= length;
            self.cancellation.cancel();
            Ok(length)
        }
    }

    #[test]
    fn read_file_reader_checks_cancellation_between_chunks() {
        let cancellation = Cancellation::default();
        let reader = CancellingReader {
            cancellation: cancellation.clone(),
            remaining: 16 * 1024,
        };

        let error = super::read_bounded(reader, 16 * 1024, &cancellation)
            .expect_err("cancellation stops the next chunk");

        assert_eq!(error.code, "cancelled");
    }

    #[test]
    fn selected_workspace_captures_its_proven_canonical_path() {
        let root = TempDir::new().expect("temporary workspace");
        let selected = root.path().join(".");

        let workspace = super::CapStdAgentWorkspace::open_selected(&selected)
            .expect("stable selected workspace");

        assert_eq!(
            workspace.canonical_path(),
            fs::canonicalize(root.path()).expect("canonical fixture")
        );
    }

    #[cfg(unix)]
    #[test]
    fn selected_workspace_rejects_replacement_after_descriptor_open() {
        let root = TempDir::new().expect("temporary root");
        let selected = root.path().join("selected");
        let original = root.path().join("original");
        fs::create_dir(&selected).expect("selected workspace");

        let result = super::open_selected_with(&selected, || {
            fs::rename(&selected, &original).expect("move selected workspace");
            fs::create_dir(&selected).expect("replacement workspace");
        });
        let error = result.err().expect("replacement must fail closed");

        assert_eq!(
            error.to_string(),
            "workspace selection changed while it was being opened"
        );
        assert!(original.is_dir());
        assert!(selected.is_dir());
    }
}
