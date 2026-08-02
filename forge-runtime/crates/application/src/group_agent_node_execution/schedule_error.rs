use crate::runtime_domain::HubStoreError;
use thiserror::Error;

use super::GroupAgentNodeExecutionContractServiceError;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentGraphExecutionScheduleServiceError {
    #[error("Graph Execution Schedule input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("Graph Execution Schedule operation conflicts: {message}")]
    Conflict { message: String },
    #[error("Graph Execution Schedule source was not found: {message}")]
    NotFound { message: String },
    #[error("Graph Execution Schedule storage is unavailable: {message}")]
    Unavailable { message: String },
    #[error("Graph Execution Schedule durable state is corrupt: {message}")]
    Corrupt { message: String },
}

impl From<HubStoreError> for GroupAgentGraphExecutionScheduleServiceError {
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

impl From<GroupAgentNodeExecutionContractServiceError>
    for GroupAgentGraphExecutionScheduleServiceError
{
    fn from(error: GroupAgentNodeExecutionContractServiceError) -> Self {
        match error {
            GroupAgentNodeExecutionContractServiceError::InvalidInput { message } => {
                Self::InvalidInput { message }
            }
            GroupAgentNodeExecutionContractServiceError::Conflict { message } => {
                Self::Conflict { message }
            }
            GroupAgentNodeExecutionContractServiceError::NotFound { message } => {
                Self::NotFound { message }
            }
            GroupAgentNodeExecutionContractServiceError::Unavailable { message } => {
                Self::Unavailable { message }
            }
            GroupAgentNodeExecutionContractServiceError::Corrupt { message } => {
                Self::Corrupt { message }
            }
        }
    }
}

pub(super) fn invalid(message: &str) -> GroupAgentGraphExecutionScheduleServiceError {
    GroupAgentGraphExecutionScheduleServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn corrupt(message: &str) -> GroupAgentGraphExecutionScheduleServiceError {
    GroupAgentGraphExecutionScheduleServiceError::Corrupt {
        message: message.into(),
    }
}
