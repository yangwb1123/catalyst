use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use forge_runtime_domain::{AgentTool, Capability, ToolContext, ToolError, ToolFuture, ToolSpec};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default)]
pub struct ReadFileTool;

#[derive(Clone, Debug)]
pub struct AllowlistedReadFileTool {
    allowed_paths: Arc<BTreeSet<String>>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadFileInput {
    path: String,
}

impl ReadFileTool {
    #[must_use]
    pub fn restricted(allowed_paths: impl IntoIterator<Item = String>) -> AllowlistedReadFileTool {
        AllowlistedReadFileTool {
            allowed_paths: Arc::new(allowed_paths.into_iter().collect()),
        }
    }
}

impl AgentTool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        read_file_spec("Read one UTF-8 text file inside the workspace.")
    }

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        execute_read(arguments, context, None)
    }
}

impl AgentTool for AllowlistedReadFileTool {
    fn spec(&self) -> ToolSpec {
        read_file_spec("Read one explicitly allowed UTF-8 text file inside the workspace.")
    }

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        execute_read(arguments, context, Some(self.allowed_paths.clone()))
    }
}

fn read_file_spec(description: &str) -> ToolSpec {
    let schema = serde_json::to_value(schema_for!(ReadFileInput))
        .expect("generated read-file schema is serializable");
    ToolSpec {
        name: "read_file".into(),
        description: description.into(),
        input_schema: schema,
        capability: Capability::WorkspaceRead,
    }
}

fn execute_read(
    arguments: Value,
    context: ToolContext,
    allowed_paths: Option<Arc<BTreeSet<String>>>,
) -> ToolFuture<'static> {
    Box::pin(async move {
        let input = parse_input(arguments)?;
        if allowed_paths
            .as_ref()
            .is_some_and(|paths| !paths.contains(&input.path))
        {
            return Err(ToolError::new(
                "path_not_allowlisted",
                "path was not explicitly allowed for this run",
            ));
        }
        if context.cancellation.is_cancelled() {
            return Err(ToolError::new("cancelled", "run was cancelled"));
        }
        let relative = PathBuf::from(input.path);
        let workspace = context.workspace;
        let max_bytes = context.max_output_bytes;
        tokio::task::spawn_blocking(move || workspace.read_file(&relative, max_bytes))
            .await
            .map_err(|error| ToolError::new("read_task_failed", error.to_string()))?
    })
}

fn parse_input(arguments: Value) -> Result<ReadFileInput, ToolError> {
    serde_json::from_value(arguments)
        .map_err(|error| ToolError::new("invalid_arguments", error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use forge_runtime_domain::{AgentTool, Cancellation, ToolContext, WorkspaceReadFactory as _};
    use serde_json::json;
    use tempfile::TempDir;

    use super::ReadFileTool;
    use crate::CapStdWorkspaceFactory;

    fn context(root: &Path) -> ToolContext {
        ToolContext {
            workspace: CapStdWorkspaceFactory
                .open(root)
                .expect("workspace capability"),
            cancellation: Cancellation::default(),
            max_output_bytes: 1024,
        }
    }

    #[tokio::test]
    async fn reads_a_file_inside_the_workspace() {
        let root = TempDir::new().expect("temporary workspace");
        fs::write(root.path().join("note.txt"), "hello").expect("fixture file");
        let output = ReadFileTool
            .execute(json!({ "path": "note.txt" }), context(root.path()))
            .await
            .expect("read succeeds");
        assert_eq!(output.content, "hello");
    }

    #[tokio::test]
    async fn restricted_tool_reads_only_the_exact_allowlisted_path() {
        let root = TempDir::new().expect("temporary workspace");
        fs::write(root.path().join("note.txt"), "hello").expect("fixture file");
        let tool = ReadFileTool::restricted(["note.txt".to_owned()]);

        let output = tool
            .execute(json!({ "path": "note.txt" }), context(root.path()))
            .await
            .expect("allowlisted read succeeds");
        let alias_error = tool
            .execute(json!({ "path": "./note.txt" }), context(root.path()))
            .await
            .expect_err("a non-exact alias is denied");

        assert_eq!(output.content, "hello");
        assert_eq!(alias_error.code, "path_not_allowlisted");
    }

    #[tokio::test]
    async fn restricted_tool_denies_unlisted_sensitive_paths() {
        let root = TempDir::new().expect("temporary workspace");
        fs::write(root.path().join(".env"), "SECRET=fixture").expect("sensitive fixture");
        let tool = ReadFileTool::restricted(["README.md".to_owned()]);

        for path in [".env", "proc/self/environ"] {
            let error = tool
                .execute(json!({ "path": path }), context(root.path()))
                .await
                .expect_err("an unlisted path is denied before workspace access");
            assert_eq!(error.code, "path_not_allowlisted");
        }
    }

    #[tokio::test]
    async fn rejects_parent_traversal() {
        let root = TempDir::new().expect("temporary workspace");
        let error = ReadFileTool
            .execute(json!({ "path": "../secret" }), context(root.path()))
            .await
            .expect_err("traversal is denied");
        assert_eq!(error.code, "invalid_path");
    }

    #[tokio::test]
    async fn rejects_a_file_larger_than_the_output_limit() {
        let root = TempDir::new().expect("temporary workspace");
        fs::write(root.path().join("large.txt"), "12345").expect("fixture file");
        let mut limited = context(root.path());
        limited.max_output_bytes = 4;
        let error = ReadFileTool
            .execute(json!({ "path": "large.txt" }), limited)
            .await
            .expect_err("oversized file is denied");
        assert_eq!(error.code, "output_limit");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn rejects_a_symlink_that_escapes_the_workspace() {
        use std::os::unix::fs::symlink;

        let root = TempDir::new().expect("temporary workspace");
        let outside = TempDir::new().expect("outside directory");
        fs::write(outside.path().join("secret.txt"), "secret").expect("outside file");
        symlink(
            outside.path().join("secret.txt"),
            root.path().join("link.txt"),
        )
        .expect("symlink fixture");
        let error = ReadFileTool
            .execute(json!({ "path": "link.txt" }), context(root.path()))
            .await
            .expect_err("escaping symlink is denied");
        assert_eq!(error.code, "path_denied");
    }

    #[cfg(unix)]
    #[test]
    fn open_directory_capability_survives_workspace_path_replacement() {
        use std::os::unix::fs::symlink;

        let base = TempDir::new().expect("temporary base");
        let workspace = base.path().join("workspace");
        let moved = base.path().join("workspace-moved");
        let outside = base.path().join("outside");
        fs::create_dir(&workspace).expect("workspace directory");
        fs::create_dir(&outside).expect("outside directory");
        fs::write(workspace.join("note.txt"), "inside").expect("inside fixture");
        fs::write(outside.join("note.txt"), "outside").expect("outside fixture");
        let capability = CapStdWorkspaceFactory
            .open(&workspace)
            .expect("workspace capability");

        fs::rename(&workspace, &moved).expect("move workspace path");
        symlink(&outside, &workspace).expect("replace workspace path");
        let output = capability
            .read_file(Path::new("note.txt"), 1024)
            .expect("open handle remains confined");

        assert_eq!(output.content, "inside");
    }
}
