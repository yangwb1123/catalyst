use forge_runtime_domain::{
    Cancellation, LimitKind, ModelEvent, ModelFinishReason, RunOutcome, RuntimeEventKind, Usage,
};
use forge_runtime_infrastructure::MemoryEventSink;
use serde_json::json;
use tempfile::TempDir;

mod support;

use support::{ProbeTool, request, runtime, tool_call, tool_turn};

#[tokio::test]
async fn text_deltas_stop_before_crossing_the_run_byte_budget() {
    let root = TempDir::new().expect("workspace");
    let turns = vec![vec![
        Ok(ModelEvent::TextDelta {
            delta: "abc".into(),
        }),
        Ok(ModelEvent::TextDelta { delta: "d".into() }),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ]];
    let runtime = runtime(turns, vec![]);
    let mut request = request(&root);
    request.limits.max_model_output_bytes = 3;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request, Cancellation::default(), &mut sink)
        .await
        .expect("a local model limit is a normal outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ModelOutput
        }
    );
    let deltas: Vec<_> = sink
        .events()
        .iter()
        .filter_map(|event| match &event.kind {
            RuntimeEventKind::AssistantDelta { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(deltas, ["abc"]);
}

#[tokio::test]
async fn model_event_budget_is_shared_across_turns() {
    let root = TempDir::new().expect("workspace");
    let call = tool_call("call-1", "probe", json!({}));
    let turns = vec![
        tool_turn(vec![call], ModelFinishReason::ToolUse),
        vec![
            Ok(ModelEvent::TextDelta {
                delta: "answer".into(),
            }),
            Ok(ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            }),
        ],
    ];
    let tool = ProbeTool::succeeds("probe", "observation");
    let runtime = runtime(turns, vec![tool]);
    let mut request = request(&root);
    request.limits.max_model_events = 1;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request, Cancellation::default(), &mut sink)
        .await
        .expect("a local model limit is a normal outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ModelOutput
        }
    );
    assert!(!sink.events().iter().any(|event| {
        matches!(
            &event.kind,
            RuntimeEventKind::AssistantDelta { delta } if delta == "answer"
        )
    }));
}

#[tokio::test]
async fn usage_event_floods_consume_the_model_event_budget() {
    let root = TempDir::new().expect("workspace");
    let usage = ModelEvent::Usage {
        usage: Usage {
            input_tokens: 1,
            output_tokens: 0,
        },
    };
    let turns = vec![vec![
        Ok(usage.clone()),
        Ok(usage.clone()),
        Ok(usage),
        Ok(ModelEvent::Finished {
            reason: ModelFinishReason::Completed,
        }),
    ]];
    let runtime = runtime(turns, vec![]);
    let mut request = request(&root);
    request.limits.max_model_events = 2;
    let mut sink = MemoryEventSink::default();

    let result = runtime
        .run(request, Cancellation::default(), &mut sink)
        .await
        .expect("a local model limit is a normal outcome");

    assert_eq!(
        result.outcome,
        RunOutcome::LimitExceeded {
            kind: LimitKind::ModelOutput
        }
    );
}
