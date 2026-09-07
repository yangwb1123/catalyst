use std::sync::Arc;

use forge_runtime_application::RuntimeError;
use forge_runtime_domain::{
    AgentTool, Cancellation, Capability, ModelFinishReason, RuntimeEventKind,
    TOOL_EFFECT_UNCERTAIN_CODE, ToolContext, ToolError, ToolFuture, ToolSpec,
};
use forge_runtime_infrastructure::MemoryEventSink;
use serde_json::json;
use tempfile::TempDir;

mod support;

use support::{request, runtime, tool_call, tool_turn};

#[tokio::test]
async fn uncertain_effect_retains_the_pending_tool_fence_without_a_terminal_event() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = runtime(
        vec![tool_turn(
            vec![tool_call("call-1", "uncertain", json!({}))],
            ModelFinishReason::ToolUse,
        )],
        vec![Arc::new(UncertainWriteTool)],
    );
    let mut run_request = request(&root);
    run_request.allowed_capabilities = vec![Capability::WorkspaceWrite];
    let mut sink = MemoryEventSink::default();

    let error = runtime
        .run(run_request, Cancellation::default(), &mut sink)
        .await
        .expect_err("uncertain effect must remain nonterminal");

    assert!(matches!(error, RuntimeError::ToolEffectUncertain { .. }));
    assert!(
        sink.events()
            .iter()
            .any(|event| { matches!(event.kind, RuntimeEventKind::ToolStarted { .. }) })
    );
    assert!(!sink.events().iter().any(|event| {
        matches!(
            event.kind,
            RuntimeEventKind::ToolFinished { .. } | RuntimeEventKind::RunFinished { .. }
        )
    }));
}

struct UncertainWriteTool;

impl AgentTool for UncertainWriteTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "uncertain".into(),
            description: "Reports an unconfirmed effect cleanup.".into(),
            input_schema: json!({ "type": "object" }),
            capability: Capability::WorkspaceWrite,
        }
    }

    fn execute(&self, _arguments: serde_json::Value, _context: ToolContext) -> ToolFuture<'_> {
        Box::pin(async {
            Err(ToolError::new(
                TOOL_EFFECT_UNCERTAIN_CODE,
                "cleanup was not confirmed",
            ))
        })
    }
}
