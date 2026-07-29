use forge_runtime_domain::{ModelEvent, ModelFinishReason, Usage};
use serde_json::{Value, json};

use super::sse::SseDecoder;

const SECRET: &str = "sk-terminal-test";

#[test]
fn max_output_tokens_is_the_only_incomplete_length_reason() {
    let event = incomplete_event(
        Vec::new(),
        Some(json!({"reason": "max_output_tokens"})),
        Some(json!({"input_tokens": 7, "output_tokens": 11})),
    );
    let events = push_event(&mut SseDecoder::new(SECRET), &event).expect("length response");

    assert_eq!(
        events,
        vec![
            ModelEvent::Usage {
                usage: Usage {
                    input_tokens: 7,
                    output_tokens: 11,
                },
            },
            ModelEvent::Finished {
                reason: ModelFinishReason::Length,
            },
        ]
    );
}

#[test]
fn content_filter_is_a_definitive_provider_failure() {
    let event = incomplete_event(Vec::new(), Some(json!({"reason": "content_filter"})), None);
    let mut decoder = SseDecoder::new(SECRET);

    let error = push_event(&mut decoder, &event).expect_err("filtered response");

    assert_eq!(error.code, "content_filter");
    assert!(!error.retryable);
    assert!(decoder.is_terminal());
}

#[test]
fn missing_and_unknown_incomplete_reasons_fail_closed() {
    for details in [None, Some(json!({"reason": "future_reason"}))] {
        let event = incomplete_event(Vec::new(), details, None);
        let error = push_event(&mut SseDecoder::new(SECRET), &event)
            .expect_err("unsupported incomplete reason");

        assert_eq!(error.code, "provider_protocol");
        assert!(!error.retryable);
    }
}

#[test]
fn terminal_event_and_response_envelope_must_agree() {
    let mut wrong_completed_status = completed_event(Vec::new());
    wrong_completed_status["response"]["status"] = json!("incomplete");
    let mut completed_details = completed_event(Vec::new());
    completed_details["response"]["incomplete_details"] = json!({"reason": "content_filter"});
    let mut wrong_incomplete_status = incomplete_event(
        Vec::new(),
        Some(json!({"reason": "max_output_tokens"})),
        None,
    );
    wrong_incomplete_status["response"]["status"] = json!("completed");
    let missing_status = json!({
        "type": "response.completed",
        "response": {"output": [], "usage": null}
    });

    for event in [
        wrong_completed_status,
        completed_details,
        wrong_incomplete_status,
        missing_status,
    ] {
        let error = push_event(&mut SseDecoder::new(SECRET), &event)
            .expect_err("contradictory response envelope");
        assert_eq!(error.code, "provider_protocol");
    }
}

#[test]
fn completed_responses_reject_noncompleted_item_statuses() {
    let invalid_items = [
        message_item("message-incomplete", "", "incomplete"),
        message_item("message-progress", "", "in_progress"),
        function_item("function-incomplete", "call-a", "incomplete", "{}"),
        function_item("function-progress", "call-b", "in_progress", "{}"),
        reasoning_item("reasoning-incomplete", "incomplete"),
        reasoning_item("reasoning-progress", "in_progress"),
    ];
    for item in invalid_items {
        let event = completed_event(vec![item]);
        let error = push_event(&mut SseDecoder::new(SECRET), &event)
            .expect_err("invalid completed item status");

        assert_eq!(error.code, "provider_protocol");
    }
}

#[test]
fn incomplete_responses_reject_in_progress_item_status() {
    let event = incomplete_event(
        vec![reasoning_item("reasoning-progress", "in_progress")],
        Some(json!({"reason": "max_output_tokens"})),
        None,
    );

    let error =
        push_event(&mut SseDecoder::new(SECRET), &event).expect_err("in-progress terminal item");

    assert_eq!(error.code, "provider_protocol");
}

#[test]
fn incomplete_responses_accept_completed_or_incomplete_message_status() {
    for status in ["completed", "incomplete"] {
        let mut decoder = SseDecoder::new(SECRET);
        let added = json!({
            "type": "response.output_item.added",
            "item": {
                "type": "message",
                "id": "partial-message",
                "role": "assistant",
                "phase": "final_answer"
            }
        });
        push_event(&mut decoder, &added).expect("message item");
        let event = incomplete_event(
            vec![message_item("partial-message", "", status)],
            Some(json!({"reason": "max_output_tokens"})),
            None,
        );

        let events = push_event(&mut decoder, &event).expect("accepted terminal status");

        assert_eq!(
            events,
            vec![ModelEvent::Finished {
                reason: ModelFinishReason::Length,
            }]
        );
    }
}

#[test]
fn incomplete_function_calls_are_protocol_failures() {
    let mut decoder = SseDecoder::new(SECRET);
    push_event(
        &mut decoder,
        &function_added("item-partial", "call-partial"),
    )
    .expect("pending function item");
    let event = incomplete_event(
        vec![function_item(
            "item-partial",
            "call-partial",
            "incomplete",
            "{",
        )],
        Some(json!({"reason": "max_output_tokens"})),
        None,
    );

    let error = push_event(&mut decoder, &event).expect_err("incomplete function response");

    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("function call"));
}

#[test]
fn completed_calls_in_an_incomplete_response_are_protocol_failures() {
    let mut decoder = SseDecoder::new(SECRET);
    push_event(&mut decoder, &function_added("item-done", "call-done")).expect("function item");
    push_event(
        &mut decoder,
        &json!({
            "type": "response.function_call_arguments.done",
            "item_id": "item-done",
            "name": "read_file",
            "arguments": "{}"
        }),
    )
    .expect("completed arguments");
    let event = incomplete_event(
        vec![function_item("item-done", "call-done", "completed", "{}")],
        Some(json!({"reason": "max_output_tokens"})),
        None,
    );

    let error = push_event(&mut decoder, &event).expect_err("incomplete response");

    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("function call"));
}

#[test]
fn terminal_function_identity_must_match_the_streamed_item() {
    let mut incomplete = SseDecoder::new(SECRET);
    let added = function_added("streamed-item", "streamed-call");
    push_event(&mut incomplete, &added).expect("pending function item");
    let event = incomplete_event(
        vec![function_item(
            "different-item",
            "different-call",
            "incomplete",
            "{",
        )],
        Some(json!({"reason": "max_output_tokens"})),
        None,
    );
    let error = push_event(&mut incomplete, &event).expect_err("mismatched pending call");
    assert_eq!(error.code, "provider_protocol");

    let mut completed = SseDecoder::new(SECRET);
    let added = function_added("streamed-item", "streamed-call");
    push_event(&mut completed, &added).expect("function item");
    let arguments_done = json!({
        "type": "response.function_call_arguments.done",
        "item_id": "streamed-item",
        "name": "read_file",
        "arguments": "{}"
    });
    push_event(&mut completed, &arguments_done).expect("completed arguments");
    let event = completed_event(vec![function_item(
        "different-item",
        "streamed-call",
        "completed",
        "{}",
    )]);
    let error = push_event(&mut completed, &event).expect_err("mismatched completed item");
    assert_eq!(error.code, "provider_protocol");
}

#[test]
fn streamed_function_terminal_items_cannot_omit_their_identity() {
    let mut incomplete = SseDecoder::new(SECRET);
    let added = function_added("streamed-item", "streamed-call");
    push_event(&mut incomplete, &added).expect("pending function item");
    let mut incomplete_item = function_item("streamed-item", "streamed-call", "incomplete", "{");
    incomplete_item
        .as_object_mut()
        .expect("function object")
        .remove("id");
    let event = incomplete_event(
        vec![incomplete_item],
        Some(json!({"reason": "max_output_tokens"})),
        None,
    );
    let error = push_event(&mut incomplete, &event).expect_err("missing incomplete item id");
    assert_eq!(error.code, "provider_protocol");

    let mut completed = SseDecoder::new(SECRET);
    push_event(&mut completed, &added).expect("function item");
    let arguments_done = json!({
        "type": "response.function_call_arguments.done",
        "item_id": "streamed-item",
        "name": "read_file",
        "arguments": "{}"
    });
    push_event(&mut completed, &arguments_done).expect("completed arguments");
    let mut completed_item = function_item("streamed-item", "streamed-call", "completed", "{}");
    completed_item
        .as_object_mut()
        .expect("function object")
        .remove("id");
    let event = completed_event(vec![completed_item]);
    let error = push_event(&mut completed, &event).expect_err("missing completed item id");
    assert_eq!(error.code, "provider_protocol");
}

fn push_event(
    decoder: &mut SseDecoder,
    event: &Value,
) -> Result<Vec<ModelEvent>, forge_runtime_domain::ProviderError> {
    let kind = event["type"].as_str().expect("event type");
    let frame = format!("event: {kind}\ndata: {event}\n\n");
    decoder.push(frame.as_bytes())
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

fn incomplete_event(output: Vec<Value>, details: Option<Value>, usage: Option<Value>) -> Value {
    let mut response = json!({
        "status": "incomplete",
        "output": Value::Array(output),
        "usage": usage.unwrap_or(Value::Null)
    });
    if let Some(details) = details {
        response["incomplete_details"] = details;
    }
    json!({"type": "response.incomplete", "response": response})
}

fn message_item(id: &str, text: &str, status: &str) -> Value {
    json!({
        "type": "message",
        "id": id,
        "status": status,
        "role": "assistant",
        "phase": "final_answer",
        "content": [{
            "type": "output_text",
            "text": text,
            "annotations": []
        }]
    })
}

fn function_added(id: &str, call_id: &str) -> Value {
    json!({
        "type": "response.output_item.added",
        "item": {
            "type": "function_call",
            "id": id,
            "call_id": call_id,
            "name": "read_file"
        }
    })
}

fn function_item(id: &str, call_id: &str, status: &str, arguments: &str) -> Value {
    json!({
        "type": "function_call",
        "id": id,
        "call_id": call_id,
        "name": "read_file",
        "arguments": arguments,
        "status": status
    })
}

fn reasoning_item(id: &str, status: &str) -> Value {
    json!({
        "type": "reasoning",
        "id": id,
        "summary": [],
        "encrypted_content": "ciphertext",
        "status": status
    })
}
