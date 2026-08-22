mod codec;
mod model;
mod validation;
mod wire;

use std::fmt;

pub use codec::{
    canonical_work_intent_json, decode_canonical_work_intent, seal_work_intent, work_intent_sha256,
};
pub use model::*;
pub use validation::validate_work_intent;

pub const API_VERSION: &str = "forgeos.work-intent/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const DIGEST_DOMAIN: &[u8] = b"forgeos.work-intent.v1\0";
pub const MAX_RECORD_BYTES: usize = 262_144;
pub const MAX_JSON_DEPTH: usize = 8;
pub const MAX_OBJECT_FIELDS: usize = 32;
pub const MAX_ARRAY_ITEMS: usize = 256;
pub const MAX_STRING_BYTES: usize = 16_384;
pub const MAX_SHORT_TEXT_BYTES: usize = 160;
pub const MAX_REFERENCE_TEXT_BYTES: usize = 4_096;
pub const MAX_NARRATIVE_ITEMS: usize = 64;
pub const MAX_NARRATIVE_TOTAL: usize = 256;
pub const MAX_RECORD_REFS_PER_KIND: usize = 64;
pub const MAX_RECORD_REFS_TOTAL: usize = 128;
pub const MAX_LOCAL_ARTIFACTS: usize = 32;
pub const SUCCESS_MARKER: &str = "STRUCTURALLY_VALID_DECLARED_WORK_INTENT_V1 (exact caller-supplied declaration only; no origin authentication, reference resolution, G0, routing, Run or RunJournal existence, lifecycle, approval, authentication, authority, completion, effect, execution, freshness, materiality, ownership, permission, persistence, scope, or truth attestation)";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkIntentContractError {
    pub message: String,
}

impl fmt::Display for WorkIntentContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WorkIntentContractError {}

impl WorkIntent {
    /// Validates exact `WorkIntent` v1 structure and self-identity declarations.
    ///
    /// # Errors
    ///
    /// Returns an error for any wire, semantic, ordering, size, or digest violation.
    pub fn validate(&self) -> Result<(), WorkIntentContractError> {
        validate_work_intent(self)
    }
}

pub(super) fn invalid(message: impl Into<String>) -> WorkIntentContractError {
    WorkIntentContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
