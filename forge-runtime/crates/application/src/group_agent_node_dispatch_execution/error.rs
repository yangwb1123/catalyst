use crate::runtime_domain::HubStoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GroupAgentNodeDispatchExecutionServiceError {
    #[error("Group Agent Node Dispatch input is invalid")]
    InvalidInput,
    #[error("Group Agent Node Dispatch supports only one-node Graphs in protocol v1")]
    UnsupportedTopology,
    #[error("fresh --confirm-off-machine consent is required")]
    ConsentRequired,
    #[error("Group Agent Node Dispatch credential is unavailable")]
    CredentialUnavailable,
    #[error("registered Group Agent Node provider is unavailable")]
    ProviderUnavailable,
    #[error("Group Agent Node Dispatch state is not ready")]
    InvalidState,
    #[error("Group Agent Node Dispatch is durably claimed and quarantined; resend is forbidden")]
    DispatchQuarantined,
    #[error("Group Agent Node Dispatch store failed: {0}")]
    Store(#[from] HubStoreError),
}
