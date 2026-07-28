use serde_json::{Value, json};

pub(super) fn text_stream() -> &'static str {
    concat!(
        "event: response.created\n",
        "data: {\"type\":\"response.created\"}\n\n",
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"message\",",
        "\"id\":\"message-1\",\"role\":\"assistant\",\"status\":\"in_progress\"}}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"message-1\",",
        "\"delta\":\"hello \"}\n\n",
        "event: response.output_text.delta\n",
        "data: {\"type\":\"response.output_text.delta\",\"item_id\":\"message-1\",",
        "\"delta\":\"world\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",",
        "\"output\":[",
        "{\"type\":\"message\",\"id\":\"message-1\",\"status\":\"completed\",",
        "\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",",
        "\"text\":\"hello world\",\"annotations\":[]}]}],\"usage\":",
        "{\"input_tokens\":17,\"output_tokens\":2}}}\n\n",
    )
}

pub(super) fn function_stream() -> &'static str {
    concat!(
        "event: response.output_item.added\n",
        "data: {\"type\":\"response.output_item.added\",\"item\":",
        "{\"id\":\"item-1\",\"type\":\"function_call\",",
        "\"call_id\":\"call-provider-1\",\"name\":\"read_file\"}}\n\n",
        "event: response.function_call_arguments.done\n",
        "data: {\"type\":\"response.function_call_arguments.done\",",
        "\"item_id\":\"item-1\",\"name\":\"read_file\",",
        "\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\"}\n\n",
        "event: response.completed\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\",",
        "\"output\":[",
        "{\"type\":\"function_call\",\"id\":\"item-1\",",
        "\"call_id\":\"call-provider-1\",\"name\":\"read_file\",",
        "\"arguments\":\"{\\\"path\\\":\\\"README.md\\\"}\",",
        "\"status\":\"completed\"}],\"usage\":null}}\n\n",
    )
}

pub(super) fn message_output(id: &str, text: &str) -> Value {
    json!({
        "type": "message",
        "id": id,
        "status": "completed",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": text,
            "annotations": []
        }]
    })
}

pub(super) fn function_output() -> Value {
    json!({
        "type": "function_call",
        "id": "item-1",
        "call_id": "call-provider-1",
        "name": "read_file",
        "arguments": "{\"path\":\"README.md\"}",
        "status": "completed"
    })
}

pub(super) fn refusal_output() -> Value {
    json!({
        "type": "message",
        "id": "refusal-1",
        "status": "completed",
        "role": "assistant",
        "content": [{
            "type": "refusal",
            "refusal": "cannot comply"
        }]
    })
}
