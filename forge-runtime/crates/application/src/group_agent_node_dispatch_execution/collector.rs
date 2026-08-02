use std::time::Duration;

use crate::runtime_domain::{
    Cancellation, GroupAgentNodeTerminalClassification, ModelEvent, ModelEventStream,
    ModelFinishReason, PreparedModelProvider, PreparedModelRequest, ProviderError, Usage,
};
use futures_util::{StreamExt, future};

#[cfg(test)]
#[path = "collector_tests.rs"]
mod tests;

#[derive(Clone, Copy)]
pub(crate) struct DispatchCollectionLimits {
    pub(crate) model_output_bytes: usize,
    pub(crate) result_bytes: usize,
    pub(crate) output_tokens: u32,
    pub(crate) events: u32,
    pub(crate) timeout_ms: u64,
}

pub(crate) struct CollectedDispatchEvidence {
    pub(crate) classification: GroupAgentNodeTerminalClassification,
    pub(crate) output: String,
    pub(crate) usage: Option<Usage>,
    pub(crate) provider_poll_started: bool,
    pub(crate) terminal_seen: bool,
    pub(crate) stream_eof_seen: bool,
}

pub(crate) async fn collect_dispatch(
    provider: &dyn PreparedModelProvider,
    body: Vec<u8>,
    cancellation: &Cancellation,
    limits: DispatchCollectionLimits,
) -> CollectedDispatchEvidence {
    let request = PreparedModelRequest::new(body, cancellation.clone());
    let mut stream = provider.stream_prepared(request);
    let mut collector = DispatchCollector::new(limits);
    collector.provider_poll_started = true;
    let drive = drive_stream(&mut stream, cancellation, &mut collector);
    let classification =
        match tokio::time::timeout(Duration::from_millis(limits.timeout_ms), drive).await {
            Ok(classification) => classification,
            Err(_) => GroupAgentNodeTerminalClassification::Timeout,
        };
    collector.evidence(classification)
}

async fn drive_stream(
    stream: &mut ModelEventStream,
    cancellation: &Cancellation,
    collector: &mut DispatchCollector,
) -> GroupAgentNodeTerminalClassification {
    loop {
        let next = Box::pin(stream.next());
        let cancelled = Box::pin(cancellation.cancelled());
        let event = match future::select(next, cancelled).await {
            future::Either::Left((event, _)) => event,
            future::Either::Right(((), _)) => {
                return GroupAgentNodeTerminalClassification::Cancelled;
            }
        };
        let Some(event) = event else {
            collector.stream_eof_seen = true;
            return GroupAgentNodeTerminalClassification::EofBeforeTerminal;
        };
        match collector.accept(event) {
            Ok(None) => {}
            Ok(Some(classification)) => {
                return require_eof(stream, cancellation, collector, classification).await;
            }
            Err(classification) => return classification,
        }
    }
}

async fn require_eof(
    stream: &mut ModelEventStream,
    cancellation: &Cancellation,
    collector: &mut DispatchCollector,
    classification: GroupAgentNodeTerminalClassification,
) -> GroupAgentNodeTerminalClassification {
    let next = Box::pin(stream.next());
    let cancelled = Box::pin(cancellation.cancelled());
    match future::select(next, cancelled).await {
        future::Either::Left((None, _)) => {
            collector.stream_eof_seen = true;
            collector.classify_terminal(classification)
        }
        future::Either::Left((Some(_), _)) => GroupAgentNodeTerminalClassification::TrailingData,
        future::Either::Right(((), _)) => GroupAgentNodeTerminalClassification::Cancelled,
    }
}

#[allow(clippy::struct_excessive_bools)]
struct DispatchCollector {
    limits: DispatchCollectionLimits,
    output: String,
    usage: Usage,
    usage_seen: bool,
    model_output_bytes: usize,
    result_bytes: usize,
    events: u32,
    provider_poll_started: bool,
    terminal_seen: bool,
    stream_eof_seen: bool,
}

impl DispatchCollector {
    fn new(limits: DispatchCollectionLimits) -> Self {
        Self {
            limits,
            output: String::new(),
            usage: Usage::default(),
            usage_seen: false,
            model_output_bytes: 0,
            result_bytes: 0,
            events: 0,
            provider_poll_started: false,
            terminal_seen: false,
            stream_eof_seen: false,
        }
    }

    fn accept(
        &mut self,
        event: Result<ModelEvent, ProviderError>,
    ) -> Result<Option<GroupAgentNodeTerminalClassification>, GroupAgentNodeTerminalClassification>
    {
        let event = event.map_err(|error| classify_provider_error(&error))?;
        self.charge_event()?;
        match event {
            ModelEvent::TextDelta { delta } => {
                self.charge_model_bytes(delta.len())?;
                self.charge_result_bytes(delta.len())?;
                self.output.push_str(&delta);
                Ok(None)
            }
            ModelEvent::ToolCall { .. } => Err(GroupAgentNodeTerminalClassification::ToolCall),
            ModelEvent::ProviderContext { provider, items } => {
                self.accept_context(&provider, &items)?;
                Ok(None)
            }
            ModelEvent::Usage { usage } => {
                self.accept_usage(usage)?;
                Ok(None)
            }
            ModelEvent::Finished { reason } => self.accept_terminal(reason),
        }
    }

    fn accept_context(
        &mut self,
        provider: &str,
        items: &[serde_json::Value],
    ) -> Result<(), GroupAgentNodeTerminalClassification> {
        if provider.trim().is_empty() || items.is_empty() {
            return Err(GroupAgentNodeTerminalClassification::ProtocolError);
        }
        let item_bytes = serde_json::to_vec(items)
            .map_err(|_| GroupAgentNodeTerminalClassification::ProtocolError)?
            .len();
        let bytes = provider
            .len()
            .checked_add(item_bytes)
            .ok_or(GroupAgentNodeTerminalClassification::LocalLimit)?;
        self.charge_model_bytes(bytes)
    }

    fn accept_usage(&mut self, usage: Usage) -> Result<(), GroupAgentNodeTerminalClassification> {
        self.usage.input_tokens = self
            .usage
            .input_tokens
            .checked_add(usage.input_tokens)
            .ok_or(GroupAgentNodeTerminalClassification::LocalLimit)?;
        self.usage.output_tokens = self
            .usage
            .output_tokens
            .checked_add(usage.output_tokens)
            .ok_or(GroupAgentNodeTerminalClassification::LocalLimit)?;
        self.usage_seen = true;
        if self.usage.output_tokens > u64::from(self.limits.output_tokens) {
            return Err(GroupAgentNodeTerminalClassification::LocalLimit);
        }
        Ok(())
    }

    fn accept_terminal(
        &mut self,
        reason: ModelFinishReason,
    ) -> Result<Option<GroupAgentNodeTerminalClassification>, GroupAgentNodeTerminalClassification>
    {
        self.terminal_seen = true;
        match reason {
            ModelFinishReason::Completed => {
                Ok(Some(GroupAgentNodeTerminalClassification::Completed))
            }
            ModelFinishReason::Length => Ok(Some(GroupAgentNodeTerminalClassification::Length)),
            ModelFinishReason::ToolUse => Err(GroupAgentNodeTerminalClassification::ToolCall),
        }
    }

    fn classify_terminal(
        &self,
        classification: GroupAgentNodeTerminalClassification,
    ) -> GroupAgentNodeTerminalClassification {
        if !self.usage_seen {
            GroupAgentNodeTerminalClassification::MissingUsage
        } else if classification == GroupAgentNodeTerminalClassification::Completed
            && self.output.is_empty()
        {
            GroupAgentNodeTerminalClassification::ProtocolError
        } else {
            classification
        }
    }

    fn charge_event(&mut self) -> Result<(), GroupAgentNodeTerminalClassification> {
        self.events = self
            .events
            .checked_add(1)
            .ok_or(GroupAgentNodeTerminalClassification::LocalLimit)?;
        if self.events > self.limits.events {
            return Err(GroupAgentNodeTerminalClassification::LocalLimit);
        }
        Ok(())
    }

    fn charge_model_bytes(
        &mut self,
        bytes: usize,
    ) -> Result<(), GroupAgentNodeTerminalClassification> {
        self.model_output_bytes = self
            .model_output_bytes
            .checked_add(bytes)
            .ok_or(GroupAgentNodeTerminalClassification::LocalLimit)?;
        if self.model_output_bytes > self.limits.model_output_bytes {
            return Err(GroupAgentNodeTerminalClassification::LocalLimit);
        }
        Ok(())
    }

    fn charge_result_bytes(
        &mut self,
        bytes: usize,
    ) -> Result<(), GroupAgentNodeTerminalClassification> {
        self.result_bytes = self
            .result_bytes
            .checked_add(bytes)
            .ok_or(GroupAgentNodeTerminalClassification::LocalLimit)?;
        if self.result_bytes > self.limits.result_bytes {
            return Err(GroupAgentNodeTerminalClassification::LocalLimit);
        }
        Ok(())
    }

    fn evidence(
        self,
        classification: GroupAgentNodeTerminalClassification,
    ) -> CollectedDispatchEvidence {
        CollectedDispatchEvidence {
            classification,
            output: self.output,
            usage: self.usage_seen.then_some(self.usage),
            provider_poll_started: self.provider_poll_started,
            terminal_seen: self.terminal_seen,
            stream_eof_seen: self.stream_eof_seen,
        }
    }
}

fn classify_provider_error(error: &ProviderError) -> GroupAgentNodeTerminalClassification {
    match error.code.as_str() {
        "cancelled" => GroupAgentNodeTerminalClassification::Cancelled,
        "transport_error" => GroupAgentNodeTerminalClassification::TransportError,
        "stream_ended" => GroupAgentNodeTerminalClassification::EofBeforeTerminal,
        "provider_protocol" => GroupAgentNodeTerminalClassification::ProtocolError,
        code if code.starts_with("http_") => GroupAgentNodeTerminalClassification::HttpError,
        _ => GroupAgentNodeTerminalClassification::ProviderError,
    }
}
