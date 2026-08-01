use crate::runtime_domain::HubStoreError;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentNodeDispatchReleaseControlServiceError {
    #[error("Node Dispatch Release Control input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("Node Dispatch Release Control source was not found: {message}")]
    NotFound { message: String },
    #[error("Node Dispatch Release Control storage is unavailable: {message}")]
    Unavailable { message: String },
    #[error("Node Dispatch Release Control durable state is corrupt: {message}")]
    Corrupt { message: String },
}

impl From<HubStoreError> for GroupAgentNodeDispatchReleaseControlServiceError {
    fn from(error: HubStoreError) -> Self {
        let message = error.to_string();
        match error {
            HubStoreError::NotFound { .. } => Self::NotFound { message },
            HubStoreError::Unavailable { .. } => Self::Unavailable { message },
            HubStoreError::Conflict { .. } | HubStoreError::Corrupt { .. } => {
                Self::Corrupt { message }
            }
        }
    }
}

pub(super) fn invalid(message: &str) -> GroupAgentNodeDispatchReleaseControlServiceError {
    GroupAgentNodeDispatchReleaseControlServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn not_found(message: &str) -> GroupAgentNodeDispatchReleaseControlServiceError {
    GroupAgentNodeDispatchReleaseControlServiceError::NotFound {
        message: message.into(),
    }
}

pub(super) fn corrupt(message: &str) -> GroupAgentNodeDispatchReleaseControlServiceError {
    GroupAgentNodeDispatchReleaseControlServiceError::Corrupt {
        message: message.into(),
    }
}
