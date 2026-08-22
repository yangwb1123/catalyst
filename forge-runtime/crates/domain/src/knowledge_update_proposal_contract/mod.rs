mod assessment;
mod canonical;
mod closure;
mod codec;
mod compatibility;
mod compatibility_model;
mod model;
mod primitives;
mod validation;

use std::fmt;

pub use assessment::{
    evaluate_declared_assessment, seal_assessment_request, seal_proposal, validate_assessment,
};
pub use codec::*;
pub use compatibility::*;
pub use compatibility_model::*;
pub use model::*;
pub use validation::{declared_target, validate_proposal};

pub const PROPOSAL_API_VERSION: &str = "forgeos.knowledge-update-proposal/v1";
pub const ASSESSMENT_REQUEST_API_VERSION: &str =
    "forgeos.knowledge-update-proposal-declared-assessment-request/v1";
pub const ASSESSMENT_API_VERSION: &str = "forgeos.knowledge-update-proposal-declared-assessment/v1";
pub const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub const PROPOSAL_KIND: &str = "KnowledgeUpdateProposal";
pub const ASSESSMENT_MODE: &str = "authority_neutral_declared_knowledge_update_only";
pub const ASSESSMENT_RESULT: &str = "ASSESSED_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY (no proposer, Grant, Context, evidence, current-knowledge, conflict, freshness, policy or authority evaluation; no truth, adoption, authorization, permission, persistence, apply, receipt, execution or effect attestation)";

pub const RECORD_SET_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.record-set.v1\0";
pub const PROPOSAL_DIGEST_DOMAIN: &[u8] = b"forgeos.knowledge-update-proposal.v1\0";
pub const DECLARED_TARGET_DIGEST_DOMAIN: &[u8] = b"forgeos.knowledge-update-declared-target.v1\0";
pub const ASSESSMENT_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forgeos.knowledge-update-proposal-declared-assessment-request.v1\0";
pub const ASSESSMENT_DIGEST_DOMAIN: &[u8] =
    b"forgeos.knowledge-update-proposal-declared-assessment.v1\0";

pub const MAX_PROPOSAL_BYTES: usize = 2_097_152;
pub const MAX_DECLARED_TARGET_BYTES: usize = 1_048_576;
pub const MAX_ASSESSMENT_REQUEST_BYTES: usize = 4_194_304;
pub const MAX_ASSESSMENT_BYTES: usize = 262_144;
pub const MAX_GOLDEN_BYTES: usize = 8_388_608;
pub const MAX_SHORT_TEXT_BYTES: usize = 160;
pub const MAX_REFERENCE_TEXT_BYTES: usize = 4_096;
pub const MAX_MUTATIONS: usize = 64;
pub const MAX_MUTATION_REASON_CODES: usize = 16;
pub const MAX_ARTIFACTS: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeUpdateProposalContractError {
    pub message: String,
}

impl fmt::Display for KnowledgeUpdateProposalContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for KnowledgeUpdateProposalContractError {}

pub(super) fn invalid(message: impl Into<String>) -> KnowledgeUpdateProposalContractError {
    KnowledgeUpdateProposalContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
