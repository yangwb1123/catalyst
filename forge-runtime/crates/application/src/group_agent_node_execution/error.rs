use crate::runtime_domain::HubStoreError;
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentNodeExecutionContractServiceError {
    #[error("Node Execution Contract input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("Node Execution Contract operation conflicts: {message}")]
    Conflict { message: String },
    #[error("Node Execution Contract source was not found: {message}")]
    NotFound { message: String },
    #[error("Node Execution Contract storage is unavailable: {message}")]
    Unavailable { message: String },
    #[error("Node Execution Contract durable state is corrupt: {message}")]
    Corrupt { message: String },
}

impl From<HubStoreError> for GroupAgentNodeExecutionContractServiceError {
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

pub(super) fn invalid(message: &str) -> GroupAgentNodeExecutionContractServiceError {
    GroupAgentNodeExecutionContractServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn conflict(message: &str) -> GroupAgentNodeExecutionContractServiceError {
    GroupAgentNodeExecutionContractServiceError::Conflict {
        message: message.into(),
    }
}

pub(super) fn corrupt(message: &str) -> GroupAgentNodeExecutionContractServiceError {
    GroupAgentNodeExecutionContractServiceError::Corrupt {
        message: message.into(),
    }
}
