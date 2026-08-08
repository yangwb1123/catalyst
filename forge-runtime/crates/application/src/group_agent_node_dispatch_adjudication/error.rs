use crate::runtime_domain::HubStoreError;
use thiserror::Error;

/// A distinct, message-stable refusal cause for the one-shot adjudication.
///
/// Every message starts with the stable literal `adjudication refused:` so
/// automation can match stderr. Refusal-vs-failure is message-only by design:
/// the codebase has no structured per-command exit codes (uniform
/// `ExitCode::FAILURE`), and `--json` output remains the structured channel.
/// This type deliberately has no `DispatchQuarantined` cause — execute's
/// blanket Core→quarantine mapping is execute-specific and would mislead here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdjudicationRefused {
    /// Run is not a stranded hard-crash claim (already terminal, run not
    /// `dispatch_unknown`, or no claim/active lane). Zero mutation, retryable.
    NotStranded { reason: String },
    /// Operator authorization/pricing body digests disagree with the persisted
    /// claim. Zero mutation, retryable with corrected bodies.
    DigestMismatch { field: &'static str },
    /// The pinned Core rejected the hard-crash terminal (old Core without
    /// `hard_crash` support, Core validation refusal incl. clock skew, or
    /// handshake-timeout interplay). Zero mutation, retryable after re-pin.
    CoreRefused { detail: String },
    /// A live executor won the terminalize CAS first. Zero mutation, retryable.
    CasConflict,
}

impl std::fmt::Display for AdjudicationRefused {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotStranded { reason } => {
                write!(formatter, "adjudication refused: {reason}")
            }
            Self::DigestMismatch { field } => write!(
                formatter,
                "adjudication refused: {field} body does not match the claimed digest"
            ),
            Self::CoreRefused { detail } => write!(
                formatter,
                "adjudication refused: Core refused the hard-crash terminal ({detail}); \
                 re-pin to a Core with hard_crash support"
            ),
            Self::CasConflict => write!(
                formatter,
                "adjudication refused: concurrent executor terminalized the claim; \
                 re-run to see the committed state"
            ),
        }
    }
}

impl std::error::Error for AdjudicationRefused {}

#[derive(Debug, Error)]
pub enum GroupAgentNodeDispatchAdjudicationServiceError {
    #[error("Group Agent Node Dispatch adjudication input is invalid")]
    InvalidInput,
    #[error("Group Agent Node Dispatch adjudication state is not ready")]
    InvalidState,
    /// Any store failure, including the 5 s busy-timeout `Unavailable` variant
    /// (live-executor WAL contention). Retryable; adjudication owns no lane and
    /// never maps anything to `DispatchQuarantined`.
    #[error("Group Agent Node Dispatch adjudication store failed: {0}")]
    Store(#[from] HubStoreError),
    #[error("{0}")]
    Refused(#[from] AdjudicationRefused),
}
