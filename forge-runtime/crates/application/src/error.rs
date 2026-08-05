use forge_runtime_domain::{EventSinkError, ProviderError};

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("provider failed: {0}")]
    Provider(#[from] ProviderError),
    #[error("model protocol failed: {0}")]
    Protocol(String),
    #[error("event sink failed: {0}")]
    EventSink(#[from] EventSinkError),
    #[error("tool catalog failed: {0}")]
    ToolCatalog(String),
    #[error("workspace unavailable: {0}")]
    Workspace(String),
    #[error("run was cancelled")]
    Cancelled,
}

impl RuntimeError {
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Provider(_) => "provider_error",
            Self::Protocol(_) => "model_protocol_error",
            Self::EventSink(_) => "event_sink_error",
            Self::ToolCatalog(_) => "tool_catalog_error",
            Self::Workspace(_) => "workspace_unavailable",
            Self::Cancelled => "cancelled",
        }
    }
}
use thiserror::Error;

use crate::runtime_domain::RunStoreError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunField {
    RunId,
    ConversationId,
    PromptId,
    ProjectId,
    Answer,
    IdempotencyKey,
    RunLimit,
}

#[derive(Debug, Error)]
pub enum RunError {
    #[error("{field:?} must not be empty")]
    Empty { field: RunField },
    #[error("{field:?} exceeds the {max_bytes}-byte limit")]
    TooLong { field: RunField, max_bytes: usize },
    #[error("RunLimit must be between {min} and {max}")]
    OutOfRange {
        field: RunField,
        min: usize,
        max: usize,
    },
    #[error("Run must be terminal-completed before assistant reconciliation")]
    NotCompleted,
    #[error("run store failed: {0}")]
    Store(#[from] RunStoreError),
}
