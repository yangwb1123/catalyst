// Cargo builds each integration-test file as a separate crate, so each one
// intentionally consumes only part of this shared fixture module.
#![allow(dead_code)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_domain::{
    AgentTool, Capability, ModelEvent, ModelFinishReason, ProviderError, RunLimits, RunRequest,
    RuntimeEvent, RuntimeEventKind, ToolCall, ToolContext, ToolError, ToolFuture, ToolOutput,
    ToolSpec,
};
use forge_runtime_infrastructure::{CapStdWorkspaceFactory, ScriptedProvider};
use serde_json::{Value, json};
use tempfile::TempDir;

pub type ScriptedTurn = Vec<Result<ModelEvent, ProviderError>>;

pub fn completed(text: &str) -> ScriptedTurn {
    vec![
        Ok(ModelEvent::TextDelta { delta: text.into() }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ]
}

pub fn tool_turn(calls: Vec<ToolCall>, reason: ModelFinishReason) -> ScriptedTurn {
    calls
        .into_iter()
        .map(|call| Ok(ModelEvent::ToolCall { call }))
        .chain([Ok(ModelEvent::Finished { reason })])
        .collect()
}

pub fn tool_call(id: &str, name: &str, arguments: Value) -> ToolCall {
    ToolCall {
        id: id.into(),
        name: name.into(),
        arguments,
    }
}

pub fn request(root: &TempDir) -> RunRequest {
    RunRequest {
        session_id: "session-1".into(),
        run_id: "run-1".into(),
        prompt: "Exercise the runtime contract.".into(),
        system_prompt: "Use only the granted tools.".into(),
        workspace: root.path().to_path_buf(),
        allowed_capabilities: vec![Capability::WorkspaceRead],
        limits: RunLimits {
            max_turns: 4,
            max_tool_calls: 4,
            max_tool_output_bytes: 1024,
        },
    }
}

pub fn runtime(turns: Vec<ScriptedTurn>, tools: Vec<Arc<dyn AgentTool>>) -> AgentRuntime {
    let mut catalog = ToolCatalog::default();
    for tool in tools {
        catalog
            .register(tool)
            .expect("probe tool registration succeeds");
    }
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(turns)),
        catalog,
        Arc::new(CapStdWorkspaceFactory),
    )
}

#[derive(Debug)]
pub struct ProbeTool {
    name: String,
    capability: Capability,
    result: Result<ToolOutput, ToolError>,
    cancel_on_execute: bool,
    invocations: AtomicUsize,
}

impl ProbeTool {
    pub fn succeeds(name: &str, content: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            capability: Capability::WorkspaceRead,
            result: Ok(ToolOutput {
                content: content.into(),
                truncated: false,
            }),
            cancel_on_execute: false,
            invocations: AtomicUsize::new(0),
        })
    }

    pub fn fails(name: &str, code: &str, message: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            capability: Capability::WorkspaceRead,
            result: Err(ToolError::new(code, message)),
            cancel_on_execute: false,
            invocations: AtomicUsize::new(0),
        })
    }

    pub fn cancelling(name: &str, content: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            capability: Capability::WorkspaceRead,
            result: Ok(ToolOutput {
                content: content.into(),
                truncated: false,
            }),
            cancel_on_execute: true,
            invocations: AtomicUsize::new(0),
        })
    }

    pub fn invocation_count(&self) -> usize {
        self.invocations.load(Ordering::SeqCst)
    }
}

impl AgentTool for ProbeTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: self.name.clone(),
            description: "A deterministic contract-test probe.".into(),
            input_schema: json!({ "type": "object" }),
            capability: self.capability,
        }
    }

    fn execute(&self, _arguments: Value, context: ToolContext) -> ToolFuture<'_> {
        self.invocations.fetch_add(1, Ordering::SeqCst);
        if self.cancel_on_execute {
            context.cancellation.cancel();
        }
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

pub fn count_terminal_events(events: &[RuntimeEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event.kind, RuntimeEventKind::RunFinished { .. }))
        .count()
}
