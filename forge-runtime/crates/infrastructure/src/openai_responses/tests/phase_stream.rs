use forge_runtime_domain::{
    Cancellation, Message, ModelEvent, ModelFinishReason, ModelRequest, ProviderError,
};
use serde_json::{Value, json};

use super::{request::encode_request, sse::SseDecoder};

const SECRET: &str = "sk-phase-test";

#[test]
fn interleaved_commentary_and_multiple_final_messages_stream_only_final_text() {
    let terminal_items = vec![
        message_item("comment-1", "hidden one", Some(json!("commentary"))),
        message_item("final-1", "answer ", Some(json!("final_answer"))),
        message_item("comment-2", "hidden two", Some(json!("commentary"))),
        message_item("final-2", "done", Some(json!("final_answer"))),
    ];
    let stream = [
        message_added("comment-1", Some(json!("commentary"))),
        text_delta("comment-1", "hidden one"),
        message_added("final-1", Some(json!("final_answer"))),
        text_delta("final-1", "answer "),
        message_added("comment-2", Some(json!("commentary"))),
        text_delta("comment-2", "hidden two"),
        message_added("final-2", Some(json!("final_answer"))),
        text_delta("final-2", "done"),
        completed_event(terminal_items.clone()),
    ];

    let events = decode(stream).expect("phase-aware response");

    assert_eq!(
        events,
        vec![
            ModelEvent::TextDelta {
                delta: "answer ".into(),
            },
            ModelEvent::TextDelta {
                delta: "done".into(),
            },
            ModelEvent::ProviderContext {
                provider: "openai.responses".into(),
                items: terminal_items.clone(),
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::Completed,
            },
        ]
    );
    assert_context_replays_with_final_projection(&terminal_items);
}

#[test]
fn omitted_and_null_phases_remain_streaming_legacy_final_text() {
    let items = vec![
        message_item("legacy-1", "old ", None),
        message_item("legacy-2", "model", Some(Value::Null)),
    ];
    let stream = [
        message_added("legacy-1", None),
        text_delta("legacy-1", "old "),
        message_added("legacy-2", Some(Value::Null)),
        text_delta("legacy-2", "model"),
        completed_event(items),
    ];

    let events = decode(stream).expect("legacy phase response");

    assert!(matches!(
        &events[0],
        ModelEvent::TextDelta { delta } if delta == "old "
    ));
    assert!(matches!(
        &events[1],
        ModelEvent::TextDelta { delta } if delta == "model"
    ));
    assert!(matches!(
        events.last(),
        Some(ModelEvent::Finished {
            reason: ModelFinishReason::Completed
        })
    ));
}

#[test]
fn commentary_only_cannot_complete_a_no_tool_response() {
    let stream = [
        message_added("comment", Some(json!("commentary"))),
        text_delta("comment", "still working"),
        completed_event(vec![message_item(
            "comment",
            "still working",
            Some(json!("commentary")),
        )]),
    ];

    let error = decode(stream).expect_err("commentary is not a final answer");

    assert_eq!(error.code, "provider_protocol");
}

#[test]
fn final_answer_cannot_be_paired_with_tool_use() {
    let stream = [
        message_added("final", Some(json!("final_answer"))),
        text_delta("final", "premature"),
        json!({
            "type": "response.output_item.added",
            "item": {
                "type": "function_call",
                "id": "function",
                "call_id": "call",
                "name": "read_file"
            }
        }),
        json!({
            "type": "response.function_call_arguments.done",
            "item_id": "function",
            "name": "read_file",
            "arguments": "{}"
        }),
        completed_event(vec![
            message_item("final", "premature", Some(json!("final_answer"))),
            json!({
                "type": "function_call",
                "id": "function",
                "call_id": "call",
                "name": "read_file",
                "arguments": "{}",
                "status": "completed"
            }),
        ]),
    ];

    let error = decode(stream).expect_err("final answer with tool");

    assert_eq!(error.code, "provider_protocol");
}

#[test]
fn unknown_delta_item_and_unknown_explicit_phase_fail_closed() {
    let unknown_delta =
        decode([text_delta("missing", "unsafe")]).expect_err("delta without output item");
    assert_eq!(unknown_delta.code, "provider_protocol");

    let unknown_phase =
        decode([message_added("message", Some(json!("analysis")))]).expect_err("unknown phase");
    assert_eq!(unknown_phase.code, "provider_protocol");
}

#[test]
fn terminal_phase_must_match_the_streamed_message_phase() {
    let stream = [
        message_added("message", Some(json!("final_answer"))),
        text_delta("message", "answer"),
        completed_event(vec![message_item(
            "message",
            "answer",
            Some(json!("commentary")),
        )]),
    ];

    let error = decode(stream).expect_err("terminal phase mismatch");

    assert_eq!(error.code, "provider_protocol");
}

fn decode<const N: usize>(events: [Value; N]) -> Result<Vec<ModelEvent>, ProviderError> {
    let mut decoder = SseDecoder::new(SECRET);
    let mut output = Vec::new();
    for event in events {
        output.extend(push_event(&mut decoder, &event)?);
    }
    Ok(output)
}

fn push_event(decoder: &mut SseDecoder, event: &Value) -> Result<Vec<ModelEvent>, ProviderError> {
    let kind = event["type"].as_str().expect("event type");
    let frame = format!("event: {kind}\ndata: {event}\n\n");
    decoder.push(frame.as_bytes())
}

fn message_added(id: &str, phase: Option<Value>) -> Value {
    let mut item = json!({"type": "message", "id": id, "role": "assistant"});
    if let Some(phase) = phase {
        item["phase"] = phase;
    }
    json!({"type": "response.output_item.added", "item": item})
}

fn text_delta(id: &str, delta: &str) -> Value {
    json!({
        "type": "response.output_text.delta",
        "item_id": id,
        "delta": delta
    })
}

fn completed_event(output: Vec<Value>) -> Value {
    json!({
        "type": "response.completed",
        "response": {
            "status": "completed",
            "output": Value::Array(output),
            "usage": null
        }
    })
}

fn message_item(id: &str, text: &str, phase: Option<Value>) -> Value {
    let mut item = json!({
        "type": "message",
        "id": id,
        "status": "completed",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": text,
            "annotations": []
        }]
    });
    if let Some(phase) = phase {
        item["phase"] = phase;
    }
    item
}

fn assert_context_replays_with_final_projection(items: &[Value]) {
    let request = ModelRequest {
        system_prompt: "system".into(),
        messages: vec![
            Message::User { text: "go".into() },
            Message::ProviderContext {
                provider: "openai.responses".into(),
                items: items.to_vec(),
            },
            Message::Assistant {
                text: "answer done".into(),
                tool_calls: Vec::new(),
            },
            Message::User {
                text: "continue".into(),
            },
        ],
        tools: Vec::new(),
        max_output_tokens: 128,
        cancellation: Cancellation::default(),
    };

    let encoded = encode_request("test-model", &request).expect("replayed context");
    let input = encoded["input"].as_array().expect("input array");

    assert_eq!(&input[1..=4], items);
    assert_eq!(input.len(), 6);
}
