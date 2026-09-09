use std::fmt;

use crate::runtime_domain::{
    execution::attempt::AttemptRequest, platform_core_contract::EventEnvelope,
};

/// Stable storage failures without embedded SQL, records or sensitive diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptJournalError {
    Invalid,
    Conflict,
    Capacity,
    Corrupt,
    Unavailable,
}

impl fmt::Display for AttemptJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "attempt_journal_invalid",
            Self::Conflict => "attempt_journal_conflict",
            Self::Capacity => "attempt_journal_capacity",
            Self::Corrupt => "attempt_journal_corrupt",
            Self::Unavailable => "attempt_journal_unavailable",
        })
    }
}

impl std::error::Error for AttemptJournalError {}

/// Whether this operation created a record or returned the original exact admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionDisposition {
    Created,
    Replayed,
}

/// A validated owned requested admission; it is not an execution receipt.
#[derive(Clone, Debug, PartialEq)]
pub struct AttemptAdmission {
    pub cursor: u64,
    pub request: AttemptRequest,
    pub request_sha256: String,
    pub event: EventEnvelope,
    pub canonical_event_json: String,
    pub event_sha256: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdmissionResult {
    pub disposition: AdmissionDisposition,
    pub admission: AttemptAdmission,
}

/// An immutable creation event whose delivery is still pending.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingEvent {
    pub cursor: u64,
    pub event: EventEnvelope,
    pub canonical_event_json: String,
    pub event_sha256: String,
}

/// Cursors are local to this database, without transport identity or acknowledgement.
#[derive(Clone, Debug, PartialEq)]
pub struct PendingPage {
    pub head_cursor: u64,
    pub next_cursor: u64,
    pub has_more: bool,
    pub events: Vec<PendingEvent>,
}
