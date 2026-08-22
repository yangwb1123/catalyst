use super::{
    BRANCH_API, BRANCH_DOMAIN, BRANCH_KIND, BRANCH_MODE, BRANCH_PREFIX, CANONICALIZATION,
    CAPSULE_PREFIX, COMPARISON_RESULT, DecisionCapsule, DecisionCapsuleContractError,
    DecisionCapsuleRef, DecisionClosureRef, EvaluationBranch, MANIFEST_PREFIX, MAX_BRANCH_BYTES,
    ReplayAttestations, StructuralReplayManifestRef, capsule, invalid, primitives, wire,
};

fn candidate_validated(capsule: &DecisionCapsule) -> EvaluationBranch {
    EvaluationBranch {
        api_version: BRANCH_API.to_owned(),
        attestations: ReplayAttestations::default(),
        branch_id: String::new(),
        branch_mode: BRANCH_MODE.to_owned(),
        branch_sha256: String::new(),
        canonicalization: CANONICALIZATION.to_owned(),
        capsule_ref: DecisionCapsuleRef {
            capsule_id: capsule.capsule_id.clone(),
            capsule_sha256: capsule.capsule_sha256.clone(),
        },
        comparison_result: COMPARISON_RESULT.to_owned(),
        decision_closure_ref: DecisionClosureRef {
            closure_id: capsule.decision_closure.closure_id.clone(),
            closure_sha256: capsule.decision_closure.closure_sha256.clone(),
        },
        effect_replay_allowed: false,
        history_rewrite_allowed: false,
        kind: BRANCH_KIND.to_owned(),
        manifest_ref: StructuralReplayManifestRef {
            manifest_id: capsule.replay_manifest.manifest_id.clone(),
            manifest_sha256: capsule.replay_manifest.manifest_sha256.clone(),
        },
    }
}

fn comparison_matches(value: &EvaluationBranch, capsule: &DecisionCapsule) -> bool {
    value.capsule_ref.capsule_id == capsule.capsule_id
        && value.capsule_ref.capsule_sha256 == capsule.capsule_sha256
        && value.decision_closure_ref.closure_id == capsule.decision_closure.closure_id
        && value.decision_closure_ref.closure_sha256 == capsule.decision_closure.closure_sha256
        && value.manifest_ref.manifest_id == capsule.replay_manifest.manifest_id
        && value.manifest_ref.manifest_sha256 == capsule.replay_manifest.manifest_sha256
}

pub(super) fn require_comparison(
    value: &EvaluationBranch,
    capsule: &DecisionCapsule,
) -> Result<(), DecisionCapsuleContractError> {
    if comparison_matches(value, capsule) {
        Ok(())
    } else {
        Err(invalid("branch is not the unique capsule comparison"))
    }
}

fn body_shape(value: &EvaluationBranch) -> Result<(), DecisionCapsuleContractError> {
    // Reject invalid own strings before consulting the capsule's local shell.
    wire::premeasure_only(&(&value.branch_id, &value.branch_sha256), MAX_BRANCH_BYTES)?;
    if value.api_version != BRANCH_API
        || value.branch_mode != BRANCH_MODE
        || value.canonicalization != CANONICALIZATION
        || value.comparison_result != COMPARISON_RESULT
        || value.kind != BRANCH_KIND
    {
        return Err(invalid("EvaluationBranch constants differ"));
    }
    if value.effect_replay_allowed || value.history_rewrite_allowed {
        return Err(invalid("branch replay controls must be false"));
    }
    primitives::attestations(&value.attestations)?;
    primitives::identity(
        &value.capsule_ref.capsule_id,
        &value.capsule_ref.capsule_sha256,
        CAPSULE_PREFIX,
        "capsule_ref",
        false,
    )?;
    primitives::identity(
        &value.decision_closure_ref.closure_id,
        &value.decision_closure_ref.closure_sha256,
        "kernel-decision-reference-closure-",
        "decision_closure_ref",
        false,
    )?;
    primitives::identity(
        &value.manifest_ref.manifest_id,
        &value.manifest_ref.manifest_sha256,
        MANIFEST_PREFIX,
        "manifest_ref",
        false,
    )
}

fn shape(value: &EvaluationBranch, allow_blank: bool) -> Result<(), DecisionCapsuleContractError> {
    body_shape(value)?;
    primitives::identity(
        &value.branch_id,
        &value.branch_sha256,
        BRANCH_PREFIX,
        "branch",
        allow_blank,
    )
}

pub(super) fn local_shape(
    value: &EvaluationBranch,
    allow_blank: bool,
) -> Result<(), DecisionCapsuleContractError> {
    shape(value, allow_blank)
}

fn blanked_after_validation(
    value: &EvaluationBranch,
) -> Result<EvaluationBranch, DecisionCapsuleContractError> {
    wire::premeasure_only(value, MAX_BRANCH_BYTES)?;
    let mut blank = wire::bounded_clone(value, MAX_BRANCH_BYTES)?;
    blank.branch_id.clear();
    blank.branch_sha256.clear();
    Ok(blank)
}

pub(super) fn validate_with_capsule(
    value: &EvaluationBranch,
    capsule: &DecisionCapsule,
    allow_blank: bool,
) -> Result<String, DecisionCapsuleContractError> {
    body_shape(value)?;
    wire::premeasure_only(value, MAX_BRANCH_BYTES)?;
    if !allow_blank {
        primitives::identity(
            &value.branch_id,
            &value.branch_sha256,
            BRANCH_PREFIX,
            "branch",
            false,
        )?;
    }
    require_comparison(value, capsule)?;
    let blank = blanked_after_validation(value)?;
    let checked = if allow_blank { &blank } else { value };
    let digest = wire::domain_digest(BRANCH_DOMAIN, &blank, MAX_BRANCH_BYTES)?;
    wire::canonical_with_max(checked, MAX_BRANCH_BYTES)?;
    if !allow_blank && value.branch_sha256 != digest {
        return Err(invalid("branch_sha256 differs from canonical preimage"));
    }
    Ok(digest)
}

pub(super) fn derive_with_capsule(
    capsule: &DecisionCapsule,
) -> Result<EvaluationBranch, DecisionCapsuleContractError> {
    let mut branch = candidate_validated(capsule);
    let digest = wire::domain_digest(BRANCH_DOMAIN, &branch, MAX_BRANCH_BYTES)?;
    branch.branch_id = format!("{BRANCH_PREFIX}{digest}");
    branch.branch_sha256 = digest;
    wire::canonical_with_max(&branch, MAX_BRANCH_BYTES)?;
    shape(&branch, false)?;
    Ok(branch)
}

/// Computes a branch digest after validating the exact sealed capsule dependency.
///
/// # Errors
///
/// Returns an error for an invalid capsule, comparison, identity, or document ceiling.
pub fn evaluation_branch_digest(
    value: &EvaluationBranch,
    decision_capsule: &DecisionCapsule,
) -> Result<String, DecisionCapsuleContractError> {
    body_shape(value)?;
    capsule::local_shape(decision_capsule, false)?;
    wire::premeasure_only(value, MAX_BRANCH_BYTES)?;
    require_comparison(value, decision_capsule)?;
    capsule::validated(decision_capsule)?;
    validate_with_capsule(value, decision_capsule, true)
}

/// Validates a branch against the one exact supplied sealed `DecisionCapsule`.
///
/// # Errors
///
/// Returns an error for an invalid capsule, comparison, identity, or document ceiling.
pub fn validate_evaluation_branch(
    value: &EvaluationBranch,
    decision_capsule: &DecisionCapsule,
) -> Result<(), DecisionCapsuleContractError> {
    shape(value, false)?;
    capsule::local_shape(decision_capsule, false)?;
    wire::premeasure_only(value, MAX_BRANCH_BYTES)?;
    require_comparison(value, decision_capsule)?;
    capsule::validated(decision_capsule)?;
    validate_with_capsule(value, decision_capsule, false).map(|_| ())
}

/// Seals a blank branch only when it is the unique comparison for its capsule.
///
/// # Errors
///
/// Returns an error for a nonblank identity, invalid capsule or comparison, or oversize.
pub fn seal_evaluation_branch(
    value: &EvaluationBranch,
    decision_capsule: &DecisionCapsule,
) -> Result<EvaluationBranch, DecisionCapsuleContractError> {
    if !value.branch_id.is_empty() || !value.branch_sha256.is_empty() {
        return Err(invalid("sealing branch requires blank own identity"));
    }
    shape(value, true)?;
    capsule::local_shape(decision_capsule, false)?;
    wire::premeasure_only(value, MAX_BRANCH_BYTES)?;
    require_comparison(value, decision_capsule)?;
    capsule::validated(decision_capsule)?;
    let digest = validate_with_capsule(value, decision_capsule, true)?;
    let mut sealed = wire::bounded_clone(value, MAX_BRANCH_BYTES)?;
    sealed.branch_id = format!("{BRANCH_PREFIX}{digest}");
    sealed.branch_sha256 = digest;
    wire::canonical_with_max(&sealed, MAX_BRANCH_BYTES)?;
    shape(&sealed, false)?;
    Ok(sealed)
}

/// Derives and seals the one comparison branch for a sealed `DecisionCapsule`.
///
/// # Errors
///
/// Returns an error when the capsule is invalid or the derived branch exceeds its ceiling.
pub fn derive_evaluation_branch(
    decision_capsule: &DecisionCapsule,
) -> Result<EvaluationBranch, DecisionCapsuleContractError> {
    capsule::local_shape(decision_capsule, false)?;
    capsule::validated(decision_capsule)?;
    derive_with_capsule(decision_capsule)
}

/// Decodes exact canonical branch bytes and validates the sealed capsule dependency.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or dependency-inconsistent bytes.
pub fn decode_evaluation_branch(
    bytes: &[u8],
    decision_capsule: &DecisionCapsule,
) -> Result<EvaluationBranch, DecisionCapsuleContractError> {
    let value = wire::decode_typed(bytes, MAX_BRANCH_BYTES)?;
    validate_evaluation_branch(&value, decision_capsule)?;
    Ok(value)
}
