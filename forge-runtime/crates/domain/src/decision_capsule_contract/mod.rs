mod branch;
mod capsule;
mod closure;
mod manifest;
mod model;
mod primitives;
mod profile;
mod profile_key;
mod wire;

use std::fmt;

pub use branch::{
    decode_evaluation_branch, derive_evaluation_branch, evaluation_branch_digest,
    seal_evaluation_branch, validate_evaluation_branch,
};
pub use capsule::{
    decision_capsule_digest, decode_decision_capsule, derive_decision_capsule,
    seal_decision_capsule, validate_decision_capsule,
};
pub use closure::{
    decode_structural_replay_closure, derive_structural_replay_closure,
    seal_structural_replay_closure, structural_replay_closure_digest,
    validate_structural_replay_closure,
};
pub use manifest::{
    decode_structural_replay_manifest, derive_structural_replay_manifest,
    seal_structural_replay_manifest, structural_replay_manifest_digest,
    validate_structural_replay_manifest,
};
pub use model::*;
pub use wire::canonical_json;

pub const MAX_MANIFEST_BYTES: usize = 4_194_304;
pub const MAX_CAPSULE_BYTES: usize = 27_262_976;
pub const MAX_BRANCH_BYTES: usize = 65_536;
pub const MAX_CLOSURE_BYTES: usize = 29_360_128;

pub const CAPSULE_RESULT: &str = concat!(
    "STRUCTURALLY_VALID_DECISION_CAPSULE_V1 (exact caller-supplied ADR-0090 ",
    "closure and complete projection of the embedded caller-supplied closure only; ",
    "replay is validate/reseal/compare only; no effect replay or history rewrite; ",
    "all thirty-two replay attestations are false)"
);
pub const SUCCESS_MARKER: &str = concat!(
    "STRUCTURALLY_VALID_DECISION_CAPSULE_REPLAY_CLOSURE_V1 (exact caller-supplied ",
    "DecisionCapsule and separately sealed deterministic structural comparison only; ",
    "dedicated ReflectionReport ArtifactRefs are unresolved and attached only by the outer ",
    "closure; upstream ArtifactRefs remain opaque and uninterpreted; no model, rule or ",
    "world-state evaluation, effect replay, history rewrite, authorization, persistence, ",
    "PDP or controller; all thirty-two replay attestations are false)"
);

pub(super) const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub(super) const MANIFEST_API: &str = "forgeos.aadm.structural-replay-manifest/v1";
pub(super) const MANIFEST_KIND: &str = "StructuralReplayManifest";
pub(super) const MANIFEST_MODE: &str = "structural_validate_reseal_compare_only";
pub(super) const MANIFEST_PREFIX: &str = "structural-replay-manifest-";
pub(super) const MANIFEST_DOMAIN: &[u8] = b"forgeos.aadm.structural-replay-manifest.v1\0";
pub(super) const CAPSULE_API: &str = "forgeos.aadm.decision-capsule/v1";
pub(super) const CAPSULE_KIND: &str = "DecisionCapsule";
pub(super) const CAPSULE_MODE: &str = "structural_replay_manifest_only";
pub(super) const CAPSULE_PREFIX: &str = "decision-capsule-";
pub(super) const CAPSULE_DOMAIN: &[u8] = b"forgeos.aadm.decision-capsule.v1\0";
pub(super) const BRANCH_API: &str = "forgeos.aadm.evaluation-branch/v1";
pub(super) const BRANCH_KIND: &str = "EvaluationBranch";
pub(super) const BRANCH_MODE: &str = "structural_validate_reseal_compare_only";
pub(super) const BRANCH_PREFIX: &str = "evaluation-branch-";
pub(super) const BRANCH_DOMAIN: &[u8] = b"forgeos.aadm.evaluation-branch.v1\0";
pub(super) const COMPARISON_RESULT: &str = "EXACT_STRUCTURAL_REFERENCE_MATCH_ONLY";
pub(super) const CLOSURE_API: &str = "forgeos.aadm.structural-replay-closure/v1";
pub(super) const CLOSURE_KIND: &str = "StructuralReplayClosure";
pub(super) const CLOSURE_PREFIX: &str = "structural-replay-closure-";
pub(super) const CLOSURE_DOMAIN: &[u8] = b"forgeos.aadm.structural-replay-closure.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecisionCapsuleContractError {
    pub message: String,
}

impl fmt::Display for DecisionCapsuleContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for DecisionCapsuleContractError {}

pub(super) fn invalid(message: impl Into<String>) -> DecisionCapsuleContractError {
    DecisionCapsuleContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
