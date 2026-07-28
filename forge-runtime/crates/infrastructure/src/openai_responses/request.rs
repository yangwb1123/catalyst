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
