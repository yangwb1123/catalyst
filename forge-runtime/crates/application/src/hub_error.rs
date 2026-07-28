use thiserror::Error;

use crate::runtime_domain::HubStoreError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HubField {
    ConversationId,
    GroupId,
    GroupRunId,
    ProjectId,
    Title,
    GroupName,
    Role,
    Prompt,
    IdempotencyKey,
    PromptLimit,
    GroupContextMembers,
    GroupContextGroupConversations,
    GroupContextProjectConversations,
    GroupContextPrompts,
    GroupContextPromptExcerptBytes,
    GroupContextBytes,
    GroupRunLimit,
}

#[derive(Debug, Error)]
pub enum HubError {
    #[error("{field:?} must not be empty")]
    Empty { field: HubField },
    #[error("{field:?} exceeds the {max_bytes}-byte limit")]
    TooLong { field: HubField, max_bytes: usize },
    #[error("{field:?} contains unsupported control characters")]
    InvalidCharacters { field: HubField },
    #[error("{field:?} must be between {min} and {max}")]
    OutOfRange {
        field: HubField,
        min: usize,
        max: usize,
    },
    #[error("unsupported Group Run request version {actual}; expected {expected}")]
    UnsupportedGroupRunVersion { actual: u16, expected: u16 },
    #[error("Group Run creation time exceeds SQLite's signed 64-bit range")]
    GroupRunCreationTimeOutOfRange,
    #[error("project path must be normalized and absolute")]
    InvalidProjectPath,
    #[error("hub store failed: {0}")]
    Store(#[from] HubStoreError),
}
