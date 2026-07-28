use forge_runtime_domain::{Cancellation, Message, ModelEvent, ModelFinishReason, ModelRequest};
use serde_json::json;

use super::{
    is_event_stream_content_type,
    request::encode_request,
    sse::{MAX_BUFFER_BYTES, SseDecoder},
    transport_error,
};

const SECRET: &str = "sk-reasoning-test";

#[test]
fn event_stream_media_type_rejects_combined_or_malformed_values() {
    assert!(is_event_stream_content_type("text/event-stream"));
    assert!(is_event_stream_content_type(
        "Text/Event-Stream; charset=\"UTF-8\""
    ));
    assert!(!is_event_stream_content_type(
        "text/event-stream; charset=utf-8, application/json"
    ));
    assert!(!is_event_stream_content_type(
        "text/event-stream; unsupported=value"
    ));
    assert!(!is_event_stream_content_type(
        "text/event-stream; charset=\"utf-8"
    ));
    assert!(!is_event_stream_content_type("text/event-stream;"));
}

#[test]
fn completed_output_is_validated_and_replayed_without_projection_duplicates() {
    let events = decoded_output();
    assert_eq!(
        events[0],
        ModelEvent::TextDelta {
            delta: "answer".into()
        }
    );
    let ModelEvent::ProviderContext { provider, items } = &events[1] else {
        panic!("provider context before finish");
    };
    assert_eq!(provider, "openai.responses");
    assert_eq!(
        events[2],
        ModelEvent::Finished {
            reason: ModelFinishReason::Completed
        }
    );
    assert_replayed_request(items);
}

fn decoded_output() -> Vec<ModelEvent> {
    let valid = json!({
        "type": "reasoning",
        "id": "rs_123",
        "summary": [{"type": "summary_text", "text": "opaque state"}],
        "content": [{"type": "reasoning_text", "text": "bounded reasoning"}],
        "encrypted_content": "encrypted-token",
        "status": "completed"
    });
    let message = assistant_output("answer", "final_answer");
    let terminal = json!({
        "type": "response.completed",
        "response": {
            "status": "completed",
            "output": [valid, message],
            "usage": null
        }
    });
    let frame = format!(
        "event: response.output_item.added\n\
         data: {{\"type\":\"response.output_item.added\",\"item\":{{\"type\":\"message\",\
         \"id\":\"msg_123\",\"role\":\"assistant\",\"phase\":\"final_answer\"}}}}\n\n\
         event: response.output_text.delta\n\
         data: {{\"type\":\"response.output_text.delta\",\"item_id\":\"msg_123\",\
         \"delta\":\"answer\"}}\n\n\
         event: response.completed\n\
         data: {terminal}\n\n"
    );
    let mut decoder = SseDecoder::new(SECRET);
    decoder.push(frame.as_bytes()).expect("completed response")
}

fn assert_replayed_request(items: &[serde_json::Value]) {
    let encoded = encode_request("test-model", &request_with_context(items.to_vec()))
        .expect("valid provider context");
    assert_eq!(encoded["include"], json!(["reasoning.encrypted_content"]));
    assert_eq!(encoded["input"][0]["content"], "first");
    assert_eq!(encoded["input"][1], items[0]);
    assert_eq!(encoded["input"][2], items[1]);
    assert_eq!(encoded["input"][3]["content"], "follow up");
    assert_eq!(encoded["input"].as_array().expect("input array").len(), 4);
}

#[test]
fn unknown_or_malformed_provider_output_fails_closed() {
    let request = request_with_context(vec![json!({
        "type": "reasoning",
        "id": "rs_123",
        "summary": [],
        "encrypted_content": "cipher",
        "unvalidated": {"type": "computer_call"}
    })]);

    let error = encode_request("test-model", &request).expect_err("unsupported field");

    assert_eq!(error.code, "provider_protocol");
    assert!(!error.retryable);
}

#[test]
fn optional_direct_function_metadata_is_preserved() {
    let item = json!({
        "type": "function_call",
        "call_id": "call-optional",
        "name": "read_file",
        "arguments": "{\"path\":\"README.md\"}",
        "caller": {"type": "direct"},
        "namespace": null
    });
    let request = request_with_function_context(item.clone());

    let encoded = encode_request("test-model", &request).expect("valid direct call");

    assert_eq!(encoded["input"][1], item);
    assert_eq!(encoded["input"][2]["type"], "function_call_output");
    assert_eq!(encoded["input"].as_array().expect("input array").len(), 3);
}

#[test]
fn in_progress_terminal_context_is_rejected() {
    let mut item = assistant_output("answer", "commentary");
    item["status"] = json!("in_progress");
    let request = request_with_context(vec![item]);

    let error = encode_request("test-model", &request).expect_err("in-progress context");

    assert_eq!(error.code, "provider_protocol");
}

#[test]
fn explicit_null_phase_is_preserved_but_missing_ciphertext_is_rejected() {
    let mut message = assistant_output("answer", "commentary");
    message["phase"] = serde_json::Value::Null;
    let encoded = encode_request("test-model", &request_with_context(vec![message]))
        .expect("null optional phase");
    assert!(encoded["input"][1]["phase"].is_null());

    let reasoning = json!({
        "type": "reasoning",
        "id": "rs-no-cipher",
        "summary": [],
        "encrypted_content": null
    });
    let error = encode_request("test-model", &request_with_context(vec![reasoning]))
        .expect_err("stateless reasoning requires ciphertext");
    assert_eq!(error.code, "provider_protocol");
}

#[test]
fn terminal_unknown_output_item_fails_closed() {
    let terminal = json!({
        "type": "response.completed",
        "response": {
            "status": "completed",
            "output": [{"type": "computer_call", "id": "unsafe"}],
            "usage": null
        }
    });
    let frame = format!("event: response.completed\ndata: {terminal}\n\n");
    let mut decoder = SseDecoder::new(SECRET);

    let error = decoder
        .push(frame.as_bytes())
        .expect_err("unsupported item");

    assert_eq!(error.code, "provider_protocol");
}

#[test]
fn parallel_calls_are_emitted_in_terminal_output_order() {
    let mut decoder = SseDecoder::new(SECRET);

    let events = decoder
        .push(reverse_completion_stream().as_bytes())
        .expect("valid parallel calls");
    let call_ids = events
        .iter()
        .filter_map(|event| match event {
            ModelEvent::ToolCall { call } => Some(call.id.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(call_ids, ["call-a", "call-b"]);
    let ModelEvent::ProviderContext { items, .. } = &events[2] else {
        panic!("provider context follows ordered calls");
    };
    assert_eq!(items[0]["call_id"], "call-a");
    assert_eq!(items[1]["call_id"], "call-b");
}

#[test]
fn oversized_single_chunk_is_rejected_before_decoder_state_changes() {
    let mut decoder = SseDecoder::new(SECRET);
    let chunk = vec![b'x'; MAX_BUFFER_BYTES + 1];

    let error = decoder.push(&chunk).expect_err("oversized transport chunk");

    assert_eq!(error.code, "provider_protocol");
    assert!(error.message.contains("buffer size limit"));
    assert!(
        decoder
            .push(b"data: {\"type\":\"response.created\"}\n\n")
            .expect("decoder remains usable")
            .is_empty()
    );
}

#[test]
fn drip_fed_event_uses_incremental_scanning_and_disables_transport_retry() {
    let padding = "x".repeat(16 * 1024);
    let frame = format!("data: {{\"type\":\"response.created\",\"padding\":\"{padding}\"}}\n\n");
    let mut decoder = SseDecoder::new(SECRET);
    for byte in frame.as_bytes() {
        assert!(decoder.push(std::slice::from_ref(byte)).is_ok());
    }

    let error = transport_error(!decoder.has_decoded_event());

    assert!(decoder.has_decoded_event());
    assert!(!error.retryable);
}

fn request_with_context(items: Vec<serde_json::Value>) -> ModelRequest {
    ModelRequest {
        system_prompt: "system".into(),
        messages: vec![
            Message::User {
                text: "first".into(),
            },
            Message::ProviderContext {
                provider: "openai.responses".into(),
                items,
            },
            Message::Assistant {
                text: "answer".into(),
                tool_calls: Vec::new(),
            },
            Message::ProviderContext {
                provider: "another.provider".into(),
                items: vec![json!({"type": "reasoning"})],
            },
            Message::User {
                text: "follow up".into(),
            },
        ],
        tools: Vec::new(),
        max_output_tokens: 128,
        cancellation: Cancellation::default(),
    }
}

fn request_with_function_context(item: serde_json::Value) -> ModelRequest {
    ModelRequest {
        system_prompt: "system".into(),
        messages: vec![
            Message::User {
                text: "first".into(),
            },
            Message::ProviderContext {
                provider: "openai.responses".into(),
                items: vec![item],
            },
            Message::Assistant {
                text: String::new(),
                tool_calls: vec![forge_runtime_domain::ToolCall {
                    id: "call-optional".into(),
                    name: "read_file".into(),
                    arguments: json!({"path": "README.md"}),
                }],
            },
            Message::Tool {
                call_id: "call-optional".into(),
                name: "read_file".into(),
                output: "contents".into(),
                is_error: false,
                truncated: false,
            },
        ],
        tools: Vec::new(),
        max_output_tokens: 128,
        cancellation: Cancellation::default(),
    }
}

fn assistant_output(text: &str, phase: &str) -> serde_json::Value {
    json!({
        "type": "message",
        "id": "msg_123",
        "status": "completed",
        "role": "assistant",
        "phase": phase,
        "content": [{
            "type": "output_text",
            "text": text,
            "annotations": [],
            "logprobs": []
        }]
    })
}

fn reverse_completion_stream() -> String {
    let call_a = function_item("item-a", "call-a", "a");
    let call_b = function_item("item-b", "call-b", "b");
    format!(
        "data: {{\"type\":\"response.output_item.added\",\"item\":{call_a}}}\n\n\
         data: {{\"type\":\"response.output_item.added\",\"item\":{call_b}}}\n\n\
         data: {{\"type\":\"response.function_call_arguments.done\",\
         \"item_id\":\"item-b\",\"name\":\"b\",\"arguments\":\"{{}}\"}}\n\n\
         data: {{\"type\":\"response.function_call_arguments.done\",\
         \"item_id\":\"item-a\",\"name\":\"a\",\"arguments\":\"{{}}\"}}\n\n\
         data: {{\"type\":\"response.completed\",\"response\":\
         {{\"status\":\"completed\",\"output\":[{call_a},{call_b}],\"usage\":null}}}}\n\n"
    )
}

fn function_item(id: &str, call_id: &str, name: &str) -> serde_json::Value {
    json!({
        "type": "function_call",
        "id": id,
        "call_id": call_id,
        "name": name,
        "arguments": "{}",
        "status": "completed"
    })
}
