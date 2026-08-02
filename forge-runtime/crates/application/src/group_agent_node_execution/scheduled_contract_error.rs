use crate::runtime_domain::HubStoreError;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentScheduledNodeContractServiceError {
    #[error("Scheduled Node Contract input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("Scheduled Node Contract operation conflicts: {message}")]
    Conflict { message: String },
    #[error("Scheduled Node Contract source was not found: {message}")]
    NotFound { message: String },
    #[error("Scheduled Node Contract storage is unavailable: {message}")]
    Unavailable { message: String },
    #[error("Scheduled Node Contract durable state is corrupt: {message}")]
    Corrupt { message: String },
}

impl From<HubStoreError> for GroupAgentScheduledNodeContractServiceError {
    fn from(error: HubStoreError) -> Self {
        let message = error.to_string();
        match error {
            HubStoreError::NotFound { .. } => Self::NotFound { message },
            HubStoreError::Conflict { .. } => Self::Conflict { message },
            HubStoreError::Unavailable { .. } => Self::Unavailable { message },
            HubStoreError::Corrupt { .. } => Self::Corrupt { message },
        }
    }
}

impl From<super::GroupAgentNodeExecutionContractServiceError>
    for GroupAgentScheduledNodeContractServiceError
{
    fn from(error: super::GroupAgentNodeExecutionContractServiceError) -> Self {
        use super::GroupAgentNodeExecutionContractServiceError as Source;
        match error {
            Source::InvalidInput { message } => Self::InvalidInput { message },
            Source::Conflict { message } => Self::Conflict { message },
            Source::NotFound { message } => Self::NotFound { message },
            Source::Unavailable { message } => Self::Unavailable { message },
            Source::Corrupt { message } => Self::Corrupt { message },
        }
    }
}

pub(super) fn invalid(message: &str) -> GroupAgentScheduledNodeContractServiceError {
    GroupAgentScheduledNodeContractServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn corrupt(message: &str) -> GroupAgentScheduledNodeContractServiceError {
    GroupAgentScheduledNodeContractServiceError::Corrupt {
        message: message.into(),
    }
}
