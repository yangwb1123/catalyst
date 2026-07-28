use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::runtime_domain::{ProviderError, ToolCall};

use super::output_semantics::{
    ItemStatus, MessageProjection, OutputTerminal, message_phase, optional_item_status,
    required_item_status, select_assistant_text,
};

pub(super) const OPENAI_RESPONSES_CONTEXT: &str = "openai.responses";

const MAX_OUTPUT_ITEMS: usize = 64;
const MAX_CONTEXT_BYTES: usize = 2 * 1024 * 1024;
const MAX_ID_BYTES: usize = 1_024;
const MAX_NAME_BYTES: usize = 1_024;
const MAX_CONTENT_PARTS: usize = 64;
const MAX_REASONING_CONTENT_PARTS: usize = 64;
const MAX_SUMMARY_PARTS: usize = 64;
const MAX_SUMMARY_TEXT_BYTES: usize = 256 * 1024;
const MAX_REASONING_TEXT_BYTES: usize = 256 * 1024;
const MAX_ARGUMENT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Default, PartialEq)]
pub(super) struct OutputProjection {
    pub(super) assistant_text: String,
    pub(super) tool_calls: Vec<ToolCall>,
    pub(super) tool_item_ids: BTreeMap<String, Option<String>>,
    pub(super) incomplete_tool_calls: Vec<IncompleteToolCall>,
    pub(super) messages: Vec<MessageProjection>,
}

#[derive(Debug, PartialEq)]
pub(super) struct IncompleteToolCall {
    pub(super) item_id: Option<String>,
    pub(super) call_id: String,
    pub(super) name: String,
}

pub(super) struct ValidatedOutput {
    pub(super) items: Vec<Value>,
    pub(super) projection: OutputProjection,
}

pub(super) fn validated_output_items(
    items: &[Value],
    terminal: OutputTerminal,
) -> Result<ValidatedOutput, ProviderError> {
    if items.len() > MAX_OUTPUT_ITEMS {
        return Err(protocol_error("response output exceeded the item limit"));
    }
    let mut ids = BTreeSet::new();
    let mut call_ids = BTreeSet::new();
    let mut bytes = 0_usize;
    let mut projection = OutputProjection::default();
    let mut messages = Vec::new();
    for item in items {
        validate_item(
            item,
            terminal,
            &mut ids,
            &mut call_ids,
            &mut projection,
            &mut messages,
        )?;
        bytes = charge_item(item, bytes)?;
    }
    projection.assistant_text =
        select_assistant_text(&messages, !projection.tool_calls.is_empty(), terminal)?;
    projection.messages = messages;
    Ok(ValidatedOutput {
        items: items.to_vec(),
        projection,
    })
}

fn validate_item(
    item: &Value,
    terminal: OutputTerminal,
    ids: &mut BTreeSet<String>,
    call_ids: &mut BTreeSet<String>,
    projection: &mut OutputProjection,
    messages: &mut Vec<MessageProjection>,
) -> Result<(), ProviderError> {
    let object = object(item, "output item")?;
    let kind = required_string(object, "type", 64)?;
    match kind {
        "reasoning" => validate_reasoning(object, terminal),
        "function_call" => validate_function_call(object, terminal, call_ids, projection),
        "message" => validate_message(object, terminal, messages),
        _ => Err(protocol_error(
            "response contained an unsupported output item",
        )),
    }?;
    if let Some(id) = optional_string(object, "id", MAX_ID_BYTES)?
        && !ids.insert(id.to_owned())
    {
        return Err(protocol_error(
            "response output contained a duplicate item id",
        ));
    }
    Ok(())
}

fn validate_reasoning(
    object: &Map<String, Value>,
    terminal: OutputTerminal,
) -> Result<(), ProviderError> {
    allowed_keys(
        object,
        &[
            "type",
            "id",
            "summary",
            "encrypted_content",
            "content",
            "status",
        ],
    )?;
    required_string(object, "id", MAX_ID_BYTES)?;
    required_string(object, "encrypted_content", MAX_CONTEXT_BYTES)?;
    validate_summary(required_array(object, "summary")?)?;
    if let Some(content) = optional_array(object, "content")? {
        validate_reasoning_content(content)?;
    }
    optional_item_status(object, terminal)?;
    Ok(())
}

fn validate_summary(parts: &[Value]) -> Result<(), ProviderError> {
    if parts.len() > MAX_SUMMARY_PARTS {
        return Err(protocol_error("reasoning summary exceeded the part limit"));
    }
    let mut text_bytes = 0_usize;
    for part in parts {
        let object = object(part, "reasoning summary part")?;
        allowed_keys(object, &["type", "text"])?;
        exact_string(object, "type", "summary_text")?;
        let text = required_string(object, "text", MAX_SUMMARY_TEXT_BYTES)?;
        text_bytes = text_bytes.saturating_add(text.len());
        if text_bytes > MAX_SUMMARY_TEXT_BYTES {
            return Err(protocol_error("reasoning summary exceeded the byte limit"));
        }
    }
    Ok(())
}

fn validate_reasoning_content(parts: &[Value]) -> Result<(), ProviderError> {
    if parts.len() > MAX_REASONING_CONTENT_PARTS {
        return Err(protocol_error("reasoning content exceeded the part limit"));
    }
    let mut text_bytes = 0_usize;
    for part in parts {
        let object = object(part, "reasoning content part")?;
        allowed_keys(object, &["type", "text"])?;
        exact_string(object, "type", "reasoning_text")?;
        let text = required_string_allow_empty(object, "text", MAX_REASONING_TEXT_BYTES)?;
        text_bytes = text_bytes.saturating_add(text.len());
        if text_bytes > MAX_REASONING_TEXT_BYTES {
            return Err(protocol_error("reasoning content exceeded the byte limit"));
        }
    }
    Ok(())
}

fn validate_function_call(
    object: &Map<String, Value>,
    terminal: OutputTerminal,
    call_ids: &mut BTreeSet<String>,
    projection: &mut OutputProjection,
) -> Result<(), ProviderError> {
    allowed_keys(
        object,
        &[
            "type",
            "id",
            "call_id",
            "name",
            "arguments",
            "status",
            "caller",
            "namespace",
        ],
    )?;
    let item_id = optional_string(object, "id", MAX_ID_BYTES)?.map(str::to_owned);
    let call_id = required_string(object, "call_id", MAX_ID_BYTES)?;
    if !call_ids.insert(call_id.to_owned()) {
        return Err(protocol_error(
            "response output contained a duplicate function call id",
        ));
    }
    let name = required_string(object, "name", MAX_NAME_BYTES)?;
    let arguments = required_string(object, "arguments", MAX_ARGUMENT_BYTES)?;
    let status = optional_item_status(object, terminal)?;
    validate_direct_caller(object.get("caller"))?;
    validate_absent_namespace(object.get("namespace"))?;
    if status == ItemStatus::Incomplete {
        projection.incomplete_tool_calls.push(IncompleteToolCall {
            item_id,
            call_id: call_id.to_owned(),
            name: name.to_owned(),
        });
        return Ok(());
    }
    let arguments = serde_json::from_str(arguments)
        .map_err(|_| protocol_error("function call arguments were not valid JSON"))?;
    projection.tool_calls.push(ToolCall {
        id: call_id.to_owned(),
        name: name.to_owned(),
        arguments,
    });
    projection.tool_item_ids.insert(call_id.to_owned(), item_id);
    Ok(())
}

fn validate_message(
    object: &Map<String, Value>,
    terminal: OutputTerminal,
    messages: &mut Vec<MessageProjection>,
) -> Result<(), ProviderError> {
    allowed_keys(
        object,
        &["type", "id", "status", "role", "content", "phase"],
    )?;
    let id = required_string(object, "id", MAX_ID_BYTES)?;
    exact_string(object, "role", "assistant")?;
    required_item_status(object, terminal)?;
    let phase = message_phase(object.get("phase"))?;
    let content = required_array(object, "content")?;
    if content.len() > MAX_CONTENT_PARTS {
        return Err(protocol_error(
            "assistant content had an invalid part count",
        ));
    }
    let mut text = String::new();
    for part in content {
        text.push_str(validate_content_part(part)?);
    }
    messages.push(MessageProjection {
        id: id.to_owned(),
        text,
        phase,
    });
    Ok(())
}

fn validate_content_part(part: &Value) -> Result<&str, ProviderError> {
    let object = object(part, "assistant content part")?;
    match required_string(object, "type", 64)? {
        "output_text" => {
            allowed_keys(object, &["type", "text", "annotations", "logprobs"])?;
            // This adapter enables no hosted tools or top-logprob output.
            validate_empty_array_field(object, "annotations")?;
            validate_optional_empty_array_field(object, "logprobs")?;
            required_string_allow_empty(object, "text", MAX_CONTEXT_BYTES)
        }
        "refusal" => {
            allowed_keys(object, &["type", "refusal"])?;
            required_string_allow_empty(object, "refusal", MAX_CONTEXT_BYTES)
        }
        _ => Err(protocol_error(
            "assistant message contained an unsupported content part",
        )),
    }
}

fn charge_item(item: &Value, current: usize) -> Result<usize, ProviderError> {
    let encoded = serde_json::to_vec(item)
        .map_err(|_| protocol_error("response output item could not be encoded"))?;
    if encoded.len() > MAX_CONTEXT_BYTES.saturating_sub(current) {
        return Err(protocol_error("response output exceeded the byte limit"));
    }
    Ok(current + encoded.len())
}

fn allowed_keys(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), ProviderError> {
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(protocol_error(
            "response output item contained an unsupported field",
        ));
    }
    Ok(())
}

fn validate_direct_caller(value: Option<&Value>) -> Result<(), ProviderError> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(());
    };
    let object = object(value, "function call caller")?;
    allowed_keys(object, &["type"])?;
    exact_string(object, "type", "direct")
}

fn validate_absent_namespace(value: Option<&Value>) -> Result<(), ProviderError> {
    if value.is_none_or(Value::is_null) {
        Ok(())
    } else {
        Err(protocol_error(
            "namespaced function calls are not enabled by this provider",
        ))
    }
}

fn exact_string(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), ProviderError> {
    let value = required_string(object, field, expected.len())?;
    if value != expected {
        return Err(protocol_error("response output item had an invalid field"));
    }
    Ok(())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    max_bytes: usize,
) -> Result<&'a str, ProviderError> {
    let value = required_string_allow_empty(object, field, max_bytes)?;
    if value.is_empty() {
        return Err(protocol_error("response output item had an empty field"));
    }
    Ok(value)
}

fn required_string_allow_empty<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    max_bytes: usize,
) -> Result<&'a str, ProviderError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| protocol_error("response output item omitted a string field"))?;
    if value.len() > max_bytes {
        return Err(protocol_error(
            "response output string exceeded the byte limit",
        ));
    }
    Ok(value)
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
    max_bytes: usize,
) -> Result<Option<&'a str>, ProviderError> {
    match object.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(_) => required_string(object, field, max_bytes).map(Some),
    }
}

fn required_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a [Value], ProviderError> {
    object
        .get(field)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| protocol_error("response output item omitted an array field"))
}

fn optional_array<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a [Value]>, ProviderError> {
    match object.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(_) => required_array(object, field).map(Some),
    }
}

fn validate_empty_array_field(
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), ProviderError> {
    let value = object
        .get(field)
        .ok_or_else(|| protocol_error("assistant content omitted an array field"))?;
    validate_empty_array(value, field)
}

fn validate_optional_empty_array_field(
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), ProviderError> {
    match object.get(field) {
        Some(value) => validate_empty_array(value, field),
        None => Ok(()),
    }
}

fn validate_empty_array(value: &Value, label: &str) -> Result<(), ProviderError> {
    if value.as_array().is_some_and(Vec::is_empty) {
        Ok(())
    } else {
        Err(protocol_error(&format!("{label} was not an empty array")))
    }
}

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>, ProviderError> {
    value
        .as_object()
        .ok_or_else(|| protocol_error(&format!("{label} was not an object")))
}

fn protocol_error(message: &str) -> ProviderError {
    ProviderError::new("provider_protocol", message, false)
}
