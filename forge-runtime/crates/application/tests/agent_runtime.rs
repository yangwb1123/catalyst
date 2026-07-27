use std::{fs, sync::Arc};

#[cfg(unix)]
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use forge_runtime_application::{AgentRuntime, ToolCatalog};
use forge_runtime_domain::{
    Cancellation, Capability, LimitKind, Message, ModelEvent, ModelEventStream, ModelFinishReason,
    ModelProvider, ModelRequest, RunLimits, RunOutcome, RunRequest, RuntimeEventKind, ToolCall,
};
use forge_runtime_infrastructure::{
    CapStdWorkspaceFactory, MemoryEventSink, ReadFileTool, ScriptedProvider,
};
use serde_json::json;
use tempfile::TempDir;

mod support;

fn completed(text: &str) -> Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>> {
    vec![
        Ok(ModelEvent::TextDelta { delta: text.into() }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ]
}

fn read_turn(
    finish_reason: ModelFinishReason,
) -> Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>> {
    vec![
        Ok(ModelEvent::ToolCall {
            call: ToolCall {
                id: "call-1".into(),
                name: "read_file".into(),
                arguments: json!({ "path": "note.txt" }),
            },
        }),
        Ok(ModelEvent::Finished {
            reason: finish_reason,
        }),
    ]
}

fn request(root: &TempDir) -> RunRequest {
    RunRequest {
        session_id: "session-1".into(),
        run_id: "run-1".into(),
        prompt: "Inspect note.txt".into(),
        system_prompt: "Use tools.".into(),
        workspace: root.path().to_path_buf(),
        allowed_capabilities: vec![Capability::WorkspaceRead],
        limits: RunLimits {
            max_turns: 4,
            max_tool_calls: 4,
            max_tool_output_bytes: 1024,
        },
    }
}

fn runtime(
    turns: Vec<Vec<Result<ModelEvent, forge_runtime_domain::ProviderError>>>,
) -> AgentRuntime {
    let mut catalog = ToolCatalog::default();
    catalog
        .register(Arc::new(ReadFileTool))
        .expect("tool registration succeeds");
    AgentRuntime::new(
        Arc::new(ScriptedProvider::new(turns)),
        catalog,
        Arc::new(CapStdWorkspaceFactory),
    )
}

#[tokio::test]
async fn executes_a_tool_and_finishes_with_strictly_ordered_events() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("note.txt"), "hello").expect("fixture file");
    let runtime = runtime(vec![
        read_turn(ModelFinishReason::ToolUse),
        completed("done"),
    ]);
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect("run succeeds");

    assert_eq!(
        result.outcome,
        RunOutcome::Completed {
            answer: "done".into()
        }
    );
    let events = sink.events();
    let event_kinds: Vec<_> = events.iter().map(|event| kind_name(&event.kind)).collect();
    assert_eq!(
        event_kinds,
        [
            "run_started",
            "message_committed",
            "turn_started",
            "message_committed",
            "tool_started",
            "tool_finished",
            "message_committed",
            "turn_started",
            "assistant_delta",
            "message_committed",
            "run_finished",
        ]
    );
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event.seq, u64::try_from(index + 1).expect("small fixture"));
        assert_eq!(event.session_id, "session-1");
        assert_eq!(event.run_id, "run-1");
    }
    let tool_results = result
        .messages
        .iter()
        .filter(|message| matches!(message, Message::Tool { .. }))
        .count();
    assert_eq!(tool_results, 1);
}

#[cfg(unix)]
#[tokio::test]
async fn runtime_workspace_capability_survives_path_swap_attack() {
    let base = TempDir::new().expect("temporary base");
    let workspace = base.path().join("workspace");
    let moved = base.path().join("workspace-moved");
    let outside = base.path().join("outside");
    fs::create_dir(&workspace).expect("workspace directory");
    fs::create_dir(&outside).expect("outside directory");
    fs::write(workspace.join("note.txt"), "inside").expect("inside fixture");
    fs::write(outside.join("note.txt"), "outside").expect("outside fixture");

    let provider = Arc::new(SwapWorkspaceProvider {
        workspace: workspace.clone(),
        moved,
        outside,
        turns: AtomicUsize::new(0),
    });
    let mut tools = ToolCatalog::default();
    tools
        .register(Arc::new(ReadFileTool))
        .expect("tool registration succeeds");
    let runtime = AgentRuntime::new(provider, tools, Arc::new(CapStdWorkspaceFactory));
    let mut run_request = request(&base);
    run_request.workspace = workspace;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(run_request, Cancellation::default(), &mut sink)
        .await
        .expect("anchored workspace run succeeds");

    assert_eq!(
        result.outcome,
        RunOutcome::Completed {
            answer: "done".into()
        }
    );
    let output = result.messages.iter().find_map(|message| match message {
        Message::Tool { output, .. } => Some(output.as_str()),
        _ => None,
    });
    assert_eq!(output, Some("inside"));
    assert_eq!(support::count_terminal_events(sink.events()), 1);
}

#[tokio::test]
async fn denies_an_ungranted_capability_and_returns_the_error_to_the_model() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = support::ProbeTool::succeeds("read_file", "must not execute");
    let runtime = support::runtime(
        vec![read_turn(ModelFinishReason::ToolUse), completed("handled")],
        vec![probe.clone()],
    );
    let mut denied_request = request(&root);
    denied_request.allowed_capabilities.clear();
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(denied_request, Cancellation::default(), &mut sink)
        .await
        .expect("denial remains a model-visible tool result");

    let denied = result.messages.iter().find_map(|message| match message {
        Message::Tool {
            output, is_error, ..
        } => Some((output, is_error)),
        _ => None,
    });
    let (output, is_error) = denied.expect("tool result is present");
    assert!(*is_error);
    assert!(output.contains("capability_denied"));
    assert_eq!(probe.invocation_count(), 0);
}

#[tokio::test]
async fn enforces_the_turn_limit_after_a_tool_round_trip() {
    let root = TempDir::new().expect("temporary workspace");
    fs::write(root.path().join("note.txt"), "hello").expect("fixture file");
    let runtime = runtime(vec![read_turn(ModelFinishReason::ToolUse)]);
    let mut limited_request = request(&root);
    limited_request.limits.max_turns = 1;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(limited_request, Cancellation::default(), &mut sink)
        .await
        .expect("limit is a normal terminal outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::Turns
        }
    );
}

#[tokio::test]
async fn never_executes_tool_arguments_from_a_truncated_model_turn() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = support::ProbeTool::succeeds("read_file", "must not execute");
    let runtime = support::runtime(
        vec![read_turn(ModelFinishReason::Length), completed("recovered")],
        vec![probe.clone()],
    );
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect("model can recover after a rejected truncated call");

    let rejected = result.messages.iter().any(|message| {
        matches!(
            message,
            Message::Tool {
                output,
                is_error: true,
                ..
            } if output.contains("truncated_tool_call")
        )
    });
    assert!(rejected);
    assert_eq!(probe.invocation_count(), 0);
    assert!(sink.events().iter().any(|event| matches!(
        &event.kind,
        RuntimeEventKind::ToolRejected { code, .. } if code == "truncated_tool_call"
    )));
    assert!(
        !sink
            .events()
            .iter()
            .any(|event| matches!(event.kind, RuntimeEventKind::ToolStarted { .. }))
    );
}

#[tokio::test]
async fn emits_one_failed_terminal_event_when_provider_protocol_is_invalid() {
    let root = TempDir::new().expect("temporary workspace");
    let invalid_turn = vec![Ok(ModelEvent::TextDelta {
        delta: "partial".into(),
    })];
    let runtime = runtime(vec![invalid_turn]);
    let mut sink = MemoryEventSink::default();

    let error = runtime
        .run(request(&root), Cancellation::default(), &mut sink)
        .await
        .expect_err("missing finished event fails the run");

    assert_eq!(error.code(), "model_protocol_error");
    let terminal_count = sink
        .events()
        .iter()
        .filter(|event| matches!(event.kind, RuntimeEventKind::RunFinished { .. }))
        .count();
    assert_eq!(terminal_count, 1);
}

fn kind_name(kind: &RuntimeEventKind) -> &'static str {
    match kind {
        RuntimeEventKind::RunStarted { .. } => "run_started",
        RuntimeEventKind::TurnStarted { .. } => "turn_started",
        RuntimeEventKind::AssistantDelta { .. } => "assistant_delta",
        RuntimeEventKind::MessageCommitted { .. } => "message_committed",
        RuntimeEventKind::ToolStarted { .. } => "tool_started",
        RuntimeEventKind::ToolFinished { .. } => "tool_finished",
        RuntimeEventKind::ToolRejected { .. } => "tool_rejected",
        RuntimeEventKind::RuntimeError { .. } => "runtime_error",
        RuntimeEventKind::RunFinished { .. } => "run_finished",
    }
}

#[cfg(unix)]
struct SwapWorkspaceProvider {
    workspace: PathBuf,
    moved: PathBuf,
    outside: PathBuf,
    turns: AtomicUsize,
}

#[cfg(unix)]
impl ModelProvider for SwapWorkspaceProvider {
    fn stream(&self, _request: ModelRequest) -> ModelEventStream {
        use std::os::unix::fs::symlink;

        let turn = self.turns.fetch_add(1, Ordering::SeqCst);
        let events = if turn == 0 {
            fs::rename(&self.workspace, &self.moved).expect("rename workspace");
            symlink(&self.outside, &self.workspace).expect("replace path with outside symlink");
            read_turn(ModelFinishReason::ToolUse)
        } else {
            completed("done")
        };
        Box::pin(futures_util::stream::iter(events))
    }
}
