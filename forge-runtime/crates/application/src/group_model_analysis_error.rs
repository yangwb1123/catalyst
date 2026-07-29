use forge_runtime_domain::{HubStoreError, ProviderError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GroupModelAnalysisServiceError {
    #[error("Group Model Analysis input is invalid")]
    InvalidInput,
    #[error("frozen Group Run failed Group Model Analysis validation")]
    InvalidSource,
    #[error("Group Model Analysis request encoding failed")]
    RequestEncoding,
    #[error("Group Model Analysis store returned inconsistent state")]
    InconsistentStoreResult,
    #[error("Group Model Analysis store failed: {0}")]
    Store(#[from] HubStoreError),
    #[error("Group Model Analysis dispatch outcome is unknown; automatic retry is disabled")]
    DispatchUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AnalysisLimit {
    OutputBytes,
    ModelEvents,
    OutputTokens,
    Usage,
    ResultBytes,
}

#[derive(Debug, Error)]
pub(crate) enum PostClaimError {
    #[error("provider failed after dispatch")]
    Provider,
    #[error("model protocol failed after dispatch")]
    Protocol,
    #[error("model emitted a tool call for a zero-tool analysis")]
    ToolCall,
    #[error("model exceeded its {0:?} limit")]
    Limit(AnalysisLimit),
    #[error("analysis was cancelled")]
    Cancelled,
    #[error("completion store failed after dispatch")]
    Store,
    #[error("completion store returned inconsistent state")]
    InconsistentStoreResult,
}

impl From<ProviderError> for PostClaimError {
    fn from(_: ProviderError) -> Self {
        Self::Provider
    }
}
