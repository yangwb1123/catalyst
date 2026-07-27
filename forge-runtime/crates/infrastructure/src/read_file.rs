use std::path::PathBuf;

use forge_runtime_domain::{AgentTool, Capability, ToolContext, ToolError, ToolFuture, ToolSpec};
use schemars::{JsonSchema, schema_for};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Default)]
pub struct ReadFileTool;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadFileInput {
    path: String,
}

impl AgentTool for ReadFileTool {
    fn spec(&self) -> ToolSpec {
        let schema = serde_json::to_value(schema_for!(ReadFileInput))
            .expect("generated read-file schema is serializable");
        ToolSpec {
            name: "read_file".into(),
            description: "Read one UTF-8 text file inside the workspace.".into(),
            input_schema: schema,
            capability: Capability::WorkspaceRead,
        }
    }

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        Box::pin(async move {
            let input = parse_input(arguments)?;
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
