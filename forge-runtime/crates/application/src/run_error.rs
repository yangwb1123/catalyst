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
