use crate::runtime_domain::{HubStoreError, ProviderError};
use thiserror::Error;

use super::GroupAgentNodeExecutionContractServiceError;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentNodeDispatchRequestServiceError {
    #[error("Node Dispatch Request input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("Node Dispatch Request operation conflicts: {message}")]
    Conflict { message: String },
    #[error("Node Dispatch Request source was not found: {message}")]
    NotFound { message: String },
    #[error("Node Dispatch Request storage is unavailable: {message}")]
    Unavailable { message: String },
    #[error("Node Dispatch Request durable state is corrupt: {message}")]
    Corrupt { message: String },
}

impl From<HubStoreError> for GroupAgentNodeDispatchRequestServiceError {
    fn from(error: HubStoreError) -> Self {
        from_parts(&error, error.to_string())
    }
}

impl From<GroupAgentNodeExecutionContractServiceError>
    for GroupAgentNodeDispatchRequestServiceError
{
    fn from(error: GroupAgentNodeExecutionContractServiceError) -> Self {
        let message = error.to_string();
        match error {
            GroupAgentNodeExecutionContractServiceError::InvalidInput { .. } => {
                Self::InvalidInput { message }
            }
            GroupAgentNodeExecutionContractServiceError::Conflict { .. } => {
                Self::Conflict { message }
            }
            GroupAgentNodeExecutionContractServiceError::NotFound { .. } => {
                Self::NotFound { message }
            }
            GroupAgentNodeExecutionContractServiceError::Unavailable { .. } => {
                Self::Unavailable { message }
            }
            GroupAgentNodeExecutionContractServiceError::Corrupt { .. } => {
                Self::Corrupt { message }
            }
        }
    }
}

fn from_parts(error: &HubStoreError, message: String) -> GroupAgentNodeDispatchRequestServiceError {
    match error {
        HubStoreError::NotFound { .. } => {
            GroupAgentNodeDispatchRequestServiceError::NotFound { message }
        }
        HubStoreError::Conflict { .. } => {
            GroupAgentNodeDispatchRequestServiceError::Conflict { message }
        }
        HubStoreError::Unavailable { .. } => {
            GroupAgentNodeDispatchRequestServiceError::Unavailable { message }
        }
        HubStoreError::Corrupt { .. } => {
            GroupAgentNodeDispatchRequestServiceError::Corrupt { message }
        }
    }
}

pub(super) fn invalid(message: &str) -> GroupAgentNodeDispatchRequestServiceError {
    GroupAgentNodeDispatchRequestServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn conflict(message: &str) -> GroupAgentNodeDispatchRequestServiceError {
    GroupAgentNodeDispatchRequestServiceError::Conflict {
        message: message.into(),
    }
}

pub(super) fn corrupt(message: &str) -> GroupAgentNodeDispatchRequestServiceError {
    GroupAgentNodeDispatchRequestServiceError::Corrupt {
        message: message.into(),
    }
}

pub(super) fn codec_error(error: &ProviderError) -> GroupAgentNodeDispatchRequestServiceError {
    invalid(&format!(
        "provider request codec rejected the contract: {error}"
    ))
}
