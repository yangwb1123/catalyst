mod assessment;
mod canonical;
mod codec;
mod evaluator;
mod grant;
mod grant_validation;
mod primitives;
mod scope;
mod scope_validation;
mod validation;
mod vocabulary;
mod vocabulary_validation;

use std::fmt;

pub use assessment::*;
pub use codec::*;
pub use evaluator::{evaluate_declared_assessment, validate_assessment};
pub use grant::*;
pub use scope::*;
pub use vocabulary::*;

pub const EFFECT_VOCABULARY_API_VERSION: &str = "forgeos.governance.effect-vocabulary/v1";
pub const CAPABILITY_GRANT_API_VERSION: &str = "forgeos.capability-grant/v1";
pub const ASSESSMENT_REQUEST_API_VERSION: &str =
    "forgeos.capability-grant-declared-assessment-request/v1";
pub const ASSESSMENT_API_VERSION: &str = "forgeos.capability-grant-declared-assessment/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const EFFECT_VOCABULARY_KIND: &str = "EffectVocabulary";
pub const CAPABILITY_GRANT_KIND: &str = "CapabilityGrant";
pub const ASSESSMENT_MODE: &str = "authority_neutral_declared_envelope_only";
pub const ASSESSMENT_RESULT: &str = "ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, approval, revocation, usage, preflight, authorization, permission, persistence, execution, or effect attestation)";

pub const EFFECT_VOCABULARY_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.effect-vocabulary.v1\0";
pub const GRANT_DIGEST_DOMAIN: &[u8] = b"forgeos.capability-grant.v1\0";
pub const REQUESTED_ACTION_DIGEST_DOMAIN: &[u8] = b"forgeos.capability-requested-action.v1\0";
pub const ASSESSMENT_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.capability-grant-declared-assessment-request.v1\0";
pub const ASSESSMENT_DIGEST_DOMAIN: &[u8] = b"forgeos.capability-grant-declared-assessment.v1\0";

pub const MAX_VOCABULARY_BYTES: usize = 128 * 1_024;
pub const MAX_GRANT_BYTES: usize = 1_024 * 1_024;
pub const MAX_ASSESSMENT_REQUEST_BYTES: usize = 2 * 1_024 * 1_024;
pub const MAX_ASSESSMENT_BYTES: usize = 256 * 1_024;
pub const MAX_DEPTH: usize = 16;
pub const MAX_OBJECT_FIELDS: usize = 64;
pub const MAX_ARRAY_ITEMS: usize = 256;
pub const MAX_STRING_BYTES: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityGrantContractError {
    pub message: String,
}

impl fmt::Display for CapabilityGrantContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CapabilityGrantContractError {}

pub(super) fn invalid(message: impl Into<String>) -> CapabilityGrantContractError {
    CapabilityGrantContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
