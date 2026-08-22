mod assessment_validation;
mod codec;
mod evaluator;
mod model;
mod primitives;
mod record_validation;
mod reference;

use std::fmt;

pub use codec::*;
pub use evaluator::{evaluate_declared_assessment, validate_assessment};
pub use model::*;
pub use reference::{ApprovalRefRelation, approval_ref, approval_ref_relation, declared_target};

pub const APPROVAL_API_VERSION: &str = "forgeos.approval-record/v1";
pub const ASSESSMENT_REQUEST_API_VERSION: &str =
    "forgeos.approval-record-declared-assessment-request/v1";
pub const ASSESSMENT_API_VERSION: &str = "forgeos.approval-record-declared-assessment/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const APPROVAL_KIND: &str = "ApprovalRecord";
pub const ASSESSMENT_MODE: &str = "authority_neutral_declared_approval_only";
pub const EFFECT_VOCABULARY_SHA256: &str =
    "a45de832e43ccdbebcb22f183575039d451594bfbc9ec713105c657a6adda49f";
pub const ASSESSMENT_RESULT: &str = "ASSESSED_APPROVAL_DECLARATIONS_ONLY (no approver or authority authentication, attestation or SoD proof verification, condition or RiskAcceptance validation, revocation evaluation, policy decision, effective approval, authorization, permission, persistence, transition, execution, or effect attestation)";

pub const APPROVAL_DIGEST_DOMAIN: &[u8] = b"forgeos.approval-record.v1\0";
pub const DECLARED_TARGET_DIGEST_DOMAIN: &[u8] = b"forgeos.approval-declared-target.v1\0";
pub const ASSESSMENT_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.approval-record-declared-assessment-request.v1\0";
pub const ASSESSMENT_DIGEST_DOMAIN: &[u8] = b"forgeos.approval-record-declared-assessment.v1\0";

pub const MAX_RECORD_BYTES: usize = 1_048_576;
pub const MAX_DECLARED_TARGET_BYTES: usize = 1_048_576;
pub const MAX_ASSESSMENT_REQUEST_BYTES: usize = 2_097_152;
pub const MAX_ASSESSMENT_BYTES: usize = 262_144;
pub const MAX_JSON_DEPTH: usize = crate::governance_contract::MAX_DEPTH;
pub const MAX_OBJECT_FIELDS: usize = crate::governance_contract::MAX_OBJECT_FIELDS;
pub const MAX_ARRAY_ITEMS: usize = crate::governance_contract::MAX_ARRAY_ITEMS;
pub const MAX_STRING_BYTES: usize = crate::governance_contract::MAX_STRING_BYTES;
pub const MAX_SHORT_TEXT_BYTES: usize = 160;
pub const MAX_PROOF_TEXT_BYTES: usize = 16_384;
pub const MAX_VALIDITY_MS: i64 = 86_400_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApprovalRecordContractError {
    pub message: String,
}

impl fmt::Display for ApprovalRecordContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApprovalRecordContractError {}

pub(super) fn invalid(message: impl Into<String>) -> ApprovalRecordContractError {
    ApprovalRecordContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
