use std::collections::BTreeSet;

use forge_runtime_domain::{
    Cancellation, ModelEvent, ModelFinishReason, ModelProvider, ModelRequest, RuntimeEventKind,
    ToolCall, Usage,
};
use futures_util::{FutureExt, StreamExt, future};

use crate::{RuntimeError, emitter::EventEmitter};

pub(crate) struct ModelTurn {
    pub(crate) text: String,
    pub(crate) tool_calls: Vec<ToolCall>,
    pub(crate) finish_reason: ModelFinishReason,
    pub(crate) usage: Usage,
}

pub(crate) async fn collect_model_turn(
    provider: &dyn ModelProvider,
    request: ModelRequest,
    cancellation: &Cancellation,
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
        if collector.accept(event?, emitter)? {
            if matches!(stream.next().now_or_never(), Some(Some(_))) {
                return Err(RuntimeError::Protocol(
                    "provider emitted an event after the finished event".into(),
                ));
            }
            return collector.finish();
        }
    }
    collector.finish()
}

#[derive(Default)]
struct TurnCollector {
    text: String,
    tool_calls: Vec<ToolCall>,
    finish_reason: Option<ModelFinishReason>,
    usage: Usage,
}

impl TurnCollector {
    fn accept(
        &mut self,
        event: ModelEvent,
        emitter: &mut EventEmitter<'_>,
    ) -> Result<bool, RuntimeError> {
        if self.finish_reason.is_some() {
            return Err(RuntimeError::Protocol(
                "provider emitted an event after the finished event".into(),
            ));
        }
        match event {
            ModelEvent::TextDelta { delta } => {
                self.text.push_str(&delta);
                emitter.emit(RuntimeEventKind::AssistantDelta { delta })?;
            }
            ModelEvent::ToolCall { call } => self.tool_calls.push(call),
            ModelEvent::Usage { usage } => self.usage.add(usage),
            ModelEvent::Finished { reason } => self.record_finish(reason)?,
        }
        Ok(self.finish_reason.is_some())
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
            finish_reason,
            usage: self.usage,
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
