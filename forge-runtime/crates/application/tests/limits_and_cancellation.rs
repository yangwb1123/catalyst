use std::{future, sync::Arc, time::Duration};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_domain::{
    AgentTool, Cancellation, Capability, LimitKind, Message, ModelEventStream, ModelFinishReason,
    ModelProvider, ModelRequest, RunOutcome, RuntimeEventKind, ToolContext, ToolFuture, ToolSpec,
};
use forge_runtime_infrastructure::{CapStdWorkspaceFactory, MemoryEventSink};
use futures_util::stream;
use serde_json::json;
use tempfile::TempDir;

mod support;

use support::{
    ProbeTool, completed, count_terminal_events, request, runtime, tool_call, tool_turn,
};

#[tokio::test]
async fn rejects_an_over_limit_tool_batch_without_executing_any_call() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = ProbeTool::succeeds("probe", "must not execute");
    let calls = vec![
        tool_call("call-1", "probe", json!({})),
        tool_call("call-2", "probe", json!({})),
    ];
    let runtime = runtime(
        vec![tool_turn(calls, ModelFinishReason::ToolUse)],
        vec![probe.clone()],
    );
    let mut limited_request = request(&root);
    limited_request.limits.max_tool_calls = 1;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(limited_request, Cancellation::default(), &mut sink)
        .await
        .expect("tool limit is a normal terminal outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ToolCalls
        }
    );
    assert_eq!(probe.invocation_count(), 0);
    assert_eq!(tool_error_count(&result.messages, "tool_call_limit"), 2);
    assert_eq!(
        sink.events()
            .iter()
            .filter(|event| matches!(event.kind, RuntimeEventKind::ToolRejected { .. }))
            .count(),
        2
    );
    assert!(
        !sink
            .events()
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ToolStarted { .. }))
    );
    assert_eq!(count_terminal_events(sink.events()), 1);
}

#[tokio::test]
async fn truncated_tool_calls_consume_the_shared_call_budget() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = ProbeTool::succeeds("probe", "must not execute");
    let runtime = runtime(
        vec![
            tool_turn(
                vec![tool_call("truncated", "probe", json!({}))],
                ModelFinishReason::Length,
            ),
            tool_turn(
                vec![tool_call("after", "probe", json!({}))],
                ModelFinishReason::ToolUse,
            ),
        ],
        vec![probe.clone()],
    );
    let mut limited_request = request(&root);
    limited_request.limits.max_tool_calls = 1;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(limited_request, Cancellation::default(), &mut sink)
        .await
        .expect("tool limit is a normal terminal outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ToolCalls
        }
    );
    assert_eq!(probe.invocation_count(), 0);
    assert!(sink.events().iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ToolRejected { call, code, .. }
            if call.id == "truncated" && code == "truncated_tool_call"
    )));
    assert!(sink.events().iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ToolRejected { call, code, .. }
            if call.id == "after" && code == "tool_call_limit"
    )));
    assert_eq!(count_terminal_events(sink.events()), 1);
}

#[tokio::test]
async fn zero_tool_call_budget_rejects_the_first_provider_call() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = ProbeTool::succeeds("probe", "must not execute");
    let runtime = runtime(
        vec![tool_turn(
            vec![tool_call("call-1", "probe", json!({}))],
            ModelFinishReason::ToolUse,
        )],
        vec![probe.clone()],
    );
    let mut limited_request = request(&root);
    limited_request.limits.max_tool_calls = 0;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(limited_request, Cancellation::default(), &mut sink)
        .await
        .expect("zero tool budget is a normal terminal outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ToolCalls
        }
    );
    assert_eq!(probe.invocation_count(), 0);
    assert!(sink.events().iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ToolRejected { call, code, .. }
            if call.id == "call-1" && code == "tool_call_limit"
    )));
    assert!(
        !sink
            .events()
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ToolStarted { .. }))
    );
    assert_eq!(count_terminal_events(sink.events()), 1);
}

#[tokio::test]
async fn a_pre_cancelled_run_never_starts_a_model_turn() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = runtime(vec![completed("must not be requested")], vec![]);
    let cancellation = Cancellation::default();
    cancellation.cancel();
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), cancellation, &mut sink)
        .await
        .expect("pre-cancellation is a normal terminal outcome");

    assert_eq!(result.outcome, RunOutcome::Cancelled);
    assert!(
        !sink
            .events()
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::TurnStarted { .. }))
    );
    assert_eq!(count_terminal_events(sink.events()), 1);
}

#[tokio::test]
async fn cancellation_between_tools_prevents_the_next_tool_from_executing() {
    let root = TempDir::new().expect("temporary workspace");
    let canceller = ProbeTool::cancelling("cancel_now", "cancelled");
    let never = ProbeTool::succeeds("must_not_run", "unexpected");
    let calls = vec![
        tool_call("call-1", "cancel_now", json!({})),
        tool_call("call-2", "must_not_run", json!({})),
    ];
    let tools: Vec<Arc<dyn AgentTool>> = vec![canceller.clone(), never.clone()];
    let runtime = runtime(vec![tool_turn(calls, ModelFinishReason::ToolUse)], tools);
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect("between-tool cancellation is a normal terminal outcome");

    assert_eq!(result.outcome, RunOutcome::Cancelled);
    assert_eq!(canceller.invocation_count(), 1);
    assert_eq!(never.invocation_count(), 0);
    let cancelled_tool = result.messages.iter().any(|message| {
        matches!(
            message,
            Message::Tool {
                name,
                output,
                is_error: true,
                ..
            } if name == "must_not_run" && output.contains("cancelled")
        )
    });
    assert!(cancelled_tool);
    assert!(!sink.events().iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ToolStarted { call } if call.name == "must_not_run"
    )));
    assert!(sink.events().iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ToolRejected { call, code, .. }
            if call.name == "must_not_run" && code == "cancelled"
    )));
    assert_eq!(count_terminal_events(sink.events()), 1);
}

#[tokio::test]
async fn cancellation_wakes_a_pending_model_stream_and_finishes_normally() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = AgentRuntime::new(
        Arc::new(PendingProvider),
        ToolCatalog::default(),
        Arc::new(CapStdWorkspaceFactory),
    );
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    let mut sink = MemoryEventSink::default();

    let run = runtime.run(request(&root), cancellation, &mut sink);
    let cancel_soon = async move {
        tokio::task::yield_now().await;
        cancel.cancel();
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::join!(run, cancel_soon)
    })
    .await
    .expect("cancellation must wake the model wait");

    assert_eq!(
        result.expect("cancellation is not a runtime error").outcome,
        RunOutcome::Cancelled
    );
    assert_cancelled_terminal(sink.events());
}

#[tokio::test]
async fn cancellation_drops_a_pending_tool_future() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = runtime(
        vec![tool_turn(
            vec![tool_call("call-1", "pending", json!({}))],
            ModelFinishReason::ToolUse,
        )],
        vec![Arc::new(PendingTool)],
    );
    let cancellation = Cancellation::default();
    let cancel = cancellation.clone();
    let mut sink = MemoryEventSink::default();

    let run = runtime.run(request(&root), cancellation, &mut sink);
    let cancel_soon = async move {
        tokio::task::yield_now().await;
        cancel.cancel();
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(1), async {
        tokio::join!(run, cancel_soon)
    })
    .await
    .expect("cancellation must wake the tool wait");

    assert_eq!(
        result.expect("cancellation is not a runtime error").outcome,
        RunOutcome::Cancelled
    );
    assert_cancelled_terminal(sink.events());
}

fn assert_cancelled_terminal(events: &[forge_runtime_domain::RuntimeEvent]) {
    assert_eq!(count_terminal_events(events), 1);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::RuntimeError { .. }))
    );
    assert!(matches!(
        events.last().map(|event| &event.kind),
        Some(RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Cancelled
        })
    ));
}

fn tool_error_count(messages: &[Message], code: &str) -> usize {
    messages
        .iter()
        .filter(|message| {
            matches!(
                message,
                Message::Tool {
                    output,
                    is_error: true,
                    ..
                } if output.contains(code)
            )
        })
        .count()
}

struct PendingProvider;

impl ModelProvider for PendingProvider {
    fn stream(&self, _request: ModelRequest) -> ModelEventStream {
        Box::pin(stream::pending())
    }
}

struct PendingTool;

impl AgentTool for PendingTool {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "pending".into(),
            description: "Never completes unless the runtime cancels it.".into(),
            input_schema: json!({ "type": "object" }),
            capability: Capability::WorkspaceRead,
        }
    }

    fn execute(&self, _arguments: serde_json::Value, _context: ToolContext) -> ToolFuture<'_> {
        Box::pin(future::pending())
    }
}
