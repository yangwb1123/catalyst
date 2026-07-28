use std::pin::Pin;

use futures_core::Stream;
use serde::{Deserialize, Serialize};

use crate::{Cancellation, Message, ToolCall, ToolSpec};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFinishReason {
    Completed,
    ToolUse,
    Length,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Usage {
    pub fn add(&mut self, other: Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelEvent {
    TextDelta {
        delta: String,
    },
    ToolCall {
        call: ToolCall,
    },
    ProviderContext {
        provider: String,
        items: Vec<serde_json::Value>,
    },
    Usage {
        usage: Usage,
    },
    Finished {
        reason: ModelFinishReason,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProviderError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ProviderError {}

#[derive(Clone, Debug)]
pub struct ModelRequest {
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
    pub max_output_tokens: u32,
    pub cancellation: Cancellation,
}

pub type ModelEventStream =
    Pin<Box<dyn Stream<Item = Result<ModelEvent, ProviderError>> + Send + 'static>>;

pub trait ModelProvider: Send + Sync {
    fn stream(&self, request: ModelRequest) -> ModelEventStream;
}
