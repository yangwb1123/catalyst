use forge_runtime_domain::{
    Cancellation, ModelEvent, ModelFinishReason, PreparedModelProvider, PreparedModelRequest, Usage,
};
use futures_util::{StreamExt, future};

use crate::group_model_analysis_error::{AnalysisLimit, PostClaimError};

#[derive(Clone, Copy)]
pub(crate) struct AnalysisModelLimits {
    pub(crate) output_bytes: usize,
    pub(crate) events: u32,
    pub(crate) output_tokens: u32,
}

pub(crate) struct AnalysisModelTurn {
    pub(crate) answer: String,
    pub(crate) finish_reason: ModelFinishReason,
    pub(crate) usage: Usage,
}

pub(crate) async fn collect_prepared_turn(
    provider: &dyn PreparedModelProvider,
    body: Vec<u8>,
    cancellation: &Cancellation,
    limits: AnalysisModelLimits,
) -> Result<AnalysisModelTurn, PostClaimError> {
    let request = PreparedModelRequest::new(body, cancellation.clone());
    let mut stream = provider.stream_prepared(request);
    let mut collector = AnalysisCollector::default();
    loop {
        let next = Box::pin(stream.next());
        let cancelled = Box::pin(cancellation.cancelled());
        let event = match future::select(next, cancelled).await {
            future::Either::Left((event, _)) => event,
            future::Either::Right(((), _)) => return Err(PostClaimError::Cancelled),
        };
        let Some(event) = event else {
            return collector.finish();
        };
        if cancellation.is_cancelled() {
            return Err(PostClaimError::Cancelled);
        }
        if collector.accept(event?, limits)? {
            require_eof(&mut stream, cancellation).await?;
            return collector.finish();
        }
    }
}

async fn require_eof(
    stream: &mut forge_runtime_domain::ModelEventStream,
    cancellation: &Cancellation,
) -> Result<(), PostClaimError> {
    let next = Box::pin(stream.next());
    let cancelled = Box::pin(cancellation.cancelled());
    match future::select(next, cancelled).await {
        future::Either::Left((None, _)) => Ok(()),
        future::Either::Left((Some(_), _)) => Err(PostClaimError::Protocol),
        future::Either::Right(((), _)) => Err(PostClaimError::Cancelled),
    }
}

#[derive(Default)]
struct AnalysisCollector {
    answer: String,
    finish_reason: Option<ModelFinishReason>,
    usage: Usage,
    output_bytes: usize,
    events: u32,
}

impl AnalysisCollector {
    fn accept(
        &mut self,
        event: ModelEvent,
        limits: AnalysisModelLimits,
    ) -> Result<bool, PostClaimError> {
        match event {
            ModelEvent::TextDelta { delta } => {
                self.charge(delta.len(), limits)?;
                self.answer.push_str(&delta);
                Ok(false)
            }
            ModelEvent::ToolCall { .. } => {
                self.charge(0, limits)?;
                Err(PostClaimError::ToolCall)
            }
            ModelEvent::ProviderContext { provider, items } => {
                self.accept_context(&provider, &items, limits)?;
                Ok(false)
            }
            ModelEvent::Usage { usage } => {
                self.accept_usage(usage, limits)?;
                Ok(false)
            }
            ModelEvent::Finished { reason } => {
                self.charge(0, limits)?;
                self.finish_reason = Some(reason);
                Ok(true)
            }
        }
    }

    fn accept_context(
        &mut self,
        provider: &str,
        items: &[serde_json::Value],
        limits: AnalysisModelLimits,
    ) -> Result<(), PostClaimError> {
        if provider.trim().is_empty() || items.is_empty() {
            return Err(PostClaimError::Protocol);
        }
        let bytes = provider
            .len()
            .checked_add(
                serde_json::to_vec(&items)
                    .map_err(|_| PostClaimError::Protocol)?
                    .len(),
            )
            .ok_or(PostClaimError::Limit(AnalysisLimit::OutputBytes))?;
        self.charge(bytes, limits)
    }

    fn accept_usage(
        &mut self,
        next: Usage,
        limits: AnalysisModelLimits,
    ) -> Result<(), PostClaimError> {
        self.charge(0, limits)?;
        self.usage = checked_usage(self.usage, next)?;
        if self.usage.output_tokens > u64::from(limits.output_tokens) {
            return Err(PostClaimError::Limit(AnalysisLimit::OutputTokens));
        }
        Ok(())
    }

    fn charge(&mut self, bytes: usize, limits: AnalysisModelLimits) -> Result<(), PostClaimError> {
        if self.events >= limits.events {
            return Err(PostClaimError::Limit(AnalysisLimit::ModelEvents));
        }
        if bytes > limits.output_bytes.saturating_sub(self.output_bytes) {
            return Err(PostClaimError::Limit(AnalysisLimit::OutputBytes));
        }
        self.events = self.events.saturating_add(1);
        self.output_bytes = self.output_bytes.saturating_add(bytes);
        Ok(())
    }

    fn finish(self) -> Result<AnalysisModelTurn, PostClaimError> {
        let finish_reason = self.finish_reason.ok_or(PostClaimError::Protocol)?;
        if matches!(finish_reason, ModelFinishReason::ToolUse) {
            return Err(PostClaimError::ToolCall);
        }
        if self.answer.trim().is_empty() {
            return Err(PostClaimError::Protocol);
        }
        Ok(AnalysisModelTurn {
            answer: self.answer,
            finish_reason,
            usage: self.usage,
        })
    }
}

fn checked_usage(total: Usage, next: Usage) -> Result<Usage, PostClaimError> {
    let input_tokens = total
        .input_tokens
        .checked_add(next.input_tokens)
        .filter(|value| i64::try_from(*value).is_ok())
        .ok_or(PostClaimError::Limit(AnalysisLimit::Usage))?;
    let output_tokens = total
        .output_tokens
        .checked_add(next.output_tokens)
        .filter(|value| i64::try_from(*value).is_ok())
        .ok_or(PostClaimError::Limit(AnalysisLimit::Usage))?;
    Ok(Usage {
        input_tokens,
        output_tokens,
    })
}
