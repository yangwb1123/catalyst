use forge_runtime_domain::HubStoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GroupAnalysisPanelServiceError {
    #[error("Group Analysis Panel input is invalid")]
    InvalidInput,
    #[error("frozen Group Run failed Group Analysis Panel validation")]
    InvalidSource,
    #[error("Group Analysis Panel store returned inconsistent state")]
    InconsistentStoreResult,
    #[error("Group Analysis Panel store failed: {0}")]
    Store(#[from] HubStoreError),
}
