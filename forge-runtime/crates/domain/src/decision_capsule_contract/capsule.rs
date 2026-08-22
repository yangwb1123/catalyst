use crate::kernel_decision_contract::KernelDecisionReferenceClosure;

use super::{
    CANONICALIZATION, CAPSULE_API, CAPSULE_DOMAIN, CAPSULE_KIND, CAPSULE_MODE, CAPSULE_PREFIX,
    CAPSULE_RESULT, DecisionCapsule, DecisionCapsuleContractError, MAX_CAPSULE_BYTES,
    ReplayAttestations, invalid, manifest, primitives, wire,
};

fn body_shape(value: &DecisionCapsule) -> Result<(), DecisionCapsuleContractError> {
    // Keep own-field profile rejection ahead of replay-manifest validation,
    // which may perform bounded canonical comparisons of local references.
    wire::premeasure_only(
        &(&value.capsule_id, &value.capsule_sha256),
        MAX_CAPSULE_BYTES,
    )?;
    if value.api_version != CAPSULE_API
        || value.canonicalization != CANONICALIZATION
        || value.capsule_mode != CAPSULE_MODE
        || value.kind != CAPSULE_KIND
        || value.result != CAPSULE_RESULT
    {
        return Err(invalid("DecisionCapsule constants or result differ"));
    }
    primitives::attestations(&value.attestations)?;
    manifest::local_shape(&value.replay_manifest, false)
}

fn shape(value: &DecisionCapsule, allow_blank: bool) -> Result<(), DecisionCapsuleContractError> {
    body_shape(value)?;
    primitives::identity(
        &value.capsule_id,
        &value.capsule_sha256,
        CAPSULE_PREFIX,
        "capsule",
        allow_blank,
    )
}

pub(super) fn local_shape(
    value: &DecisionCapsule,
    allow_blank: bool,
) -> Result<(), DecisionCapsuleContractError> {
    shape(value, allow_blank)
}

fn blanked_after_validation(
    value: &DecisionCapsule,
) -> Result<DecisionCapsule, DecisionCapsuleContractError> {
    wire::premeasure_only(value, MAX_CAPSULE_BYTES)?;
    let mut blank = wire::bounded_clone(value, MAX_CAPSULE_BYTES)?;
    blank.capsule_id.clear();
    blank.capsule_sha256.clear();
    Ok(blank)
}

pub(super) fn validate_components(
    value: &DecisionCapsule,
    allow_blank: bool,
) -> Result<String, DecisionCapsuleContractError> {
    body_shape(value)?;
    wire::premeasure_only(value, MAX_CAPSULE_BYTES)?;
    if !allow_blank {
        primitives::identity(
            &value.capsule_id,
            &value.capsule_sha256,
            CAPSULE_PREFIX,
            "capsule",
            false,
        )?;
    }
    manifest::require_projection(&value.replay_manifest, &value.decision_closure)?;
    let closure = primitives::reseal_decision_closure(&value.decision_closure)?;
    manifest::validate_with_closure(&value.replay_manifest, &closure, false)?;
    let blank = blanked_after_validation(value)?;
    let checked = if allow_blank { &blank } else { value };
    let digest = wire::domain_digest(CAPSULE_DOMAIN, &blank, MAX_CAPSULE_BYTES)?;
    wire::canonical_with_max(checked, MAX_CAPSULE_BYTES)?;
    if !allow_blank && value.capsule_sha256 != digest {
        return Err(invalid("capsule_sha256 differs from canonical preimage"));
    }
    Ok(digest)
}

pub(super) fn validated(value: &DecisionCapsule) -> Result<(), DecisionCapsuleContractError> {
    validate_components(value, false).map(|_| ())
}

/// Computes the `DecisionCapsule` digest after fully validating its dependencies.
///
/// # Errors
///
/// Returns an error for any invalid dependency, projection, identity, or document ceiling.
pub fn decision_capsule_digest(
    value: &DecisionCapsule,
) -> Result<String, DecisionCapsuleContractError> {
    validate_components(value, true)
}

/// Validates an exact sealed `DecisionCapsule` and its complete ADR-0090 projection.
///
/// # Errors
///
/// Returns an error for any invalid dependency, projection, identity, or document ceiling.
pub fn validate_decision_capsule(
    value: &DecisionCapsule,
) -> Result<(), DecisionCapsuleContractError> {
    validated(value)
}

/// Seals one blank `DecisionCapsule` after exact dependency and byte-bound validation.
///
/// # Errors
///
/// Returns an error for nonblank identity, invalid dependencies or projection, or oversize.
pub fn seal_decision_capsule(
    value: &DecisionCapsule,
) -> Result<DecisionCapsule, DecisionCapsuleContractError> {
    if !value.capsule_id.is_empty() || !value.capsule_sha256.is_empty() {
        return Err(invalid("sealing capsule requires blank own identity"));
    }
    shape(value, true)?;
    wire::premeasure_only(value, MAX_CAPSULE_BYTES)?;
    let digest = validate_components(value, true)?;
    let mut sealed = wire::bounded_clone(value, MAX_CAPSULE_BYTES)?;
    sealed.capsule_id = format!("{CAPSULE_PREFIX}{digest}");
    sealed.capsule_sha256 = digest;
    wire::canonical_with_max(&sealed, MAX_CAPSULE_BYTES)?;
    shape(&sealed, false)?;
    Ok(sealed)
}

/// Derives the only `DecisionCapsule` admitted for a sealed ADR-0090 closure.
///
/// # Errors
///
/// Returns an error when the closure is invalid or the derived capsule exceeds its ceiling.
pub fn derive_decision_capsule(
    decision_closure: &KernelDecisionReferenceClosure,
) -> Result<DecisionCapsule, DecisionCapsuleContractError> {
    let closure = primitives::reseal_decision_closure(decision_closure)?;
    let mut capsule = DecisionCapsule {
        api_version: CAPSULE_API.to_owned(),
        attestations: ReplayAttestations::default(),
        canonicalization: CANONICALIZATION.to_owned(),
        capsule_id: String::new(),
        capsule_mode: CAPSULE_MODE.to_owned(),
        capsule_sha256: String::new(),
        decision_closure: wire::bounded_clone(
            &closure,
            crate::kernel_decision_contract::MAX_CLOSURE_BYTES,
        )?,
        kind: CAPSULE_KIND.to_owned(),
        replay_manifest: manifest::derive_with_closure(&closure)?,
        result: CAPSULE_RESULT.to_owned(),
    };
    let digest = wire::domain_digest(CAPSULE_DOMAIN, &capsule, MAX_CAPSULE_BYTES)?;
    capsule.capsule_id = format!("{CAPSULE_PREFIX}{digest}");
    capsule.capsule_sha256 = digest;
    wire::canonical_with_max(&capsule, MAX_CAPSULE_BYTES)?;
    shape(&capsule, false)?;
    Ok(capsule)
}

/// Decodes exact canonical `DecisionCapsule` bytes and validates all dependencies.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or invalid dependent bytes.
pub fn decode_decision_capsule(
    bytes: &[u8],
) -> Result<DecisionCapsule, DecisionCapsuleContractError> {
    let value = wire::decode_typed(bytes, MAX_CAPSULE_BYTES)?;
    validate_decision_capsule(&value)?;
    Ok(value)
}
