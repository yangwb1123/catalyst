use forge_runtime_domain::{EventSinkError, ProviderError};
use thiserror::Error;

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
