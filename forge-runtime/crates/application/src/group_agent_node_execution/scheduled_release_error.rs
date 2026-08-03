use crate::runtime_domain::HubStoreError;
use thiserror::Error;

use super::{
    GroupAgentNodeExecutionContractServiceError, GroupAgentScheduledNodeContractServiceError,
    GroupAgentScheduledNodeProviderRequestServiceError,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentScheduledNodeDispatchReleaseControlServiceError {
    #[error("Scheduled Node Dispatch Release Control input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("Scheduled Node Dispatch Release Control operation conflicts: {message}")]
    Conflict { message: String },
    #[error("Scheduled Node Dispatch Release Control source was not found: {message}")]
    NotFound { message: String },
    #[error("Scheduled Node Dispatch Release Control storage is unavailable: {message}")]
    Unavailable { message: String },
    #[error("Scheduled Node Dispatch Release Control durable state is corrupt: {message}")]
    Corrupt { message: String },
}

impl From<HubStoreError> for GroupAgentScheduledNodeDispatchReleaseControlServiceError {
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

impl From<GroupAgentScheduledNodeProviderRequestServiceError>
    for GroupAgentScheduledNodeDispatchReleaseControlServiceError
{
    fn from(error: GroupAgentScheduledNodeProviderRequestServiceError) -> Self {
        match error {
            GroupAgentScheduledNodeProviderRequestServiceError::InvalidInput { message } => {
                Self::InvalidInput { message }
            }
            GroupAgentScheduledNodeProviderRequestServiceError::Conflict { message } => {
                Self::Conflict { message }
            }
            GroupAgentScheduledNodeProviderRequestServiceError::NotFound { message } => {
                Self::NotFound { message }
            }
            GroupAgentScheduledNodeProviderRequestServiceError::Unavailable { message } => {
                Self::Unavailable { message }
            }
            GroupAgentScheduledNodeProviderRequestServiceError::Corrupt { message } => {
                Self::Corrupt { message }
            }
        }
    }
}

impl From<GroupAgentNodeExecutionContractServiceError>
    for GroupAgentScheduledNodeDispatchReleaseControlServiceError
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

impl From<GroupAgentScheduledNodeContractServiceError>
    for GroupAgentScheduledNodeDispatchReleaseControlServiceError
{
    fn from(error: GroupAgentScheduledNodeContractServiceError) -> Self {
        match error {
            GroupAgentScheduledNodeContractServiceError::InvalidInput { message } => {
                Self::InvalidInput { message }
            }
            GroupAgentScheduledNodeContractServiceError::Conflict { message } => {
                Self::Conflict { message }
            }
            GroupAgentScheduledNodeContractServiceError::NotFound { message } => {
                Self::NotFound { message }
            }
            GroupAgentScheduledNodeContractServiceError::Unavailable { message } => {
                Self::Unavailable { message }
            }
            GroupAgentScheduledNodeContractServiceError::Corrupt { message } => {
                Self::Corrupt { message }
            }
        }
    }
}

pub(super) fn invalid(message: &str) -> GroupAgentScheduledNodeDispatchReleaseControlServiceError {
    GroupAgentScheduledNodeDispatchReleaseControlServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn corrupt(message: &str) -> GroupAgentScheduledNodeDispatchReleaseControlServiceError {
    GroupAgentScheduledNodeDispatchReleaseControlServiceError::Corrupt {
        message: message.into(),
    }
}
