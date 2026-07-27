use std::{path::Path, sync::Arc};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_domain::{
    Cancellation, EventSink, EventSinkError, Message, ModelFinishReason, ProviderError, RunOutcome,
    RuntimeEvent, RuntimeEventKind, WorkspaceOpenError, WorkspaceReadCapability,
    WorkspaceReadFactory,
};
use forge_runtime_infrastructure::{MemoryEventSink, ScriptedProvider};
use serde_json::json;
use tempfile::TempDir;

mod support;

use support::{
    ProbeTool, completed, count_terminal_events, request, runtime, tool_call, tool_turn,
};

#[tokio::test]
async fn unknown_tool_is_model_visible_and_does_not_crash_the_runtime() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = runtime(
        vec![
            tool_turn(
                vec![tool_call("call-1", "missing", json!({}))],
                ModelFinishReason::ToolUse,
            ),
            completed("handled"),
        ],
        vec![],
    );
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect("the model can recover from an unknown tool");

    assert_eq!(
        result.outcome,
        RunOutcome::Completed {
            answer: "handled".into()
        }
    );
    assert!(has_tool_error(&result.messages, "unknown_tool"));
    assert_unique_success_terminal(sink.events());
}

#[tokio::test]
async fn tool_error_is_model_visible_and_has_one_successful_terminal_event() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = ProbeTool::fails("fallible", "probe_failed", "expected failure");
    let runtime = runtime(
        vec![
            tool_turn(
                vec![tool_call("call-1", "fallible", json!({}))],
                ModelFinishReason::ToolUse,
            ),
            completed("recovered"),
        ],
        vec![probe.clone()],
    );
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect("tool failure remains recoverable by the model");

    assert_eq!(probe.invocation_count(), 1);
    assert!(has_tool_error(&result.messages, "probe_failed"));
    assert_unique_success_terminal(sink.events());
}

#[tokio::test]
async fn provider_error_emits_exactly_one_error_and_one_failed_terminal() {
    let root = TempDir::new().expect("temporary workspace");
    let provider_error = ProviderError::new("provider_down", "offline fixture", true);
    let runtime = runtime(vec![vec![Err(provider_error)]], vec![]);
    let mut sink = MemoryEventSink::default();

    let error = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect_err("provider failure must fail the run");

    assert_eq!(error.code(), "provider_error");
    assert_eq!(runtime_error_count(sink.events()), 1);
    assert_eq!(count_terminal_events(sink.events()), 1);
    assert!(matches!(
        sink.events().last().map(|event| &event.kind),
        Some(RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Failed { code, .. }
        }) if code == "provider_error"
    ));
}

#[tokio::test]
async fn workspace_open_failure_emits_one_workspace_unavailable_terminal() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = AgentRuntime::new(
        Arc::new(ScriptedProvider::new(vec![completed("unreachable")])),
        ToolCatalog::default(),
        Arc::new(UnavailableWorkspaceFactory),
    );
    let mut sink = MemoryEventSink::default();

    let error = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect_err("workspace factory failure must fail the run");

    assert_eq!(error.code(), "workspace_unavailable");
    assert_eq!(runtime_error_count(sink.events()), 1);
    assert_eq!(count_terminal_events(sink.events()), 1);
    assert!(sink.events().iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::RuntimeError { code, .. } if code == "workspace_unavailable"
    )));
    assert!(matches!(
        sink.events().last().map(|event| &event.kind),
        Some(RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Failed { code, .. }
        }) if code == "workspace_unavailable"
    ));
}

#[tokio::test]
async fn terminal_sink_failure_does_not_attempt_a_second_terminal_event() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = runtime(vec![completed("done")], vec![]);
    let mut sink = FailOnRecordedTerminalSink::default();

    let error = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect_err("a sink failure must be returned");

    assert_eq!(error.code(), "event_sink_error");
    assert_eq!(count_terminal_events(&sink.events), 1);
    assert_eq!(runtime_error_count(&sink.events), 0);
}

#[tokio::test]
async fn midstream_sink_failure_aborts_without_writing_more_events() {
    let root = TempDir::new().expect("temporary workspace");
    let runtime = runtime(vec![completed("done")], vec![]);
    let mut sink = FailAtSink::new(2);

    let error = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect_err("sink failure must abort the event stream");

    assert_eq!(error.code(), "event_sink_error");
    assert_eq!(sink.attempts, 2);
    assert_eq!(sink.events.len(), 1);
    assert!(matches!(
        sink.events.first().map(|event| &event.kind),
        Some(RuntimeEventKind::RunStarted { .. })
    ));
}

fn has_tool_error(messages: &[Message], code: &str) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            Message::Tool {
                output,
                is_error: true,
                ..
            } if output.contains(code)
        )
    })
}

fn assert_unique_success_terminal(events: &[RuntimeEvent]) {
    assert_eq!(runtime_error_count(events), 0);
    assert_eq!(count_terminal_events(events), 1);
    assert!(matches!(
        events.last().map(|event| &event.kind),
        Some(RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed { .. }
        })
    ));
}

fn runtime_error_count(events: &[RuntimeEvent]) -> usize {
    events
        .iter()
        .filter(|event| matches!(event.kind, RuntimeEventKind::RuntimeError { .. }))
        .count()
}

struct UnavailableWorkspaceFactory;

impl WorkspaceReadFactory for UnavailableWorkspaceFactory {
    fn open(&self, _workspace: &Path) -> Result<WorkspaceReadCapability, WorkspaceOpenError> {
        Err(WorkspaceOpenError::new(
            "deterministic workspace open failure",
        ))
    }
}

#[derive(Default)]
struct FailOnRecordedTerminalSink {
    events: Vec<RuntimeEvent>,
}

struct FailAtSink {
    fail_at: usize,
    attempts: usize,
    events: Vec<RuntimeEvent>,
}

impl FailAtSink {
    fn new(fail_at: usize) -> Self {
        Self {
            fail_at,
            attempts: 0,
            events: Vec::new(),
        }
    }
}

impl EventSink for FailAtSink {
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError> {
        self.attempts += 1;
        if self.attempts == self.fail_at {
            return Err(EventSinkError::new("deterministic sink failure"));
        }
        self.events.push(event.clone());
        Ok(())
    }
}

impl EventSink for FailOnRecordedTerminalSink {
    fn emit(&mut self, event: &RuntimeEvent) -> Result<(), EventSinkError> {
        self.events.push(event.clone());
        if matches!(event.kind, RuntimeEventKind::RunFinished { .. }) {
            return Err(EventSinkError::new("terminal write failed after append"));
        }
        Ok(())
    }
}
