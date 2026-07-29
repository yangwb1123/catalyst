use std::collections::BTreeMap;

use serde_json::{Value, json};

use crate::runtime_domain::{Message, ModelRequest, ProviderError, ToolCall, ToolSpec};

use super::{
    output_items::{OPENAI_RESPONSES_CONTEXT, OutputProjection, validated_output_items},
    output_semantics::OutputTerminal,
};

pub(super) fn encode_request(model: &str, request: &ModelRequest) -> Result<Value, ProviderError> {
    Ok(json!({
        "model": model,
        "instructions": request.system_prompt,
        "input": encode_messages(&request.messages)?,
        "include": ["reasoning.encrypted_content"],
        "tools": request.tools.iter().map(encode_tool).collect::<Vec<_>>(),
        "max_output_tokens": request.max_output_tokens,
        "stream": true,
        "store": false,
    }))
}

pub(super) fn encode_request_bytes(
    model: &str,
    request: &ModelRequest,
) -> Result<Vec<u8>, ProviderError> {
    canonical_json_bytes(encode_request(model, request)?)
}

pub(super) fn validate_exact_request_bytes(
    model: &str,
    expected_request: &ModelRequest,
    actual_bytes: &[u8],
) -> Result<(), ProviderError> {
    let expected_bytes = encode_request_bytes(model, expected_request)?;
    if actual_bytes == expected_bytes.as_slice() {
        Ok(())
    } else {
        Err(protocol_error(
            "prepared request body does not exactly match the expected request",
        ))
    }
}

pub(super) fn validate_request_bytes(
    configured_model: &str,
    bytes: &[u8],
) -> Result<(), ProviderError> {
    let value = serde_json::from_slice::<Value>(bytes)
        .map_err(|_| protocol_error("prepared request body is not valid JSON"))?;
    let canonical = canonical_json_bytes(value.clone())?;
    if canonical != bytes {
        return Err(protocol_error(
            "prepared request body is not canonical JSON",
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| protocol_error("prepared request body must be a JSON object"))?;
    if object.get("model").and_then(Value::as_str) != Some(configured_model) {
        return Err(protocol_error(
            "prepared request model does not match the configured model",
        ));
    }
    if object.get("store") != Some(&Value::Bool(false)) {
        return Err(protocol_error("prepared request must set store to false"));
    }
    if object.get("stream") != Some(&Value::Bool(true)) {
        return Err(protocol_error("prepared request must set stream to true"));
    }
    Ok(())
}

fn canonical_json_bytes(value: Value) -> Result<Vec<u8>, ProviderError> {
    serde_json::to_vec(&sort_json(value))
        .map_err(|_| protocol_error("provider request could not be encoded"))
}

fn sort_json(value: Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.into_iter().map(sort_json).collect()),
        Value::Object(items) => {
            let sorted = items
                .into_iter()
                .map(|(key, value)| (key, sort_json(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect())
        }
        other => other,
    }
}

fn encode_messages(messages: &[Message]) -> Result<Vec<Value>, ProviderError> {
    let mut input = Vec::new();
    let mut expected_projection = None;
    for message in messages {
        if expected_projection.is_some() && !matches!(message, Message::Assistant { .. }) {
            return Err(protocol_error(
                "provider output context was not followed by its assistant projection",
            ));
        }
        match message {
            Message::User { text } => input.push(message_item("user", text)),
            Message::ProviderContext { provider, items }
                if provider == OPENAI_RESPONSES_CONTEXT =>
            {
                let validated = validated_output_items(items, OutputTerminal::Completed)?;
                if validated.items.is_empty() {
                    return Err(protocol_error("provider output context was empty"));
                }
                input.extend(validated.items);
                expected_projection = Some(validated.projection);
            }
            Message::ProviderContext { .. } => {}
            Message::Assistant { text, tool_calls } => {
                if let Some(projection) = expected_projection.take() {
                    validate_projection(&projection, text, tool_calls)?;
                } else {
                    encode_assistant(&mut input, text, tool_calls);
                }
            }
            Message::Tool {
                call_id, output, ..
            } => input.push(json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": output,
            })),
        }
    }
    if expected_projection.is_some() {
        return Err(protocol_error(
            "provider output context omitted its assistant projection",
        ));
    }
    Ok(input)
}

fn encode_assistant(input: &mut Vec<Value>, text: &str, tool_calls: &[ToolCall]) {
    if !text.is_empty() {
        input.push(message_item("assistant", text));
    }
    input.extend(tool_calls.iter().map(encode_call));
}

fn validate_projection(
    expected: &OutputProjection,
    text: &str,
    tool_calls: &[ToolCall],
) -> Result<(), ProviderError> {
    if expected.assistant_text != text || expected.tool_calls != tool_calls {
        return Err(protocol_error(
            "provider output context did not match its assistant projection",
        ));
    }
    Ok(())
}

fn message_item(role: &str, text: &str) -> Value {
    json!({
        "type": "message",
        "role": role,
        "content": text,
    })
}

fn encode_call(call: &ToolCall) -> Value {
    json!({
        "type": "function_call",
        "call_id": call.id,
        "name": call.name,
        "arguments": call.arguments.to_string(),
    })
}

fn encode_tool(tool: &ToolSpec) -> Value {
    json!({
        "type": "function",
        "name": tool.name,
        "description": tool.description,
        "parameters": tool.input_schema,
    })
}

fn protocol_error(message: &str) -> ProviderError {
    ProviderError::new("provider_protocol", message, false)
}
