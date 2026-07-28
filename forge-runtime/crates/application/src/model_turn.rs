use std::collections::BTreeSet;

use forge_runtime_domain::{
    Cancellation, Message, ModelEvent, ModelFinishReason, ModelProvider, ModelRequest,
    RuntimeEventKind, ToolCall, Usage,
};
use futures_util::{FutureExt, StreamExt, future};

use crate::{RuntimeError, emitter::EventEmitter};

pub(crate) struct ModelTurn {
    pub(crate) text: String,
    pub(crate) tool_calls: Vec<ToolCall>,
    pub(crate) provider_context: Vec<Message>,
    pub(crate) finish_reason: ModelFinishReason,
    pub(crate) usage: Usage,
    pub(crate) output_bytes: usize,
    pub(crate) output_events: u32,
}

#[derive(Clone, Copy)]
pub(crate) struct ModelBudget {
    pub(crate) remaining_bytes: usize,
    pub(crate) remaining_events: u32,
}

pub(crate) async fn collect_model_turn(
    provider: &dyn ModelProvider,
    request: ModelRequest,
    cancellation: &Cancellation,
    budget: ModelBudget,
    emitter: &mut EventEmitter<'_>,
) -> Result<ModelTurn, RuntimeError> {
    let mut stream = provider.stream(request);
    let mut collector = TurnCollector::default();
    loop {
        let next = Box::pin(stream.next());
        let cancelled = Box::pin(cancellation.cancelled());
        let event = match future::select(next, cancelled).await {
            future::Either::Left((event, _)) => event,
            future::Either::Right(((), _)) => return Err(RuntimeError::Cancelled),
        };
        let Some(event) = event else {
            break;
        };
        if cancellation.is_cancelled() {
            return Err(RuntimeError::Cancelled);
        }
        match collector.accept(event?, budget, emitter)? {
            CollectAction::Continue => {}
            CollectAction::LocalLimit => return collector.finish(),
            CollectAction::ProviderFinished => {
                if matches!(stream.next().now_or_never(), Some(Some(_))) {
                    return Err(RuntimeError::Protocol(
                        "provider emitted an event after the finished event".into(),
                    ));
                }
                return collector.finish();
            }
        }
    }
    collector.finish()
}

enum CollectAction {
    Continue,
    ProviderFinished,
    LocalLimit,
}

#[derive(Default)]
struct TurnCollector {
    text: String,
    tool_calls: Vec<ToolCall>,
    provider_context: Vec<Message>,
    finish_reason: Option<ModelFinishReason>,
    usage: Usage,
    output_bytes: usize,
    output_events: u32,
}

impl TurnCollector {
    fn accept(
        &mut self,
        event: ModelEvent,
        budget: ModelBudget,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<CollectAction, RuntimeError> {
        if self.finish_reason.is_some() {
            return Err(RuntimeError::Protocol(
                "provider emitted an event after the finished event".into(),
            ));
        }
        match event {
            ModelEvent::TextDelta { delta } => self.accept_delta(delta, budget, emitter),
            ModelEvent::ToolCall { call } => self.accept_tool_call(call, budget),
            ModelEvent::ProviderContext { provider, items } => {
                self.accept_provider_context(provider, items, budget)
            }
            ModelEvent::Usage { usage } => {
                if !self.charge_output(0, budget) {
                    return Ok(self.local_limit());
                }
                self.usage.add(usage);
                Ok(CollectAction::Continue)
            }
            ModelEvent::Finished { reason } => {
                self.record_finish(reason)?;
                Ok(CollectAction::ProviderFinished)
            }
        }
    }

    fn accept_delta(
        &mut self,
        delta: String,
        budget: ModelBudget,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<CollectAction, RuntimeError> {
        if !self.charge_output(delta.len(), budget) {
            return Ok(self.local_limit());
        }
        self.text.push_str(&delta);
        emitter.emit(RuntimeEventKind::AssistantDelta { delta })?;
        Ok(CollectAction::Continue)
    }

    fn accept_tool_call(
        &mut self,
        call: ToolCall,
        budget: ModelBudget,
    ) -> Result<CollectAction, RuntimeError> {
        let bytes = serde_json::to_vec(&call)
            .map_err(|error| RuntimeError::Protocol(format!("tool call cannot encode: {error}")))?
            .len();
        if !self.charge_output(bytes, budget) {
            return Ok(self.local_limit());
        }
        self.tool_calls.push(call);
        Ok(CollectAction::Continue)
    }

    fn accept_provider_context(
        &mut self,
        provider: String,
        items: Vec<serde_json::Value>,
        budget: ModelBudget,
    ) -> Result<CollectAction, RuntimeError> {
        if provider.trim().is_empty() || items.is_empty() {
            return Err(RuntimeError::Protocol(
                "provider context must name its provider and contain at least one item".into(),
            ));
        }
        let message = Message::ProviderContext { provider, items };
        let bytes = serde_json::to_vec(&message)
            .map_err(|error| RuntimeError::Protocol(format!("context cannot encode: {error}")))?
            .len();
        if !self.charge_output(bytes, budget) {
            return Ok(self.local_limit());
        }
        self.provider_context.push(message);
        Ok(CollectAction::Continue)
    }

    fn charge_output(&mut self, bytes: usize, budget: ModelBudget) -> bool {
        if self.output_events >= budget.remaining_events
            || bytes > budget.remaining_bytes.saturating_sub(self.output_bytes)
        {
            return false;
        }
        self.output_events = self.output_events.saturating_add(1);
        self.output_bytes = self.output_bytes.saturating_add(bytes);
        true
    }

    fn local_limit(&mut self) -> CollectAction {
        self.tool_calls.clear();
        self.provider_context.clear();
        self.finish_reason = Some(ModelFinishReason::Length);
        CollectAction::LocalLimit
    }

    fn record_finish(&mut self, reason: ModelFinishReason) -> Result<(), RuntimeError> {
        if self.finish_reason.replace(reason).is_some() {
            return Err(RuntimeError::Protocol(
                "provider emitted more than one finished event".into(),
            ));
        }
        Ok(())
    }

    fn finish(self) -> Result<ModelTurn, RuntimeError> {
        let finish_reason = self.finish_reason.ok_or_else(|| {
            RuntimeError::Protocol("provider stream ended without a finished event".into())
        })?;
        validate_tool_calls(&self.tool_calls)?;
        Ok(ModelTurn {
            text: self.text,
            tool_calls: self.tool_calls,
            provider_context: self.provider_context,
            finish_reason,
            usage: self.usage,
            output_bytes: self.output_bytes,
            output_events: self.output_events,
        })
    }
}

fn validate_tool_calls(calls: &[ToolCall]) -> Result<(), RuntimeError> {
    let mut ids = BTreeSet::new();
    for call in calls {
        if call.id.trim().is_empty() || call.name.trim().is_empty() {
            return Err(RuntimeError::Protocol(
                "provider emitted a tool call with an empty id or name".into(),
            ));
        }
        if !ids.insert(call.id.as_str()) {
            return Err(RuntimeError::Protocol(format!(
                "provider emitted duplicate tool call id '{}'",
                call.id
            )));
        }
    }
    Ok(())
}
