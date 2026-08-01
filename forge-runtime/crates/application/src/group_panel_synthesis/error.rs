use crate::runtime_domain::{HubStoreError, ProviderError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GroupPanelSynthesisServiceError {
    #[error("Group Panel Synthesis input is invalid")]
    InvalidInput,
    #[error("Group Analysis Panel failed synthesis source validation")]
    InvalidSource,
    #[error("Group Panel Synthesis request encoding failed")]
    RequestEncoding,
    #[error("Group Panel Synthesis store returned inconsistent state")]
    InconsistentStoreResult,
    #[error("Group Panel Synthesis store failed: {0}")]
    Store(#[from] HubStoreError),
    #[error("Group Panel Synthesis dispatch outcome is unknown; automatic retry is disabled")]
    DispatchUnknown,
}

#[derive(Debug, Error)]
pub(crate) enum SynthesisPostClaimError {
    #[error("prepared model turn failed after dispatch")]
    Turn,
    #[error("completion store failed after dispatch")]
    Store,
    #[error("completion store returned inconsistent state")]
    InconsistentStoreResult,
}

impl From<crate::group_model_analysis_error::PostClaimError> for SynthesisPostClaimError {
    fn from(_: crate::group_model_analysis_error::PostClaimError) -> Self {
        Self::Turn
    }
}

impl From<ProviderError> for SynthesisPostClaimError {
    fn from(_: ProviderError) -> Self {
        Self::Turn
    }
}
