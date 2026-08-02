use crate::runtime_domain::{HubStoreError, ProviderError};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GroupAgentScheduledNodeProviderRequestServiceError {
    #[error("Scheduled Node Provider Request input is invalid: {message}")]
    InvalidInput { message: String },
    #[error("Scheduled Node Provider Request operation conflicts: {message}")]
    Conflict { message: String },
    #[error("Scheduled Node Provider Request source was not found: {message}")]
    NotFound { message: String },
    #[error("Scheduled Node Provider Request storage is unavailable: {message}")]
    Unavailable { message: String },
    #[error("Scheduled Node Provider Request durable state is corrupt: {message}")]
    Corrupt { message: String },
}

impl From<HubStoreError> for GroupAgentScheduledNodeProviderRequestServiceError {
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

impl From<super::GroupAgentScheduledNodeContractServiceError>
    for GroupAgentScheduledNodeProviderRequestServiceError
{
    fn from(error: super::GroupAgentScheduledNodeContractServiceError) -> Self {
        use super::GroupAgentScheduledNodeContractServiceError as Source;
        match error {
            Source::InvalidInput { message } => Self::InvalidInput { message },
            Source::Conflict { message } => Self::Conflict { message },
            Source::NotFound { message } => Self::NotFound { message },
            Source::Unavailable { message } => Self::Unavailable { message },
            Source::Corrupt { message } => Self::Corrupt { message },
        }
    }
}

pub(super) fn invalid(message: &str) -> GroupAgentScheduledNodeProviderRequestServiceError {
    GroupAgentScheduledNodeProviderRequestServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) fn corrupt(message: &str) -> GroupAgentScheduledNodeProviderRequestServiceError {
    GroupAgentScheduledNodeProviderRequestServiceError::Corrupt {
        message: message.into(),
    }
}

pub(super) fn codec_input_error(
    error: &ProviderError,
) -> GroupAgentScheduledNodeProviderRequestServiceError {
    invalid(&format!(
        "provider request codec rejected the scheduled contract: {error}"
    ))
}
