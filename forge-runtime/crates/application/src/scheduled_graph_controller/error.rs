use thiserror::Error;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ScheduledGraphControllerServiceError {
    #[error("scheduled Graph controller input is invalid")]
    InvalidInput,
    #[error("scheduled Graph controller does not match the pinned Core")]
    CorePinMismatch,
    #[error("scheduled Graph controller schedule is incompatible")]
    IncompatibleSchedule,
    #[error("scheduled Graph controller consent anchors are stale")]
    StaleConsent,
    #[error("fresh exact-request off-machine consent is required")]
    ConsentRequired,
    #[error("fresh predecessor-content consent is required")]
    PredecessorContentConsentRequired,
    #[error("scheduled Graph controller journal is unavailable")]
    StoreUnavailable,
    #[error("scheduled Graph controller journal changed concurrently")]
    ConcurrentUpdate,
    #[error("scheduled Graph reconciliation failed")]
    ReconcileFailed,
    #[error("scheduled Graph candidate materialization failed")]
    MaterializationFailed,
    #[error("scheduled Graph candidate admission failed")]
    AdmissionFailed,
    #[error("scheduled Graph provider-request preparation failed")]
    PreparationFailed,
    #[error("scheduled Graph ready authorization failed")]
    AuthorizationFailed,
    #[error("scheduled Graph pricing evidence is unavailable")]
    PricingUnavailable,
    #[error("scheduled Graph ready-node execution failed")]
    ExecutionFailed,
    #[error("scheduled Graph controller observed corrupt durable evidence")]
    CorruptEvidence,
}
