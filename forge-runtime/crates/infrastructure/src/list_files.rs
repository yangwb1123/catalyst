use std::{collections::BTreeSet, path::Path, sync::Arc};

use cap_std::{ambient_authority, fs::Dir};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    runtime_domain::{
        AgentTool, Capability, ToolContext, ToolError, ToolFuture, ToolOutput, ToolSpec,
    },
    workspace_discovery::{
        IncompleteReason, MAX_TOOL_OUTPUT_BYTES, WalkDirective, WalkOptions, WalkStats,
        walk_regular_files,
    },
};

#[derive(Clone)]
pub struct ListFilesTool {
    workspace: Arc<Dir>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListFilesInput {
    path: Option<String>,
    max_depth: Option<usize>,
    max_entries: Option<usize>,
}

#[derive(Serialize)]
struct ListFilesOutput<'a> {
    files: &'a [String],
    complete: bool,
    reasons: &'a BTreeSet<IncompleteReason>,
    stats: &'a WalkStats,
}

impl ListFilesTool {
    /// Opens a discovery capability anchored to `workspace`.
    ///
    /// # Errors
    /// Returns `workspace_unavailable` when the directory cannot be opened.
    pub fn open(workspace: &Path) -> Result<Self, ToolError> {
        let directory = Dir::open_ambient_dir(workspace, ambient_authority())
            .map_err(|error| ToolError::new("workspace_unavailable", error.to_string()))?;
        Ok(Self::from_anchored(Arc::new(directory)))
    }

    pub(crate) fn from_anchored(workspace: Arc<Dir>) -> Self {
        Self { workspace }
    }
}

impl AgentTool for ListFilesTool {
    fn spec(&self) -> ToolSpec {
        let schema = serde_json::to_value(schema_for!(ListFilesInput))
            .expect("generated list-files schema is serializable");
        ToolSpec {
            name: "list_files".into(),
            description: "List regular workspace-relative file paths without following symbolic links or entering .git metadata; return completeness reasons and bounded scan statistics.".into(),
            input_schema: schema,
            capability: Capability::WorkspaceRead,
        }
    }

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        let workspace = self.workspace.clone();
        Box::pin(async move {
            let options = parse_input(arguments)?;
            let cancellation = context.cancellation;
            let output_limit = context.max_output_bytes.min(MAX_TOOL_OUTPUT_BYTES);
            tokio::task::spawn_blocking(move || {
                list_blocking(&workspace, &options, &cancellation, output_limit)
            })
            .await
            .map_err(|error| ToolError::new("list_task_failed", error.to_string()))?
        })
    }
}

fn parse_input(arguments: Value) -> Result<WalkOptions, ToolError> {
    let input: ListFilesInput = serde_json::from_value(arguments)
        .map_err(|error| ToolError::new("invalid_arguments", error.to_string()))?;
    WalkOptions::new(input.path, input.max_depth, input.max_entries)
}

fn list_blocking(
    workspace: &Arc<Dir>,
    options: &WalkOptions,
    cancellation: &crate::runtime_domain::Cancellation,
    output_limit: usize,
) -> Result<ToolOutput, ToolError> {
    let mut files = Vec::new();
    let summary = walk_regular_files(workspace, options, cancellation, |path, _parent, _name| {
        files.push(path.to_owned());
        Ok(WalkDirective::Continue)
    })?;
    files.sort_unstable();
    encode_output(&files, &summary.reasons, &summary.stats, output_limit)
}

fn encode_output(
    files: &[String],
    reasons: &BTreeSet<IncompleteReason>,
    stats: &WalkStats,
    output_limit: usize,
) -> Result<ToolOutput, ToolError> {
    let content = serialize_output(files, reasons, stats)?;
    if content.len() <= output_limit {
        return Ok(ToolOutput {
            content,
            truncated: !reasons.is_empty(),
        });
    }
    let mut limited_reasons = reasons.clone();
    limited_reasons.insert(IncompleteReason::OutputLimit);
    let count = largest_fitting_prefix(files, &limited_reasons, stats, output_limit)?;
    let content = serialize_output(&files[..count], &limited_reasons, stats)?;
    if content.len() > output_limit {
        return Err(output_limit_error());
    }
    Ok(ToolOutput {
        content,
        truncated: true,
    })
}

fn largest_fitting_prefix(
    files: &[String],
    reasons: &BTreeSet<IncompleteReason>,
    stats: &WalkStats,
    limit: usize,
) -> Result<usize, ToolError> {
    let (mut low, mut high) = (0, files.len());
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if serialize_output(&files[..middle], reasons, stats)?.len() <= limit {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    Ok(low)
}

fn serialize_output(
    files: &[String],
    reasons: &BTreeSet<IncompleteReason>,
    stats: &WalkStats,
) -> Result<String, ToolError> {
    let output = ListFilesOutput {
        files,
        complete: reasons.is_empty(),
        reasons,
        stats,
    };
    serde_json::to_string(&output)
        .map_err(|error| ToolError::new("output_encoding_failed", error.to_string()))
}

fn output_limit_error() -> ToolError {
    ToolError::new(
        "output_limit",
        "output limit is too small for the list-files result envelope",
    )
}

#[cfg(test)]
#[path = "list_files_tests.rs"]
mod tests;
