mod assessment_model;
mod assessment_validation;
mod codec;
mod compatibility;
mod compatibility_model;
mod evaluator;
mod model;
mod primitives;
mod validation;
mod vocabulary;

use std::fmt;

pub use assessment_model::*;
pub use codec::*;
pub use compatibility::*;
pub use compatibility_model::*;
pub use evaluator::{declared_target, evaluate_declared_assessment, validate_assessment};
pub use model::*;
pub use vocabulary::transition_vocabulary;

pub const VOCABULARY_API_VERSION: &str = "forgeos.transition-state-vocabulary/v1";
pub const RECEIPT_API_VERSION: &str = "forgeos.transition-receipt/v1";
pub const ASSESSMENT_REQUEST_API_VERSION: &str =
    "forgeos.transition-receipt-declared-assessment-request/v1";
pub const ASSESSMENT_API_VERSION: &str = "forgeos.transition-receipt-declared-assessment/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const VOCABULARY_KIND: &str = "TransitionStateVocabulary";
pub const RECEIPT_KIND: &str = "TransitionReceipt";
pub const ASSESSMENT_MODE: &str = "authority_neutral_declared_transition_only";
pub const TRANSITION_VOCABULARY_SHA256: &str =
    "cc354fb2b440d81514045b50266d41d3964b6440ed9d40afa17f5991519d7d0d";

pub const ASSESSMENT_RESULT: &str = "ASSESSED_TRANSITION_DECLARATIONS_ONLY (no controller, actor, Grant, Approval, evidence, waiver, precondition or state authentication; no policy decision, authorization, persistence, transition, ledger, execution, effect or completion attestation)";
pub const GRANT_COMPATIBILITY_RESULT: &str =
    "ASSESSED_GRANT_TRANSITION_DECLARATIONS_ONLY (no permission or transition authority)";
pub const APPROVAL_COMPATIBILITY_RESULT: &str = "ASSESSED_APPROVAL_TRANSITION_DECLARATIONS_ONLY (no effective approval or transition authority)";

pub const VOCABULARY_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.transition-state-vocabulary.v1\0";
pub const RECEIPT_DIGEST_DOMAIN: &[u8] = b"forgeos.transition-receipt.v1\0";
pub const DECLARED_TARGET_DIGEST_DOMAIN: &[u8] = b"forgeos.transition-declared-target.v1\0";
pub const ASSESSMENT_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.transition-receipt-declared-assessment-request.v1\0";
pub const ASSESSMENT_DIGEST_DOMAIN: &[u8] = b"forgeos.transition-receipt-declared-assessment.v1\0";

pub const MAX_VOCABULARY_BYTES: usize = 262_144;
pub const MAX_RECEIPT_BYTES: usize = 1_048_576;
pub const MAX_DECLARED_TARGET_BYTES: usize = 1_048_576;
pub const MAX_ASSESSMENT_REQUEST_BYTES: usize = 4_194_304;
pub const MAX_ASSESSMENT_BYTES: usize = 262_144;
pub const MAX_SHORT_TEXT_BYTES: usize = 160;
pub const MAX_REFERENCE_TEXT_BYTES: usize = 4_096;
pub const MAX_TOTAL_EVIDENCE_REFS: usize = 256;
pub const MAX_TOTAL_REASON_CODES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionReceiptContractError {
    pub message: String,
}

impl fmt::Display for TransitionReceiptContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TransitionReceiptContractError {}

pub(super) fn invalid(message: impl Into<String>) -> TransitionReceiptContractError {
    TransitionReceiptContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
