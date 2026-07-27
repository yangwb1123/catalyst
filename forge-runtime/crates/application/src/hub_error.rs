use forge_runtime_domain::HubStoreError;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HubField {
    ConversationId,
    GroupId,
    ProjectId,
    Title,
    GroupName,
    Role,
    Prompt,
    IdempotencyKey,
    PromptLimit,
}

#[derive(Debug, Error)]
pub enum HubError {
    #[error("{field:?} must not be empty")]
    Empty { field: HubField },
    #[error("{field:?} exceeds the {max_bytes}-byte limit")]
    TooLong { field: HubField, max_bytes: usize },
    #[error("PromptLimit must be between {min} and {max}")]
    OutOfRange {
        field: HubField,
        min: usize,
        max: usize,
    },
    #[error("project path must be normalized and absolute")]
    InvalidProjectPath,
    #[error("hub store failed: {0}")]
    Store(#[from] HubStoreError),
}
