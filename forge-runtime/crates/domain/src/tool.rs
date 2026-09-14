use std::{future::Future, pin::Pin};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Cancellation, WorkspaceReadCapability,
    execution::fabric::{ExecutionEvidence, ToolInvocationRef},
};

/// A started effect could not be proven stopped or completed. The journal must
/// retain its pending `ToolStarted` fence instead of recording a result.
pub const TOOL_EFFECT_UNCERTAIN_CODE: &str = "tool_effect_uncertain";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    WorkspaceRead,
    WorkspaceWrite,
    Process,
    Network,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub capability: Capability,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolOutput {
    pub content: String,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolError {
    pub code: String,
    pub message: String,
}

impl ToolError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ToolError {}

#[derive(Clone)]
pub struct ToolContext {
    pub workspace: WorkspaceReadCapability,
    pub cancellation: Cancellation,
    pub max_output_bytes: usize,
}

pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<ToolOutput, ToolError>> + Send + 'a>>;

/// A tool result plus optional local execution-fabric observation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ToolExecutionResult {
    pub output: ToolOutput,
    pub execution_evidence: Option<ExecutionEvidence>,
}

pub type ToolExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ToolExecutionResult, ToolError>> + Send + 'a>>;

pub trait AgentTool: Send + Sync {
    fn spec(&self) -> ToolSpec;

    fn execute(&self, arguments: Value, context: ToolContext) -> ToolFuture<'_>;

    /// Executes one invocation with its existing Runtime event correlation.
    /// Tools that do not use the local execution ABI keep their current path.
    fn execute_with_invocation(
        &self,
        arguments: Value,
        context: ToolContext,
        _invocation: ToolInvocationRef,
    ) -> ToolFuture<'_> {
        self.execute(arguments, context)
    }

    /// Executes an invocation and returns optional execution-fabric evidence.
    /// Existing tools preserve their output-only behavior by default.
    fn execute_with_invocation_evidence(
        &self,
        arguments: Value,
        context: ToolContext,
        invocation: ToolInvocationRef,
    ) -> ToolExecutionFuture<'_> {
        let execution = self.execute_with_invocation(arguments, context, invocation);
        Box::pin(async move {
            execution.await.map(|output| ToolExecutionResult {
                output,
                execution_evidence: None,
            })
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User {
        text: String,
    },
    ProviderContext {
        provider: String,
        items: Vec<serde_json::Value>,
    },
    Assistant {
        text: String,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        call_id: String,
        name: String,
        output: String,
        is_error: bool,
        truncated: bool,
    },
}
