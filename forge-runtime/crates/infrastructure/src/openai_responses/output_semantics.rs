use serde_json::{Map, Value};

use crate::runtime_domain::ProviderError;

use super::sse_wire::IncompleteDetails;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OutputTerminal {
    Completed,
    Incomplete,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ItemStatus {
    Missing,
    Completed,
    Incomplete,
}

#[derive(Debug, PartialEq)]
pub(super) struct MessageProjection {
    pub(super) id: String,
    pub(super) text: String,
    pub(super) phase: Option<MessagePhase>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MessagePhase {
    Commentary,
    FinalAnswer,
}

pub(super) enum IncompleteReason {
    MaxOutputTokens,
    ContentFilter,
}

pub(super) fn optional_item_status(
    object: &Map<String, Value>,
    terminal: OutputTerminal,
) -> Result<ItemStatus, ProviderError> {
    let status = match object.get("status") {
        Some(Value::Null) | None => ItemStatus::Missing,
        Some(Value::String(value)) if value == "completed" => ItemStatus::Completed,
        Some(Value::String(value)) if value == "incomplete" => ItemStatus::Incomplete,
        Some(Value::String(_)) => {
            return Err(protocol_error("output item status was not supported"));
        }
        Some(_) => return Err(protocol_error("output item status was not a string")),
    };
    if terminal == OutputTerminal::Completed && status == ItemStatus::Incomplete {
        return Err(protocol_error(
            "completed response contained an incomplete output item",
        ));
    }
    Ok(status)
}

pub(super) fn required_item_status(
    object: &Map<String, Value>,
    terminal: OutputTerminal,
) -> Result<ItemStatus, ProviderError> {
    let status = optional_item_status(object, terminal)?;
    if status == ItemStatus::Missing {
        Err(protocol_error("response output item omitted status"))
    } else {
        Ok(status)
    }
}

pub(super) fn message_phase(value: Option<&Value>) -> Result<Option<MessagePhase>, ProviderError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(phase)) if phase == "commentary" => Ok(Some(MessagePhase::Commentary)),
        Some(Value::String(phase)) if phase == "final_answer" => {
            Ok(Some(MessagePhase::FinalAnswer))
        }
        Some(Value::String(_)) => Err(protocol_error("message phase was not supported")),
        Some(_) => Err(protocol_error("message phase was not a string")),
    }
}

fn joined_text(
    messages: &[MessageProjection],
    include: impl Fn(&MessageProjection) -> bool,
) -> String {
    messages
        .iter()
        .filter(|message| include(message))
        .map(|message| message.text.as_str())
        .collect()
}

pub(super) fn select_assistant_text(
    messages: &[MessageProjection],
    has_tools: bool,
    terminal: OutputTerminal,
) -> Result<String, ProviderError> {
    if terminal == OutputTerminal::Completed && messages.is_empty() {
        return has_tools
            .then(String::new)
            .ok_or_else(|| protocol_error("completed response omitted an assistant message"));
    }
    let has_final = messages
        .iter()
        .any(|message| message.phase != Some(MessagePhase::Commentary));
    let has_explicit_final = messages
        .iter()
        .any(|message| message.phase == Some(MessagePhase::FinalAnswer));
    if terminal == OutputTerminal::Completed && has_tools && has_explicit_final {
        return Err(protocol_error(
            "tool response contained a final-answer message",
        ));
    }
    if terminal == OutputTerminal::Completed && !has_tools && !has_final {
        return Err(protocol_error(
            "completed response omitted a final-answer message",
        ));
    }
    Ok(joined_text(messages, |message| {
        message.phase != Some(MessagePhase::Commentary)
    }))
}

pub(super) fn incomplete_reason(
    details: Option<&IncompleteDetails>,
) -> Result<IncompleteReason, ProviderError> {
    match details.and_then(|details| details.reason.as_deref()) {
        Some("max_output_tokens") => Ok(IncompleteReason::MaxOutputTokens),
        Some("content_filter") => Ok(IncompleteReason::ContentFilter),
        Some(_) => Err(protocol_error(
            "response incomplete reason was not supported",
        )),
        None => Err(protocol_error("response incomplete reason was omitted")),
    }
}

fn protocol_error(message: &str) -> ProviderError {
    ProviderError::new("provider_protocol", message, false)
}
