use std::collections::{HashMap, HashSet};

use crate::runtime_domain::{ModelEvent, ModelFinishReason, ProviderError, ToolCall, Usage};

use super::{
    output_items::{OPENAI_RESPONSES_CONTEXT, OutputProjection, validated_output_items},
    output_semantics::{IncompleteReason, MessagePhase, OutputTerminal, incomplete_reason},
    redaction::{StreamingRedactor, redact_text},
    sse_wire::{
        CompletedResponse, OutputItem, StreamEvent, WireUsage, frame_boundary, parse_frame,
    },
};

pub(super) const MAX_BUFFER_BYTES: usize = 2 * 1024 * 1024;
pub(super) const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_RESPONSE_FRAMES: usize = 16_384;
pub(super) const MAX_PENDING_CALLS: usize = 64;

pub(super) struct SseDecoder {
    buffer: Vec<u8>,
    calls: HashMap<String, PendingCall>,
    messages: HashMap<String, StreamedMessage>,
    seen_items: HashSet<String>,
    secret: String,
    received_bytes: usize,
    frame_count: usize,
    scan_from: usize,
    decoded_event: bool,
    assistant_text: String,
    text_redactor: StreamingRedactor,
    completed_calls: HashMap<String, CompletedCall>,
    terminal: bool,
}

impl SseDecoder {
    pub(super) fn new(secret: &str) -> Self {
        Self {
            buffer: Vec::new(),
            calls: HashMap::new(),
            messages: HashMap::new(),
            seen_items: HashSet::new(),
            secret: secret.to_owned(),
            received_bytes: 0,
            frame_count: 0,
            scan_from: 0,
            decoded_event: false,
            assistant_text: String::new(),
            text_redactor: StreamingRedactor::new(secret),
            completed_calls: HashMap::new(),
            terminal: false,
        }
    }

    pub(super) fn push(&mut self, bytes: &[u8]) -> Result<Vec<ModelEvent>, ProviderError> {
        if bytes.len() > MAX_RESPONSE_BYTES.saturating_sub(self.received_bytes) {
            return Err(protocol_error("SSE response exceeded the total byte limit"));
        }
        if bytes.len() > MAX_BUFFER_BYTES.saturating_sub(self.buffer.len()) {
            return Err(protocol_error("SSE input exceeded the buffer size limit"));
        }
        self.buffer
            .try_reserve(bytes.len())
            .map_err(|_| protocol_error("failed to reserve bounded SSE input"))?;
        self.received_bytes += bytes.len();
        self.buffer.extend_from_slice(bytes);
        self.decode_available_frames()
    }

    pub(super) fn finish(&mut self) -> Result<Vec<ModelEvent>, ProviderError> {
        if self.buffer.iter().all(u8::is_ascii_whitespace) {
            self.buffer.clear();
            return Ok(Vec::new());
        }
        let frame = std::mem::take(&mut self.buffer);
        let mut events = Vec::new();
        self.charge_frame(frame.len())?;
        self.decode_frame(&frame, &mut events)?;
        Ok(events)
    }

    pub(super) fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub(super) fn has_decoded_event(&self) -> bool {
        self.decoded_event
    }

    fn decode_available_frames(&mut self) -> Result<Vec<ModelEvent>, ProviderError> {
        let mut events = Vec::new();
        let mut frame_start = 0;
        let mut search_start = self.scan_from.min(self.buffer.len());
        while let Some((offset, delimiter_len)) = frame_boundary(&self.buffer[search_start..]) {
            let end = search_start + offset;
            self.charge_frame(end - frame_start)?;
            let parsed = parse_frame(&self.buffer[frame_start..end], &self.secret)?;
            self.decode_parsed_frame(parsed, &mut events)?;
            frame_start = end + delimiter_len;
            search_start = frame_start;
        }
        if frame_start > 0 {
            self.buffer.drain(..frame_start);
        }
        self.scan_from = self.buffer.len().saturating_sub(3);
        Ok(events)
    }

    fn charge_frame(&mut self, frame_bytes: usize) -> Result<(), ProviderError> {
        if frame_bytes > MAX_BUFFER_BYTES {
            return Err(protocol_error("SSE event exceeded the size limit"));
        }
        if self.frame_count >= MAX_RESPONSE_FRAMES {
            return Err(protocol_error("SSE response exceeded the frame limit"));
        }
        self.frame_count += 1;
        Ok(())
    }

    fn decode_frame(
        &mut self,
        frame: &[u8],
        output: &mut Vec<ModelEvent>,
    ) -> Result<(), ProviderError> {
        let parsed = parse_frame(frame, &self.secret)?;
        self.decode_parsed_frame(parsed, output)
    }

    fn decode_parsed_frame(
        &mut self,
        event: Option<StreamEvent>,
        output: &mut Vec<ModelEvent>,
    ) -> Result<(), ProviderError> {
        if self.terminal {
            return Err(protocol_error("SSE event followed a terminal event"));
        }
        let Some(event) = event else {
            return Ok(());
        };
        self.decoded_event = true;
        self.decode_event(event, output)
    }

    fn decode_event(
        &mut self,
        event: StreamEvent,
        output: &mut Vec<ModelEvent>,
    ) -> Result<(), ProviderError> {
        match event {
            StreamEvent::TextDelta { item_id, delta }
            | StreamEvent::RefusalDelta { item_id, delta } => {
                self.accept_text_delta(&item_id, &delta, output)?;
            }
            StreamEvent::OutputItemAdded { item } => self.remember_item(item)?,
            StreamEvent::FunctionArgumentsDone {
                item_id,
                name,
                arguments,
            } => self.emit_call(&item_id, name, &arguments)?,
            StreamEvent::Completed { response } => self.complete(response, output)?,
            StreamEvent::Incomplete { response } => self.incomplete(response, output)?,
            StreamEvent::Failed { response } => {
                self.terminal = true;
                let failure = response.error.unwrap_or_default();
                let code = failure.code.unwrap_or_else(|| "response_failed".into());
                let message = failure
                    .message
                    .unwrap_or_else(|| "provider response failed".into());
                return Err(ProviderError::new(
                    code.replace(&self.secret, "[REDACTED]"),
                    message.replace(&self.secret, "[REDACTED]"),
                    false,
                ));
            }
            StreamEvent::Error { code, message } => {
                self.terminal = true;
                let code = code.unwrap_or_else(|| "provider_error".into());
                let retryable = retryable_code(&code);
                return Err(ProviderError::new(
                    code.replace(&self.secret, "[REDACTED]"),
                    message.replace(&self.secret, "[REDACTED]"),
                    retryable,
                ));
            }
            StreamEvent::Other => {}
        }
        Ok(())
    }

    fn accept_text_delta(
        &mut self,
        item_id: &str,
        delta: &str,
        output: &mut Vec<ModelEvent>,
    ) -> Result<(), ProviderError> {
        let message = self
            .messages
            .get_mut(item_id)
            .ok_or_else(|| protocol_error("text delta referenced an unknown message item"))?;
        message.text.push_str(delta);
        if message.phase != Some(MessagePhase::Commentary) {
            self.assistant_text.push_str(delta);
            let delta = self.text_redactor.push(delta);
            if !delta.is_empty() {
                output.push(ModelEvent::TextDelta { delta });
            }
        }
        Ok(())
    }

    fn remember_item(&mut self, item: OutputItem) -> Result<(), ProviderError> {
        if !self.seen_items.insert(item.id.clone()) {
            return Err(protocol_error("duplicate streamed output item id"));
        }
        match item.kind.as_str() {
            "function_call" => self.remember_call(item),
            "message" => self.remember_message(item),
            _ => Ok(()),
        }
    }

    fn remember_call(&mut self, item: OutputItem) -> Result<(), ProviderError> {
        let call_id = item
            .call_id
            .ok_or_else(|| protocol_error("function call item omitted call_id"))?;
        let name = item
            .name
            .ok_or_else(|| protocol_error("function call item omitted name"))?;
        if self.calls.len() >= MAX_PENDING_CALLS {
            return Err(protocol_error(
                "response exceeded the pending function call limit",
            ));
        }
        self.calls.insert(item.id, PendingCall { call_id, name });
        Ok(())
    }

    fn remember_message(&mut self, item: OutputItem) -> Result<(), ProviderError> {
        if item.role.as_deref() != Some("assistant") {
            return Err(protocol_error(
                "streamed message was not from the assistant",
            ));
        }
        let phase = match item.phase.as_deref() {
            None => None,
            Some("commentary") => Some(MessagePhase::Commentary),
            Some("final_answer") => Some(MessagePhase::FinalAnswer),
            Some(_) => return Err(protocol_error("streamed message phase was not supported")),
        };
        self.messages.insert(
            item.id,
            StreamedMessage {
                phase,
                text: String::new(),
            },
        );
        Ok(())
    }

    fn emit_call(
        &mut self,
        item_id: &str,
        name: String,
        arguments: &str,
    ) -> Result<(), ProviderError> {
        let pending = self
            .calls
            .remove(item_id)
            .ok_or_else(|| protocol_error("function arguments lacked an output item"))?;
        if pending.name != name {
            return Err(protocol_error("function name changed during streaming"));
        }
        let arguments = serde_json::from_str(arguments)
            .map_err(|_| protocol_error("function arguments were not valid JSON"))?;
        let call = ToolCall {
            id: pending.call_id,
            name,
            arguments,
        };
        let completed = CompletedCall {
            item_id: item_id.into(),
            call,
        };
        if self
            .completed_calls
            .insert(completed.call.id.clone(), completed)
            .is_some()
        {
            return Err(protocol_error("duplicate completed function call id"));
        }
        Ok(())
    }

    fn complete(
        &mut self,
        response: CompletedResponse,
        output: &mut Vec<ModelEvent>,
    ) -> Result<(), ProviderError> {
        self.terminal = true;
        if response.status != "completed" || response.incomplete_details.is_some() {
            return Err(protocol_error(
                "completed event contained a contradictory response status",
            ));
        }
        if !self.calls.is_empty() {
            return Err(protocol_error(
                "response completed with unfinished function calls",
            ));
        }
        let final_delta = self.finish_text();
        let context = validated_output_items(&response.output, OutputTerminal::Completed)?;
        self.validate_completed_projection(&context.projection)?;
        let reason = if self.completed_calls.is_empty() {
            ModelFinishReason::Completed
        } else {
            ModelFinishReason::ToolUse
        };
        for call in &context.projection.tool_calls {
            output.push(ModelEvent::ToolCall { call: call.clone() });
        }
        if !final_delta.is_empty() {
            output.push(ModelEvent::TextDelta { delta: final_delta });
        }
        if !context.items.is_empty() {
            output.push(ModelEvent::ProviderContext {
                provider: OPENAI_RESPONSES_CONTEXT.into(),
                items: context.items,
            });
        }
        emit_usage(response.usage, output);
        output.push(ModelEvent::Finished { reason });
        Ok(())
    }

    fn incomplete(
        &mut self,
        response: CompletedResponse,
        output: &mut Vec<ModelEvent>,
    ) -> Result<(), ProviderError> {
        self.terminal = true;
        if response.status != "incomplete" {
            return Err(protocol_error(
                "incomplete event contained a contradictory response status",
            ));
        }
        match incomplete_reason(response.incomplete_details.as_ref())? {
            IncompleteReason::ContentFilter => {
                return Err(ProviderError::new(
                    "content_filter",
                    "provider response was blocked by the content filter",
                    false,
                ));
            }
            IncompleteReason::MaxOutputTokens => {}
        }
        let final_delta = self.finish_text();
        let context = validated_output_items(&response.output, OutputTerminal::Incomplete)?;
        self.validate_incomplete_projection(&context.projection)?;
        if !final_delta.is_empty() {
            output.push(ModelEvent::TextDelta { delta: final_delta });
        }
        emit_usage(response.usage, output);
        output.push(ModelEvent::Finished {
            reason: ModelFinishReason::Length,
        });
        Ok(())
    }

    fn finish_text(&mut self) -> String {
        redact_text(&mut self.assistant_text, &self.secret);
        for message in self.messages.values_mut() {
            redact_text(&mut message.text, &self.secret);
        }
        self.text_redactor.finish()
    }

    fn validate_completed_projection(
        &self,
        projection: &OutputProjection,
    ) -> Result<(), ProviderError> {
        if projection.assistant_text != self.assistant_text
            || !self.messages_match(projection)
            || !projection.incomplete_tool_calls.is_empty()
            || !self.completed_calls_match(projection)
        {
            return Err(protocol_error(
                "terminal output did not match streamed assistant events",
            ));
        }
        Ok(())
    }

    fn validate_incomplete_projection(
        &self,
        projection: &OutputProjection,
    ) -> Result<(), ProviderError> {
        if projection.assistant_text != self.assistant_text
            || !self.messages_match(projection)
            || !self.incomplete_calls_match(projection)
            || !self.completed_calls_match(projection)
        {
            return Err(protocol_error(
                "incomplete output did not match streamed assistant events",
            ));
        }
        if !projection.tool_calls.is_empty() || !projection.incomplete_tool_calls.is_empty() {
            return Err(protocol_error(
                "incomplete response contained a function call",
            ));
        }
        Ok(())
    }

    fn completed_calls_match(&self, projection: &OutputProjection) -> bool {
        projection.tool_calls.len() == self.completed_calls.len()
            && projection.tool_calls.iter().all(|call| {
                self.completed_calls.get(&call.id).is_some_and(|completed| {
                    completed.call == *call
                        && projection
                            .tool_item_ids
                            .get(&call.id)
                            .is_some_and(|item_id| {
                                item_id
                                    .as_deref()
                                    .is_some_and(|item_id| item_id == completed.item_id)
                            })
                })
            })
    }

    fn incomplete_calls_match(&self, projection: &OutputProjection) -> bool {
        projection.incomplete_tool_calls.len() == self.calls.len()
            && projection.incomplete_tool_calls.iter().all(|call| {
                if let Some(item_id) = &call.item_id {
                    return self.calls.get(item_id).is_some_and(|pending| {
                        pending.call_id == call.call_id && pending.name == call.name
                    });
                }
                false
            })
    }

    fn messages_match(&self, projection: &OutputProjection) -> bool {
        projection.messages.len() == self.messages.len()
            && projection.messages.iter().all(|message| {
                self.messages.get(&message.id).is_some_and(|streamed| {
                    streamed.text == message.text && streamed.phase == message.phase
                })
            })
    }
}

fn emit_usage(usage: Option<WireUsage>, output: &mut Vec<ModelEvent>) {
    if let Some(usage) = usage {
        output.push(ModelEvent::Usage {
            usage: Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
            },
        });
    }
}

struct PendingCall {
    call_id: String,
    name: String,
}

struct CompletedCall {
    item_id: String,
    call: ToolCall,
}

struct StreamedMessage {
    phase: Option<MessagePhase>,
    text: String,
}

fn protocol_error(message: &str) -> ProviderError {
    ProviderError::new("provider_protocol", message, false)
}

fn retryable_code(code: &str) -> bool {
    matches!(code, "server_error" | "rate_limit_exceeded")
}
