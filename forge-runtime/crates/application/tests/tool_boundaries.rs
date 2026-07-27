use forge_runtime_domain::{
    Cancellation, Message, ModelFinishReason, RunOutcome, RuntimeEvent, RuntimeEventKind,
};
use forge_runtime_infrastructure::MemoryEventSink;
use serde_json::json;
use tempfile::TempDir;

mod support;

use support::{ProbeTool, completed, request, runtime, tool_call, tool_turn};

#[tokio::test]
async fn oversized_tool_output_is_utf8_safely_truncated_before_model_commit() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = ProbeTool::succeeds("verbose", "ééé");
    let runtime = runtime(
        vec![
            tool_turn(
                vec![tool_call("call-1", "verbose", json!({}))],
                ModelFinishReason::ToolUse,
            ),
            completed("handled"),
        ],
        vec![probe.clone()],
    );
    let mut bounded_request = request(&root);
    bounded_request.limits.max_tool_output_bytes = 3;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(bounded_request, Cancellation::default(), &mut sink)
        .await
        .expect("output truncation preserves a valid run");

    let expected = RunOutcome::Completed {
        answer: "handled".into(),
    };
    assert_eq!(result.outcome, expected);
    assert_eq!(probe.invocation_count(), 1);
    let output = successful_message_output(&result.messages);
    assert_eq!(output, Some(("é", true)));
    assert!(output.expect("tool output").0.len() <= 3);
    let event_output = successful_event_output(sink.events());
    assert_eq!(event_output, Some(("é", true)));
}

#[tokio::test]
async fn tool_errors_obey_the_same_output_limit_and_expose_truncation() {
    let root = TempDir::new().expect("temporary workspace");
    let probe = ProbeTool::fails(
        "fallible",
        "probe_failed",
        "an intentionally long deterministic error",
    );
    let runtime = runtime(
        vec![
            tool_turn(
                vec![tool_call("call-1", "fallible", json!({}))],
                ModelFinishReason::ToolUse,
            ),
            completed("handled"),
        ],
        vec![probe],
    );
    let mut bounded_request = request(&root);
    bounded_request.limits.max_tool_output_bytes = 8;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(bounded_request, Cancellation::default(), &mut sink)
        .await
        .expect("bounded tool errors remain model-visible");

    let message = result.messages.iter().find_map(|message| match message {
        Message::Tool {
            output,
            is_error: true,
            truncated,
            ..
        } => Some((output, *truncated)),
        _ => None,
    });
    let (output, truncated) = message.expect("error tool message");
    assert!(output.len() <= 8);
    assert!(truncated);
    assert!(sink.events().iter().any(|event| matches!(
        event.kind,
        RuntimeEventKind::ToolFinished {
            is_error: true,
            truncated: true,
            ..
        }
    )));
}

fn successful_message_output(messages: &[Message]) -> Option<(&str, bool)> {
    messages.iter().find_map(|message| match message {
        Message::Tool {
            output,
            is_error: false,
            truncated,
            ..
        } => Some((output.as_str(), *truncated)),
        _ => None,
    })
}

fn successful_event_output(events: &[RuntimeEvent]) -> Option<(&str, bool)> {
    events.iter().find_map(|event| match &event.kind {
        RuntimeEventKind::ToolFinished {
            output,
            is_error: false,
            truncated,
            ..
        } => Some((output.as_str(), *truncated)),
        _ => None,
    })
}
