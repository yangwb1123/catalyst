use crate::kernel_operational_contract::ArtifactRef;

use super::{
    CANONICALIZATION, CLOSURE_API, CLOSURE_DOMAIN, CLOSURE_KIND, CLOSURE_PREFIX, DecisionCapsule,
    DecisionCapsuleContractError, MAX_CAPSULE_BYTES, MAX_CLOSURE_BYTES, ReplayAttestations,
    SUCCESS_MARKER, StructuralReplayClosure, branch, capsule, invalid, primitives, wire,
};

fn body_shape(value: &StructuralReplayClosure) -> Result<(), DecisionCapsuleContractError> {
    // Outer identity profile limits precede reflection, branch, and capsule
    // local validation on every public digest/validate/seal route.
    wire::premeasure_only(
        &(&value.closure_id, &value.closure_sha256),
        MAX_CLOSURE_BYTES,
    )?;
    if value.api_version != CLOSURE_API
        || value.canonicalization != CANONICALIZATION
        || value.kind != CLOSURE_KIND
        || value.result != SUCCESS_MARKER
    {
        return Err(invalid(
            "StructuralReplayClosure constants or result differ",
        ));
    }
    primitives::attestations(&value.attestations)?;
    if value.reflection_report_artifact_refs.len() > 32 {
        return Err(invalid("reflection report ArtifactRefs exceed 32"));
    }
    primitives::reflection_refs(&value.reflection_report_artifact_refs)?;
    branch::local_shape(&value.evaluation_branch, false)?;
    capsule::local_shape(&value.decision_capsule, false)
}

fn shape(
    value: &StructuralReplayClosure,
    allow_blank: bool,
) -> Result<(), DecisionCapsuleContractError> {
    body_shape(value)?;
    primitives::identity(
        &value.closure_id,
        &value.closure_sha256,
        CLOSURE_PREFIX,
        "closure",
        allow_blank,
    )
}

fn blanked_after_validation(
    value: &StructuralReplayClosure,
) -> Result<StructuralReplayClosure, DecisionCapsuleContractError> {
    wire::premeasure_only(value, MAX_CLOSURE_BYTES)?;
    let mut blank = wire::bounded_clone(value, MAX_CLOSURE_BYTES)?;
    blank.closure_id.clear();
    blank.closure_sha256.clear();
    Ok(blank)
}

pub(super) fn validate_components(
    value: &StructuralReplayClosure,
    allow_blank: bool,
) -> Result<String, DecisionCapsuleContractError> {
    body_shape(value)?;
    wire::premeasure_only(value, MAX_CLOSURE_BYTES)?;
    if !allow_blank {
        primitives::identity(
            &value.closure_id,
            &value.closure_sha256,
            CLOSURE_PREFIX,
            "closure",
            false,
        )?;
    }
    branch::require_comparison(&value.evaluation_branch, &value.decision_capsule)?;
    capsule::validated(&value.decision_capsule)?;
    branch::validate_with_capsule(&value.evaluation_branch, &value.decision_capsule, false)?;
    let blank = blanked_after_validation(value)?;
    let checked = if allow_blank { &blank } else { value };
    let digest = wire::domain_digest(CLOSURE_DOMAIN, &blank, MAX_CLOSURE_BYTES)?;
    wire::canonical_with_max(checked, MAX_CLOSURE_BYTES)?;
    if !allow_blank && value.closure_sha256 != digest {
        return Err(invalid("closure_sha256 differs from canonical preimage"));
    }
    Ok(digest)
}

/// Computes an outer closure digest after validating the complete embedded DAG.
///
/// # Errors
///
/// Returns an error for any invalid dependency, report inventory, identity, or ceiling.
pub fn structural_replay_closure_digest(
    value: &StructuralReplayClosure,
) -> Result<String, DecisionCapsuleContractError> {
    validate_components(value, true)
}

/// Validates a sealed `StructuralReplayClosure` and every embedded dependency.
///
/// # Errors
///
/// Returns an error for any invalid dependency, report inventory, identity, or ceiling.
pub fn validate_structural_replay_closure(
    value: &StructuralReplayClosure,
) -> Result<(), DecisionCapsuleContractError> {
    shape(value, false)?;
    validate_components(value, false).map(|_| ())
}

/// Seals one blank outer closure after validating its capsule, branch and report refs.
///
/// # Errors
///
/// Returns an error for nonblank identity, invalid dependencies or reports, or oversize.
pub fn seal_structural_replay_closure(
    value: &StructuralReplayClosure,
) -> Result<StructuralReplayClosure, DecisionCapsuleContractError> {
    if !value.closure_id.is_empty() || !value.closure_sha256.is_empty() {
        return Err(invalid("sealing outer closure requires blank own identity"));
    }
    shape(value, true)?;
    wire::premeasure_only(value, MAX_CLOSURE_BYTES)?;
    let digest = validate_components(value, true)?;
    let mut sealed = wire::bounded_clone(value, MAX_CLOSURE_BYTES)?;
    sealed.closure_id = format!("{CLOSURE_PREFIX}{digest}");
    sealed.closure_sha256 = digest;
    wire::canonical_with_max(&sealed, MAX_CLOSURE_BYTES)?;
    shape(&sealed, false)?;
    Ok(sealed)
}

/// Derives the only outer closure for a capsule and unresolved reflection report refs.
///
/// # Errors
///
/// Returns an error for an invalid capsule or report inventory, or an oversized result.
pub fn derive_structural_replay_closure(
    decision_capsule: &DecisionCapsule,
    reflection_report_artifact_refs: &[ArtifactRef],
) -> Result<StructuralReplayClosure, DecisionCapsuleContractError> {
    primitives::reflection_refs(reflection_report_artifact_refs)?;
    capsule::local_shape(decision_capsule, false)?;
    wire::premeasure_only(reflection_report_artifact_refs, MAX_CLOSURE_BYTES)?;
    capsule::validated(decision_capsule)?;
    let mut closure = StructuralReplayClosure {
        api_version: CLOSURE_API.to_owned(),
        attestations: ReplayAttestations::default(),
        canonicalization: CANONICALIZATION.to_owned(),
        closure_id: String::new(),
        closure_sha256: String::new(),
        decision_capsule: wire::bounded_clone(decision_capsule, MAX_CAPSULE_BYTES)?,
        evaluation_branch: branch::derive_with_capsule(decision_capsule)?,
        kind: CLOSURE_KIND.to_owned(),
        reflection_report_artifact_refs: reflection_report_artifact_refs.to_vec(),
        result: SUCCESS_MARKER.to_owned(),
    };
    let digest = wire::domain_digest(CLOSURE_DOMAIN, &closure, MAX_CLOSURE_BYTES)?;
    closure.closure_id = format!("{CLOSURE_PREFIX}{digest}");
    closure.closure_sha256 = digest;
    wire::canonical_with_max(&closure, MAX_CLOSURE_BYTES)?;
    shape(&closure, false)?;
    Ok(closure)
}

/// Decodes exact canonical outer closure bytes and validates the full structural DAG.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or invalid dependent bytes.
pub fn decode_structural_replay_closure(
    bytes: &[u8],
) -> Result<StructuralReplayClosure, DecisionCapsuleContractError> {
    let value = wire::decode_typed(bytes, MAX_CLOSURE_BYTES)?;
    validate_structural_replay_closure(&value)?;
    Ok(value)
}
