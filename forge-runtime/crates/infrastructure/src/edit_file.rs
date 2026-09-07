use std::{
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use crate::runtime_domain::{
    AgentTool, Capability, TOOL_EFFECT_UNCERTAIN_CODE, ToolContext, ToolError, ToolFuture,
    ToolOutput, ToolSpec,
};
use cap_std::{
    ambient_authority,
    fs::{Dir, Permissions},
};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const MAX_INPUT_BYTES: usize = 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;

#[path = "edit_file_commit.rs"]
mod commit;
use commit::{
    TargetIdentity, identity_from_cap, verify_replacement_metadata, verify_staged_ownership,
    verify_target_unchanged,
};
#[path = "edit_file_read.rs"]
mod read;
#[cfg(test)]
use read::read_bounded;
use read::read_text;
#[path = "edit_file_metadata.rs"]
mod metadata;
#[path = "edit_file_stage.rs"]
mod stage;
#[cfg(test)]
use stage::create_temp;
use stage::{
    cleanup_if_owned, stage_file, sync_parent, verify_staged_metadata, verify_staged_name,
};

#[derive(Clone)]
pub struct EditFileTool {
    workspace: Arc<Dir>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EditFileInput {
    path: String,
    old_text: Option<String>,
    new_text: String,
}

#[derive(Serialize)]
struct EditFileResult<'a> {
    path: &'a str,
    before_sha256: Option<&'a str>,
    after_sha256: &'a str,
}

struct EditPlan {
    contents: String,
    before_sha256: Option<String>,
    before_identity: Option<TargetIdentity>,
    after_sha256: String,
    permissions: Option<Permissions>,
}

impl EditFileTool {
    /// Opens a write capability anchored to `workspace`.
    ///
    /// # Errors
    /// Returns `workspace_unavailable` when the workspace directory cannot be opened.
    pub fn open(workspace: &Path) -> Result<Self, ToolError> {
        let directory = Dir::open_ambient_dir(workspace, ambient_authority())
            .map_err(|error| ToolError::new("workspace_unavailable", error.to_string()))?;
        Ok(Self::from_anchored(Arc::new(directory)))
    }

    pub(crate) fn from_anchored(workspace: Arc<Dir>) -> Self {
        Self { workspace }
    }
}

impl AgentTool for EditFileTool {
    fn spec(&self) -> ToolSpec {
        let schema = serde_json::to_value(schema_for!(EditFileInput))
            .expect("generated edit-file schema is serializable");
        ToolSpec {
            name: "edit_file".into(),
            description: "Replace one unique exact text occurrence, or create a new UTF-8 file."
                .into(),
            input_schema: schema,
            capability: Capability::WorkspaceWrite,
        }
    }

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        execute_edit(arguments, context, self.workspace.clone(), MAX_INPUT_BYTES)
    }
}

fn execute_edit(
    arguments: Value,
    context: ToolContext,
    workspace: Arc<Dir>,
    max_input_bytes: usize,
) -> ToolFuture<'static> {
    Box::pin(async move {
        let input = parse_input(arguments, max_input_bytes)?;
        if context.cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let cancellation = context.cancellation;
        let max_output_bytes = context.max_output_bytes;
        tokio::task::spawn_blocking(move || {
            edit_blocking(
                &workspace,
                input,
                &cancellation,
                max_input_bytes,
                max_output_bytes,
            )
        })
        .await
        .map_err(blocking_task_failure)?
    })
}

fn blocking_task_failure(error: impl std::fmt::Display) -> ToolError {
    ToolError::new(
        TOOL_EFFECT_UNCERTAIN_CODE,
        format!("edit worker ended abnormally after effect ownership transferred: {error}"),
    )
}

fn parse_input(arguments: Value, max_bytes: usize) -> Result<EditFileInput, ToolError> {
    let input: EditFileInput = serde_json::from_value(arguments)
        .map_err(|error| ToolError::new("invalid_arguments", error.to_string()))?;
    validate_path(Path::new(&input.path), input.path.len())?;
    let old_bytes = input.old_text.as_ref().map_or(0, String::len);
    let input_bytes = input
        .path
        .len()
        .checked_add(old_bytes)
        .and_then(|size| size.checked_add(input.new_text.len()))
        .ok_or_else(input_limit)?;
    if input_bytes > max_bytes {
        return Err(input_limit());
    }
    Ok(input)
}

fn validate_path(path: &Path, path_bytes: usize) -> Result<(), ToolError> {
    if path.as_os_str().is_empty() || path.is_absolute() || path_bytes > MAX_PATH_BYTES {
        return Err(ToolError::new(
            "invalid_path",
            "path must be a non-empty workspace-relative path of at most 4096 bytes",
        ));
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) || path.file_name().is_none()
    {
        return Err(ToolError::new(
            "invalid_path",
            "path traversal and directory-only paths are not allowed",
        ));
    }
    Ok(())
}

fn edit_blocking(
    workspace: &Dir,
    input: EditFileInput,
    cancellation: &forge_runtime_domain::Cancellation,
    max_input_bytes: usize,
    max_output_bytes: usize,
) -> Result<ToolOutput, ToolError> {
    if cancellation.is_cancelled() {
        return Err(cancelled());
    }
    let path = Path::new(&input.path);
    let parent_path = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let parent = workspace
        .open_dir(parent_path.unwrap_or_else(|| Path::new(".")))
        .map_err(|error| ToolError::new("path_denied", error.to_string()))?;
    let target = PathBuf::from(path.file_name().expect("validated file name"));
    let plan = prepare_plan(
        &parent,
        &target,
        input.old_text.as_deref(),
        input.new_text,
        max_input_bytes,
        cancellation,
    )?;
    let output = encode_output(&input.path, &plan, max_output_bytes)?;
    if cancellation.is_cancelled() {
        return Err(cancelled());
    }
    persist(&parent, &target, &plan, cancellation)?;
    Ok(ToolOutput {
        content: output,
        truncated: false,
    })
}

fn prepare_plan(
    parent: &Dir,
    target: &Path,
    old_text: Option<&str>,
    new_text: String,
    max_bytes: usize,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<EditPlan, ToolError> {
    match parent.symlink_metadata(target) {
        Ok(_) if old_text.is_none() => Err(ToolError::new(
            "path_exists",
            "creation requires a path that does not already exist",
        )),
        Ok(metadata) => prepare_replacement(
            parent,
            target,
            &metadata,
            old_text.expect("replacement text is present"),
            &new_text,
            max_bytes,
            cancellation,
        ),
        Err(error) if error.kind() == ErrorKind::NotFound && old_text.is_none() => {
            prepare_creation(new_text, max_bytes)
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Err(ToolError::new(
            "path_not_found",
            "replacement requires an existing file",
        )),
        Err(error) => Err(ToolError::new("path_denied", error.to_string())),
    }
}

fn prepare_creation(contents: String, max_bytes: usize) -> Result<EditPlan, ToolError> {
    validate_result_size(contents.len(), max_bytes)?;
    let after_sha256 = sha256(contents.as_bytes());
    Ok(EditPlan {
        contents,
        before_sha256: None,
        before_identity: None,
        after_sha256,
        permissions: None,
    })
}

fn prepare_replacement(
    parent: &Dir,
    target: &Path,
    metadata: &cap_std::fs::Metadata,
    old_text: &str,
    new_text: &str,
    max_bytes: usize,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<EditPlan, ToolError> {
    ensure_regular(metadata)?;
    let original = read_text(parent, target, metadata, max_bytes, cancellation)?;
    let contents = replace_unique(&original, old_text, new_text)?;
    validate_result_size(contents.len(), max_bytes)?;
    Ok(EditPlan {
        before_sha256: Some(sha256(original.as_bytes())),
        before_identity: Some(identity_from_cap(metadata)),
        after_sha256: sha256(contents.as_bytes()),
        contents,
        permissions: Some(metadata.permissions()),
    })
}

fn ensure_regular(metadata: &cap_std::fs::Metadata) -> Result<(), ToolError> {
    if metadata.is_symlink() {
        return Err(ToolError::new(
            "path_denied",
            "symbolic-link targets are not editable",
        ));
    }
    if !metadata.is_file() {
        return Err(ToolError::new("not_a_file", "path is not a regular file"));
    }
    Ok(())
}

fn replace_unique(source: &str, old_text: &str, new_text: &str) -> Result<String, ToolError> {
    let mut matches = source.match_indices(old_text);
    let (start, _) = matches.next().ok_or_else(|| {
        ToolError::new("old_text_not_found", "old_text does not occur in the file")
    })?;
    if matches.next().is_some() {
        return Err(ToolError::new(
            "old_text_not_unique",
            "old_text occurs more than once in the file",
        ));
    }
    let mut result =
        String::with_capacity(source.len().saturating_sub(old_text.len()) + new_text.len());
    result.push_str(&source[..start]);
    result.push_str(new_text);
    result.push_str(&source[start + old_text.len()..]);
    Ok(result)
}

fn encode_output(path: &str, plan: &EditPlan, max_bytes: usize) -> Result<String, ToolError> {
    let result = EditFileResult {
        path,
        before_sha256: plan.before_sha256.as_deref(),
        after_sha256: &plan.after_sha256,
    };
    let output = serde_json::to_string(&result)
        .map_err(|error| ToolError::new("output_failed", error.to_string()))?;
    if output.len() > max_bytes {
        return Err(ToolError::new(
            "output_limit",
            format!("edit result exceeds the {max_bytes}-byte output limit"),
        ));
    }
    Ok(output)
}

fn persist(
    parent: &Dir,
    target: &Path,
    plan: &EditPlan,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<(), ToolError> {
    let (temp, staged_file) = stage_file(parent, plan.contents.as_bytes())?;
    if plan.before_sha256.is_none() {
        return persist_creation(parent, target, &temp, &staged_file, cancellation);
    }

    let expected = plan.before_identity.expect("replacement identity");
    let renamed = verify_staged_ownership(&staged_file, expected)
        .and_then(|()| verify_target_unchanged(parent, target, plan, cancellation))
        .and_then(|()| {
            ensure_active(cancellation)?;
            metadata::ensure_staged_metadata_safe(&staged_file)?;
            verify_staged_name(parent, &temp, &staged_file)?;
            parent
                .rename(&temp, parent, target)
                .map_err(|_| uncertain_commit())
        });
    if let Err(error) = renamed {
        cleanup_if_owned(parent, &temp, &staged_file)?;
        return Err(error);
    }

    confirm_staged_target(parent, target, &staged_file)?;

    let permissions = plan
        .permissions
        .clone()
        .expect("replacement permissions are present");
    staged_file
        .set_permissions(permissions)
        .and_then(|()| staged_file.sync_all())
        .map_err(|_| {
            ToolError::new(
                TOOL_EFFECT_UNCERTAIN_CODE,
                "replacement was committed but restoring its permissions could not be confirmed",
            )
        })?;
    verify_replacement_metadata(&staged_file, expected).map_err(|_| uncertain_commit())?;
    metadata::ensure_staged_metadata_safe(&staged_file).map_err(|_| uncertain_commit())?;
    verify_staged_metadata(parent, target, &staged_file).map_err(|_| uncertain_commit())?;
    sync_committed_parent(parent)
}

fn persist_creation(
    parent: &Dir,
    target: &Path,
    temp: &Path,
    staged_file: &cap_std::fs::File,
    cancellation: &forge_runtime_domain::Cancellation,
) -> Result<(), ToolError> {
    let committed = ensure_active(cancellation)
        .and_then(|()| metadata::ensure_parent_acl_safe(parent))
        .and_then(|()| metadata::ensure_staged_metadata_safe(staged_file))
        .and_then(|()| verify_staged_name(parent, temp, staged_file))
        .and_then(|()| {
            parent
                .hard_link(temp, parent, target)
                .map_err(|_| uncertain_commit())
        });
    if let Err(error) = committed {
        cleanup_if_owned(parent, temp, staged_file)?;
        return Err(error);
    }
    confirm_staged_target(parent, target, staged_file)?;
    cleanup_committed_temp(parent, temp, staged_file)?;
    sync_committed_parent(parent)
}

fn confirm_staged_target(
    parent: &Dir,
    target: &Path,
    staged_file: &cap_std::fs::File,
) -> Result<(), ToolError> {
    metadata::ensure_staged_metadata_safe(staged_file).map_err(|_| uncertain_commit())?;
    verify_staged_name(parent, target, staged_file).map_err(|_| uncertain_commit())
}

fn cleanup_committed_temp(
    parent: &Dir,
    temp: &Path,
    staged_file: &cap_std::fs::File,
) -> Result<(), ToolError> {
    metadata::ensure_staged_metadata_safe(staged_file).map_err(|_| uncertain_commit())?;
    verify_staged_metadata(parent, temp, staged_file).map_err(|_| uncertain_commit())?;
    parent.remove_file(temp).map_err(|_| uncertain_commit())
}

fn sync_committed_parent(parent: &Dir) -> Result<(), ToolError> {
    sync_parent(parent).map_err(|_| uncertain_commit())
}

fn uncertain_commit() -> ToolError {
    ToolError::new(
        TOOL_EFFECT_UNCERTAIN_CODE,
        "edit commit or durable directory state could not be confirmed",
    )
}

fn cancelled() -> ToolError {
    ToolError::new("cancelled", "run was cancelled")
}

fn ensure_active(cancellation: &forge_runtime_domain::Cancellation) -> Result<(), ToolError> {
    if cancellation.is_cancelled() {
        Err(cancelled())
    } else {
        Ok(())
    }
}

fn validate_result_size(size: usize, max_bytes: usize) -> Result<(), ToolError> {
    if size > max_bytes {
        return Err(ToolError::new(
            "file_too_large",
            format!("edited file exceeds the {max_bytes}-byte edit limit"),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn input_limit() -> ToolError {
    ToolError::new(
        "input_limit",
        "edit input exceeds the configured byte limit",
    )
}

#[cfg(test)]
#[path = "edit_file_metadata_tests.rs"]
mod metadata_tests;
#[cfg(test)]
#[path = "edit_file_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "edit_file_uncertainty_tests.rs"]
mod uncertainty_tests;
