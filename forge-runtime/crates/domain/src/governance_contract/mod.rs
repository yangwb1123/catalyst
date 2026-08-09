mod codec;
mod model;
mod validation;

use std::fmt;

pub use model::*;

pub const API_VERSION: &str = "forgeos.governance/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const EVIDENCE_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.evidence-record.v1\0";
pub const CLAIM_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.knowledge-claim.v1\0";
pub const MAX_RECORD_BYTES: usize = 131_072;
pub const MAX_RECORD_SET_BYTES: usize = 1_048_576;
pub const MAX_DEPTH: usize = 16;
pub const MAX_OBJECT_FIELDS: usize = 64;
pub const MAX_ARRAY_ITEMS: usize = 256;
pub const MAX_STRING_BYTES: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GovernanceContractError {
    pub message: String,
}

impl fmt::Display for GovernanceContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GovernanceContractError {}

impl GovernanceRecord {
    #[must_use]
    pub fn metadata(&self) -> &RecordMetadata {
        match self {
            Self::Evidence(record) => &record.metadata,
            Self::Claim(record) => &record.metadata,
        }
    }

    #[must_use]
    pub fn integrity(&self) -> &Integrity {
        match self {
            Self::Evidence(record) => &record.integrity,
            Self::Claim(record) => &record.integrity,
        }
    }

    #[must_use]
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Evidence(_) => "EvidenceRecord",
            Self::Claim(_) => "KnowledgeClaim",
        }
    }

    /// Returns compact canonical JSON after blanking the self digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the record exceeds canonical JSON limits.
    pub fn canonical_payload_json(&self) -> Result<String, GovernanceContractError> {
        codec::canonical_payload_json(self)
    }

    /// Returns the complete compact canonical JSON record.
    ///
    /// # Errors
    ///
    /// Returns an error if the record exceeds canonical JSON limits.
    pub fn canonical_record_json(&self) -> Result<String, GovernanceContractError> {
        codec::canonical_record_json(self)
    }

    /// Computes the kind-separated digest over the blank-self-digest payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload cannot be canonically encoded.
    pub fn expected_sha256(&self) -> Result<String, GovernanceContractError> {
        codec::expected_sha256(self)
    }

    /// Validates one shadow record without asserting truth or authority.
    ///
    /// # Errors
    ///
    /// Returns an error for any structural, semantic, canonical, or digest violation.
    pub fn validate(&self) -> Result<(), GovernanceContractError> {
        validation::validate_record(self)
    }

    pub(super) fn integrity_mut(&mut self) -> &mut Integrity {
        match self {
            Self::Evidence(record) => &mut record.integrity,
            Self::Claim(record) => &mut record.integrity,
        }
    }

    pub(super) fn digest_domain(&self) -> &'static [u8] {
        match self {
            Self::Evidence(_) => EVIDENCE_DIGEST_DOMAIN,
            Self::Claim(_) => CLAIM_DIGEST_DOMAIN,
        }
    }
}

/// Decodes one exact compact canonical record and validates its shadow semantics.
///
/// # Errors
///
/// Returns an error for invalid JSON, duplicate or unknown fields, non-canonical bytes,
/// invalid semantics, or a digest mismatch.
pub fn decode_canonical_record(bytes: &[u8]) -> Result<GovernanceRecord, GovernanceContractError> {
    codec::decode_canonical_record(bytes)
}

/// Decodes one exact canonical record array and validates all cross-record references.
///
/// # Errors
///
/// Returns an error for invalid records, ordering, identity, references, or supersession.
pub fn decode_canonical_record_set(
    bytes: &[u8],
) -> Result<Vec<GovernanceRecord>, GovernanceContractError> {
    codec::decode_canonical_record_set(bytes)
}

/// Validates a nonempty, ordered set of shadow records.
///
/// # Errors
///
/// Returns an error for invalid records, identities, references, or supersession.
pub fn validate_record_set(records: &[GovernanceRecord]) -> Result<(), GovernanceContractError> {
    validation::validate_record_set(records)
}

pub(super) fn invalid(message: impl Into<String>) -> GovernanceContractError {
    GovernanceContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
