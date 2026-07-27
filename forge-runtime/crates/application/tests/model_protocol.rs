use std::{sync::Arc, time::Duration};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_domain::{
    Cancellation, LimitKind, Message, ModelEvent, ModelEventStream, ModelFinishReason,
    ModelProvider, ModelRequest, RunOutcome, RuntimeEventKind,
};
use forge_runtime_infrastructure::{CapStdWorkspaceFactory, MemoryEventSink};
use futures_util::{StreamExt, stream};
use serde_json::json;
use tempfile::TempDir;

mod support;

use support::{ScriptedTurn, count_terminal_events, request, runtime, tool_call, tool_turn};

#[tokio::test]
async fn rejects_finish_reasons_that_disagree_with_tool_calls() {
    let completed_with_call = tool_turn(
        vec![tool_call("call-1", "missing", json!({}))],
        ModelFinishReason::Completed,
    );
    assert_protocol_failure(completed_with_call).await;

    let tool_use_without_call = vec![Ok(ModelEvent::Finished {
        reason: ModelFinishReason::ToolUse,
    })];
    assert_protocol_failure(tool_use_without_call).await;
}

#[tokio::test]
async fn rejects_every_provider_event_after_finished() {
    let events = vec![
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
        Ok(ModelEvent::TextDelta {
            delta: "late".into(),
        }),
    ];
    assert_protocol_failure(events).await;
}

#[tokio::test]
async fn length_without_tool_calls_is_a_limit_not_a_completed_answer() {
    let root = TempDir::new().expect("temporary workspace");
    let events = vec![
        Ok(ModelEvent::TextDelta {
            delta: "partial".into(),
        }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Length,
        }),
    ];
    let runtime = runtime(vec![events], vec![]);
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect("model length is a normal limit outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ModelOutput
        }
    );
    assert!(
        !result
            .messages
            .iter()
            .any(|message| matches!(message, Message::Assistant { .. }))
    );
    assert_eq!(count_terminal_events(sink.events()), 1);
}

#[tokio::test]
async fn rejects_duplicate_tool_call_ids() {
    let calls = vec![
        tool_call("duplicate", "first", json!({})),
        tool_call("duplicate", "second", json!({})),
    ];
    assert_protocol_failure(tool_turn(calls, ModelFinishReason::ToolUse)).await;
}

#[tokio::test]
async fn finished_event_does_not_wait_for_provider_eof() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = AgentRuntime::new(
        Arc::new(FinishedThenPending),
        ToolCatalog::default(),
        Arc::new(CapStdWorkspaceFactory),
    );
    let mut sink = MemoryEventSink::default();

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        runtime.run(request(&root), Cancellation::default(), &mut sink),
    )
    .await
    .expect("finished is terminal even if the provider stays pending")
    .expect("valid finished turn succeeds");

    assert_eq!(
        result.outcome,
        RunOutcome::Completed {
            answer: "done".into()
        }
    );
}

async fn assert_protocol_failure(turn: ScriptedTurn) {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = runtime(vec![turn], vec![]);
    let mut sink = MemoryEventSink::default();

    let error = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect_err("invalid provider protocol fails the run");

    assert_eq!(error.code(), "model_protocol_error");
    assert_eq!(count_terminal_events(sink.events()), 1);
    assert!(matches!(
        sink.events().last().map(|event| &event.kind),
        Some(RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Failed { code, .. }
        }) if code == "model_protocol_error"
    ));
}

struct FinishedThenPending;

impl ModelProvider for FinishedThenPending {
    fn stream(&self, _request: ModelRequest) -> ModelEventStream {
        let events = vec![
            Ok(ModelEvent::TextDelta {
                delta: "done".into(),
            }),
            Ok(ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            }),
        ];
        Box::pin(stream::iter(events).chain(stream::pending()))
    }
}
