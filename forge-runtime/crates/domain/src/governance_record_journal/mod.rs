mod append_validation;
mod model;
mod store;
mod validation;

use std::fmt;

pub use append_validation::{
    validate_governance_record_append, validate_governance_record_relations,
};
pub use model::*;
pub use store::GovernanceRecordJournalStore;
pub use validation::is_governance_record_identifier;
pub use validation::{
    governance_record_append_request_sha256, governance_record_set_sha256,
    validate_governance_record_idempotency_key,
};

pub const GOVERNANCE_RECORD_JOURNAL_VERSION: u16 = 1;
pub const GOVERNANCE_RECORD_SET_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.record-set.v1\0";
pub const GOVERNANCE_RECORD_APPEND_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.record-journal.append-request.v1\0";
pub const GOVERNANCE_RECORD_BATCH_ID_PREFIX: &str = "governance-record-batch-";
pub const MAX_GOVERNANCE_RECORD_IDENTIFIER_BYTES: usize = 160;
pub const MAX_GOVERNANCE_RECORD_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_GOVERNANCE_RECORD_LIST_LIMIT: usize = 100;
pub const MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS: usize = 1_024;
pub const MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceRecordJournalError {
    pub message: String,
}

impl fmt::Display for GovernanceRecordJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GovernanceRecordJournalError {}

impl AppendGovernanceRecordBatch {
    /// Builds a deterministic append request from exact canonical record-set bytes.
    ///
    /// The local append timestamp is receipt metadata and is deliberately excluded from
    /// request identity, allowing a later exact replay to return the original receipt.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid canonical records, key, size, or timestamp.
    pub fn from_canonical_record_set(
        canonical_record_set_json: String,
        idempotency_key: String,
        appended_at_ms: u64,
    ) -> Result<Self, GovernanceRecordJournalError> {
        validation::build_append_request(canonical_record_set_json, idempotency_key, appended_at_ms)
    }

    /// Validates every deterministic identity and the exact canonical append batch.
    ///
    /// # Errors
    ///
    /// Returns an error for any malformed or divergent request field.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        validation::validate_request(self)
    }

    /// Decodes the exact candidate records without resolving journal-external references.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, non-canonical, or individually invalid records.
    pub fn records(
        &self,
    ) -> Result<Vec<crate::governance_contract::GovernanceRecord>, GovernanceRecordJournalError>
    {
        validation::request_records(self)
    }
}

impl GovernanceRecordAppendReceipt {
    /// Validates bounded immutable receipt metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identity, digest, count, or ordering.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        validation::validate_receipt(self)
    }
}

impl GovernanceRecordMetadata {
    /// Validates one content-minimized journal metadata projection.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or out-of-range metadata.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        validation::validate_metadata(self)
    }
}

impl GovernanceRecordInspection {
    /// Validates metadata and any explicitly revealed exact canonical record.
    ///
    /// # Errors
    ///
    /// Returns an error for metadata/content divergence.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        validation::validate_inspection(self)
    }
}

impl GovernanceStructuralHead {
    /// Validates only structural sequence projection metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed fields; no epistemic meaning is inferred.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        validation::validate_head(self)
    }
}

impl GovernanceRecordListFilter {
    /// Validates bounded metadata filters and explicit content-reveal intent.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid aggregate identifier or list limit.
    pub fn validate(&self) -> Result<(), GovernanceRecordJournalError> {
        validation::validate_filter(self)
    }
}

fn invalid(message: impl Into<String>) -> GovernanceRecordJournalError {
    GovernanceRecordJournalError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
