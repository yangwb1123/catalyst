mod evaluate;
mod identity;
mod lifecycle;
mod model;
mod projection;
mod state;
mod store;
mod validation;

pub use evaluate::{
    evaluate_governance_semantic_projection, governance_claim_conflict_groups,
    governance_validation_job,
};
pub use lifecycle::{validate_governance_semantic_append, validate_governance_semantic_transition};
pub use model::*;
pub use projection::{governance_semantic_projection, governance_semantic_projection_sha256};
pub use store::GovernanceSemanticViewStore;

pub const GOVERNANCE_SEMANTIC_VIEW_VERSION: u16 = 1;
pub const GOVERNANCE_SEMANTIC_PROJECTION_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.semantic-projection.v1\0";
pub const GOVERNANCE_CLAIM_CONFLICT_KEY_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.claim-conflict-key.v1\0";
pub const GOVERNANCE_CLAIM_OBJECT_DIGEST_DOMAIN: &[u8] = b"forgeos.governance.claim-object.v1\0";
pub const GOVERNANCE_VALIDATION_PLAN_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.validation-plan.v1\0";
pub const GOVERNANCE_VALIDATION_JOB_DIGEST_DOMAIN: &[u8] =
    b"forgeos.governance.validation-job.v1\0";
pub const GOVERNANCE_VALIDATION_JOB_ID_PREFIX: &str = "governance-validation-job-";
pub const MAX_GOVERNANCE_SEMANTIC_LIST_LIMIT: usize = 100;
pub const MAX_GOVERNANCE_CONFLICT_MEMBERS: usize = 100;
pub const MAX_GOVERNANCE_SEMANTIC_SCAN_RECORDS: usize = 10_000;

pub(crate) use super::GovernanceRecordJournalError;
pub(crate) use validation::{digest_hex, invalid};

#[cfg(test)]
mod tests;
