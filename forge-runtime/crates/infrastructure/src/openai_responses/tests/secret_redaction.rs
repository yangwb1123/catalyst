use crate::runtime_domain::ModelEvent;
use serde_json::{Value, json};

use super::sse::SseDecoder;

const SECRET: &str = "sk-mock-secret-never-log";

#[test]
fn redacts_the_api_key_from_successful_events_and_provider_context() {
    assert_redacted(&[&format!("provider echoed {SECRET}")]);
}

#[test]
fn redacts_an_api_key_split_across_streamed_deltas() {
    assert_redacted(&["provider echoed sk-mock-secret-", "never-log"]);
}

fn assert_redacted(deltas: &[&str]) {
    let events = SseDecoder::new(SECRET)
        .push(secret_stream(deltas).as_bytes())
        .expect("valid redacted stream");
    let encoded = serde_json::to_string(&events).expect("serializable events");
    let emitted = events
        .iter()
        .filter_map(|event| match event {
            ModelEvent::TextDelta { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect::<String>();

    assert!(!encoded.contains(SECRET));
    assert!(!emitted.contains(SECRET));
    assert_eq!(emitted, "provider echoed [REDACTED]");
    assert!(encoded.contains("[REDACTED]"));
    assert!(events.iter().any(|event| matches!(
        event,
        ModelEvent::ProviderContext { items, .. }
            if !serialized(items).contains(SECRET)
                && serialized(items).contains("[REDACTED]")
    )));
}

fn secret_stream(deltas: &[&str]) -> String {
    let item_id = format!("message-{SECRET}");
    let text = format!("provider echoed {SECRET}");
    let mut frames = vec![added_frame(&item_id)];
    frames.extend(deltas.iter().map(|delta| delta_frame(&item_id, delta)));
    frames.push(completed_frame(&item_id, &text));
    frames.concat()
}

fn added_frame(item_id: &str) -> String {
    frame(
        "response.output_item.added",
        &json!({
            "type": "response.output_item.added",
            "item": {
                "type": "message",
                "id": item_id,
                "role": "assistant",
                "status": "in_progress"
            }
        }),
    )
}

fn delta_frame(item_id: &str, delta: &str) -> String {
    frame(
        "response.output_text.delta",
        &json!({
            "type": "response.output_text.delta",
            "item_id": item_id,
            "delta": delta
        }),
    )
}

fn completed_frame(item_id: &str, text: &str) -> String {
    frame(
        "response.completed",
        &json!({
            "type": "response.completed",
            "response": {
                "status": "completed",
                "output": [{
                    "type": "message",
                    "id": item_id,
                    "status": "completed",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": text,
                        "annotations": []
                    }]
                }],
                "usage": null
            }
        }),
    )
}

fn frame(event: &str, data: &Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

fn serialized(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("serializable value")
}
