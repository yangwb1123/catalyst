use serde::Deserialize;

use crate::runtime_domain::ProviderError;

use super::redaction::redact_json_strings;

pub(super) fn frame_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"\n\n") {
            return Some((index, 2));
        }
        if bytes[index..].starts_with(b"\r\n\r\n") {
            return Some((index, 4));
        }
        index += 1;
    }
    None
}

pub(super) fn parse_frame(
    frame: &[u8],
    secret: &str,
) -> Result<Option<StreamEvent>, ProviderError> {
    let text =
        std::str::from_utf8(frame).map_err(|_| protocol_error("SSE event was not valid UTF-8"))?;
    let mut event_name = None;
    let mut data = Vec::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("event:") {
            event_name = Some(value.trim());
        } else if let Some(value) = line.strip_prefix("data:") {
            data.push(value.trim_start());
        }
    }
    if data.is_empty() || data == ["[DONE]"] {
        return Ok(None);
    }
    let mut value: serde_json::Value = serde_json::from_str(&data.join("\n"))
        .map_err(|_| protocol_error("SSE data was not a valid typed event"))?;
    redact_json_strings(&mut value, secret);
    let event: StreamEvent = serde_json::from_value(value)
        .map_err(|_| protocol_error("SSE data was not a valid typed event"))?;
    validate_event_name(event_name, &event)?;
    Ok(Some(event))
}

fn validate_event_name(event_name: Option<&str>, event: &StreamEvent) -> Result<(), ProviderError> {
    if let (Some(name), Some(kind)) = (event_name, event.kind())
        && name != kind
    {
        return Err(protocol_error("SSE event name did not match its data type"));
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub(super) enum StreamEvent {
    #[serde(rename = "response.output_text.delta")]
    TextDelta { item_id: String, delta: String },
    #[serde(rename = "response.refusal.delta")]
    RefusalDelta { item_id: String, delta: String },
    #[serde(rename = "response.output_item.added")]
    OutputItemAdded { item: OutputItem },
    #[serde(rename = "response.function_call_arguments.done")]
    FunctionArgumentsDone {
        item_id: String,
        name: String,
        arguments: String,
    },
    #[serde(rename = "response.completed")]
    Completed { response: CompletedResponse },
    #[serde(rename = "response.incomplete")]
    Incomplete { response: CompletedResponse },
    #[serde(rename = "response.failed")]
    Failed { response: FailedResponse },
    #[serde(rename = "error")]
    Error {
        code: Option<String>,
        message: String,
    },
    #[serde(other)]
    Other,
}

impl StreamEvent {
    fn kind(&self) -> Option<&'static str> {
        match self {
            Self::TextDelta { .. } => Some("response.output_text.delta"),
            Self::RefusalDelta { .. } => Some("response.refusal.delta"),
            Self::OutputItemAdded { .. } => Some("response.output_item.added"),
            Self::FunctionArgumentsDone { .. } => Some("response.function_call_arguments.done"),
            Self::Completed { .. } => Some("response.completed"),
            Self::Incomplete { .. } => Some("response.incomplete"),
            Self::Failed { .. } => Some("response.failed"),
            Self::Error { .. } => Some("error"),
            Self::Other => None,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct OutputItem {
    pub(super) id: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) call_id: Option<String>,
    pub(super) name: Option<String>,
    pub(super) role: Option<String>,
    pub(super) phase: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct CompletedResponse {
    pub(super) status: String,
    #[serde(default)]
    pub(super) output: Vec<serde_json::Value>,
    pub(super) usage: Option<WireUsage>,
    pub(super) incomplete_details: Option<IncompleteDetails>,
}

#[derive(Deserialize)]
pub(super) struct IncompleteDetails {
    pub(super) reason: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct FailedResponse {
    #[serde(default)]
    pub(super) error: Option<ResponseFailure>,
}

#[derive(Default, Deserialize)]
pub(super) struct ResponseFailure {
    #[serde(default)]
    pub(super) code: Option<String>,
    #[serde(default)]
    pub(super) message: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct WireUsage {
    pub(super) input_tokens: u64,
    pub(super) output_tokens: u64,
}

fn protocol_error(message: &str) -> ProviderError {
    ProviderError::new("provider_protocol", message, false)
}
