use crate::kernel_decision_contract::{AtomRef, KernelDecisionReferenceClosure};
use std::collections::HashSet;

use super::{
    CANONICALIZATION, DecisionCapsuleContractError, MANIFEST_API, MANIFEST_DOMAIN, MANIFEST_KIND,
    MANIFEST_MODE, MANIFEST_PREFIX, MAX_MANIFEST_BYTES, StructuralReplayManifest, invalid,
    primitives, wire,
};

fn header_projection_matches(
    value: &StructuralReplayManifest,
    closure: &KernelDecisionReferenceClosure,
) -> bool {
    let operational = &closure.operational_closure;
    let transaction = &closure.decision_transaction;
    value.decision_closure_ref.closure_id == closure.closure_id
        && value.decision_closure_ref.closure_sha256 == closure.closure_sha256
        && value.decision_transaction_ref.decision_transaction_id
            == transaction.decision_transaction_id
        && value.decision_transaction_ref.decision_transaction_sha256
            == transaction.decision_transaction_sha256
        && value.operational_closure_ref.closure_id == operational.closure_id
        && value.operational_closure_ref.closure_sha256 == operational.closure_sha256
}

fn operational_projection_matches(
    value: &StructuralReplayManifest,
    closure: &KernelDecisionReferenceClosure,
) -> bool {
    let operational = &closure.operational_closure;
    let receipts = value
        .artifact_receipt_refs
        .iter()
        .map(|item| (&item.artifact_receipt_id, &item.artifact_receipt_sha256));
    let expected_receipts = operational
        .artifact_receipts
        .iter()
        .map(|item| (&item.artifact_receipt_id, &item.artifact_receipt_sha256));
    let invocations = value
        .capability_invocation_refs
        .iter()
        .map(|item| (&item.invocation_id, &item.invocation_sha256));
    let expected_invocations = operational
        .capability_invocations
        .iter()
        .map(|item| (&item.invocation_id, &item.invocation_sha256));
    value.artifact_refs == operational.artifacts
        && receipts.eq(expected_receipts)
        && invocations.eq(expected_invocations)
        && event_receipt_projection_matches(value, closure)
}

fn event_receipt_projection_matches(
    value: &StructuralReplayManifest,
    closure: &KernelDecisionReferenceClosure,
) -> bool {
    let operational = &closure.operational_closure;
    let events = value
        .interaction_event_refs
        .iter()
        .map(|item| (&item.event_id, &item.event_sha256));
    let expected_events = operational
        .interaction_events
        .iter()
        .map(|item| (&item.event_id, &item.event_sha256));
    let executions = value
        .execution_receipt_refs
        .iter()
        .map(|item| (&item.execution_receipt_id, &item.execution_receipt_sha256));
    let expected_executions = operational
        .execution_receipts
        .iter()
        .map(|item| (&item.execution_receipt_id, &item.execution_receipt_sha256));
    events.eq(expected_events) && executions.eq(expected_executions)
}

fn atom_projection_matches(
    actual: &[AtomRef],
    closure: &KernelDecisionReferenceClosure,
    phase: &str,
) -> bool {
    let actual = actual.iter().map(|item| (&item.atom_id, &item.atom_sha256));
    let expected = closure
        .cognitive_atoms
        .iter()
        .filter(|atom| atom.source.source_phase == phase)
        .map(|item| (&item.atom_id, &item.atom_sha256));
    actual.eq(expected)
}

fn projection_matches(
    value: &StructuralReplayManifest,
    closure: &KernelDecisionReferenceClosure,
) -> bool {
    header_projection_matches(value, closure)
        && operational_projection_matches(value, closure)
        && atom_projection_matches(&value.predecision_atom_refs, closure, "predecision")
        && atom_projection_matches(&value.postdecision_atom_refs, closure, "postdecision")
}

pub(super) fn require_projection(
    value: &StructuralReplayManifest,
    closure: &KernelDecisionReferenceClosure,
) -> Result<(), DecisionCapsuleContractError> {
    if projection_matches(value, closure) {
        Ok(())
    } else {
        Err(invalid(
            "manifest is not the exact ordered closure projection",
        ))
    }
}

fn references<'a>(
    values: impl IntoIterator<Item = (&'a str, &'a str)>,
    prefix: &str,
    label: &str,
    sorted: bool,
) -> Result<(), DecisionCapsuleContractError> {
    let mut identities = Vec::new();
    for (identity, digest) in values {
        primitives::identity(identity, digest, prefix, label, false)?;
        identities.push(identity);
    }
    let mut seen = HashSet::with_capacity(identities.len());
    if identities.iter().any(|identity| !seen.insert(*identity)) {
        return Err(invalid(format!("{label} must be unique")));
    }
    if sorted
        && !identities
            .windows(2)
            .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
    {
        return Err(invalid(format!("{label} must preserve identity order")));
    }
    Ok(())
}

fn validate_header_refs(
    value: &StructuralReplayManifest,
) -> Result<(), DecisionCapsuleContractError> {
    primitives::identity(
        &value.decision_closure_ref.closure_id,
        &value.decision_closure_ref.closure_sha256,
        "kernel-decision-reference-closure-",
        "decision_closure_ref",
        false,
    )?;
    primitives::identity(
        &value.decision_transaction_ref.decision_transaction_id,
        &value.decision_transaction_ref.decision_transaction_sha256,
        "decision-transaction-",
        "decision_transaction_ref",
        false,
    )?;
    primitives::identity(
        &value.operational_closure_ref.closure_id,
        &value.operational_closure_ref.closure_sha256,
        "kernel-operational-reference-closure-",
        "operational_closure_ref",
        false,
    )
}

fn validate_atom_refs(
    value: &StructuralReplayManifest,
) -> Result<(), DecisionCapsuleContractError> {
    let pre = value
        .predecision_atom_refs
        .iter()
        .map(|item| (item.atom_id.as_str(), item.atom_sha256.as_str()));
    references(pre, "cognitive-atom-", "predecision_atom_refs", true)?;
    let post = value
        .postdecision_atom_refs
        .iter()
        .map(|item| (item.atom_id.as_str(), item.atom_sha256.as_str()));
    references(post, "cognitive-atom-", "postdecision_atom_refs", true)?;
    let combined = value
        .predecision_atom_refs
        .iter()
        .chain(&value.postdecision_atom_refs)
        .map(|item| (item.atom_id.as_str(), item.atom_sha256.as_str()));
    references(combined, "cognitive-atom-", "combined atom refs", false)
}

fn validate_artifact_refs(
    value: &StructuralReplayManifest,
) -> Result<(), DecisionCapsuleContractError> {
    let mut encoded = Vec::with_capacity(value.artifact_refs.len());
    for artifact in &value.artifact_refs {
        primitives::validate_artifact(artifact)?;
        encoded.push(wire::canonical_with_max(artifact, MAX_MANIFEST_BYTES)?);
    }
    if encoded.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(invalid(
            "artifact_refs must be canonical-byte sorted and unique",
        ))
    }
}

fn validate_attempt_refs(
    value: &StructuralReplayManifest,
) -> Result<(), DecisionCapsuleContractError> {
    let receipts = value.artifact_receipt_refs.iter().map(|item| {
        (
            item.artifact_receipt_id.as_str(),
            item.artifact_receipt_sha256.as_str(),
        )
    });
    references(receipts, "artifact-receipt-", "artifact_receipt_refs", true)?;
    let invocations = value
        .capability_invocation_refs
        .iter()
        .map(|item| (item.invocation_id.as_str(), item.invocation_sha256.as_str()));
    references(
        invocations,
        "capability-invocation-",
        "capability_invocation_refs",
        false,
    )
}

fn validate_event_receipt_refs(
    value: &StructuralReplayManifest,
) -> Result<(), DecisionCapsuleContractError> {
    let events = value
        .interaction_event_refs
        .iter()
        .map(|item| (item.event_id.as_str(), item.event_sha256.as_str()));
    references(
        events,
        "interaction-event-",
        "interaction_event_refs",
        false,
    )?;
    let executions = value.execution_receipt_refs.iter().map(|item| {
        (
            item.execution_receipt_id.as_str(),
            item.execution_receipt_sha256.as_str(),
        )
    });
    references(
        executions,
        "execution-receipt-",
        "execution_receipt_refs",
        false,
    )
}

fn body_shape(value: &StructuralReplayManifest) -> Result<(), DecisionCapsuleContractError> {
    // Digest ignores the values of the two own identity fields, but their JSON
    // profile limits still precede local canonical-key work and all dependency
    // validation.
    wire::premeasure_only(
        &(&value.manifest_id, &value.manifest_sha256),
        MAX_MANIFEST_BYTES,
    )?;
    if value.api_version != MANIFEST_API
        || value.canonicalization != CANONICALIZATION
        || value.kind != MANIFEST_KIND
        || value.replay_mode != MANIFEST_MODE
    {
        return Err(invalid("StructuralReplayManifest constants differ"));
    }
    if value.effect_replay_allowed || value.history_rewrite_allowed {
        return Err(invalid("manifest replay controls must be false"));
    }
    primitives::attestations(&value.attestations)?;
    let atoms = value.predecision_atom_refs.len() + value.postdecision_atom_refs.len();
    if value.predecision_atom_refs.is_empty()
        || atoms > 256
        || value.artifact_refs.len() > 256
        || value.artifact_receipt_refs.len() > 64
        || !(1..=64).contains(&value.capability_invocation_refs.len())
        || value.interaction_event_refs.len() > 256
        || !(1..=64).contains(&value.execution_receipt_refs.len())
    {
        return Err(invalid("manifest inventory cardinality differs"));
    }
    validate_header_refs(value)?;
    validate_atom_refs(value)?;
    validate_artifact_refs(value)?;
    validate_attempt_refs(value)?;
    validate_event_receipt_refs(value)
}

fn shape(
    value: &StructuralReplayManifest,
    allow_blank: bool,
) -> Result<(), DecisionCapsuleContractError> {
    body_shape(value)?;
    primitives::identity(
        &value.manifest_id,
        &value.manifest_sha256,
        MANIFEST_PREFIX,
        "manifest",
        allow_blank,
    )
}

pub(super) fn local_shape(
    value: &StructuralReplayManifest,
    allow_blank: bool,
) -> Result<(), DecisionCapsuleContractError> {
    shape(value, allow_blank)
}

fn blanked_after_validation(
    value: &StructuralReplayManifest,
) -> Result<StructuralReplayManifest, DecisionCapsuleContractError> {
    wire::premeasure_only(value, MAX_MANIFEST_BYTES)?;
    let mut blank = wire::bounded_clone(value, MAX_MANIFEST_BYTES)?;
    blank.manifest_id.clear();
    blank.manifest_sha256.clear();
    Ok(blank)
}

pub(super) fn validate_with_closure(
    value: &StructuralReplayManifest,
    closure: &KernelDecisionReferenceClosure,
    allow_blank: bool,
) -> Result<String, DecisionCapsuleContractError> {
    body_shape(value)?;
    wire::premeasure_only(value, MAX_MANIFEST_BYTES)?;
    if !allow_blank {
        primitives::identity(
            &value.manifest_id,
            &value.manifest_sha256,
            MANIFEST_PREFIX,
            "manifest",
            false,
        )?;
    }
    require_projection(value, closure)?;
    let blank = blanked_after_validation(value)?;
    let checked = if allow_blank { &blank } else { value };
    let digest = wire::domain_digest(MANIFEST_DOMAIN, &blank, MAX_MANIFEST_BYTES)?;
    wire::canonical_with_max(checked, MAX_MANIFEST_BYTES)?;
    if !allow_blank && value.manifest_sha256 != digest {
        return Err(invalid("manifest_sha256 differs from canonical preimage"));
    }
    Ok(digest)
}

pub(super) fn derive_with_closure(
    closure: &KernelDecisionReferenceClosure,
) -> Result<StructuralReplayManifest, DecisionCapsuleContractError> {
    let mut manifest = primitives::project_manifest(closure);
    let digest = wire::domain_digest(MANIFEST_DOMAIN, &manifest, MAX_MANIFEST_BYTES)?;
    manifest.manifest_id = format!("{MANIFEST_PREFIX}{digest}");
    manifest.manifest_sha256 = digest;
    wire::canonical_with_max(&manifest, MAX_MANIFEST_BYTES)?;
    shape(&manifest, false)?;
    Ok(manifest)
}

/// Computes the manifest digest after validating its sealed ADR-0090 dependency.
///
/// # Errors
///
/// Returns an error for an invalid dependency, projection, identity, or document ceiling.
pub fn structural_replay_manifest_digest(
    value: &StructuralReplayManifest,
    decision_closure: &KernelDecisionReferenceClosure,
) -> Result<String, DecisionCapsuleContractError> {
    body_shape(value)?;
    wire::premeasure_only(value, MAX_MANIFEST_BYTES)?;
    require_projection(value, decision_closure)?;
    let closure = primitives::reseal_decision_closure(decision_closure)?;
    validate_with_closure(value, &closure, true)
}

/// Validates the exact manifest projection and its sealed ADR-0090 dependency.
///
/// # Errors
///
/// Returns an error for an invalid dependency, projection, identity, or document ceiling.
pub fn validate_structural_replay_manifest(
    value: &StructuralReplayManifest,
    decision_closure: &KernelDecisionReferenceClosure,
) -> Result<(), DecisionCapsuleContractError> {
    shape(value, false)?;
    wire::premeasure_only(value, MAX_MANIFEST_BYTES)?;
    require_projection(value, decision_closure)?;
    let closure = primitives::reseal_decision_closure(decision_closure)?;
    validate_with_closure(value, &closure, false).map(|_| ())
}

/// Seals a blank manifest only when it exactly projects the supplied ADR-0090 closure.
///
/// # Errors
///
/// Returns an error for a nonblank identity, invalid dependency or projection, or oversize.
pub fn seal_structural_replay_manifest(
    value: &StructuralReplayManifest,
    decision_closure: &KernelDecisionReferenceClosure,
) -> Result<StructuralReplayManifest, DecisionCapsuleContractError> {
    if !value.manifest_id.is_empty() || !value.manifest_sha256.is_empty() {
        return Err(invalid("sealing manifest requires blank own identity"));
    }
    shape(value, true)?;
    wire::premeasure_only(value, MAX_MANIFEST_BYTES)?;
    require_projection(value, decision_closure)?;
    let closure = primitives::reseal_decision_closure(decision_closure)?;
    let digest = validate_with_closure(value, &closure, true)?;
    let mut sealed = wire::bounded_clone(value, MAX_MANIFEST_BYTES)?;
    sealed.manifest_id = format!("{MANIFEST_PREFIX}{digest}");
    sealed.manifest_sha256 = digest;
    wire::canonical_with_max(&sealed, MAX_MANIFEST_BYTES)?;
    shape(&sealed, false)?;
    Ok(sealed)
}

/// Derives and seals the only complete manifest for a sealed ADR-0090 closure.
///
/// # Errors
///
/// Returns an error when the dependency is invalid or its projection exceeds the ceiling.
pub fn derive_structural_replay_manifest(
    decision_closure: &KernelDecisionReferenceClosure,
) -> Result<StructuralReplayManifest, DecisionCapsuleContractError> {
    let closure = primitives::reseal_decision_closure(decision_closure)?;
    derive_with_closure(&closure)
}

/// Decodes canonical manifest bytes and validates them against a sealed ADR-0090 closure.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, or dependency-inconsistent bytes.
pub fn decode_structural_replay_manifest(
    bytes: &[u8],
    decision_closure: &KernelDecisionReferenceClosure,
) -> Result<StructuralReplayManifest, DecisionCapsuleContractError> {
    let value = wire::decode_typed(bytes, MAX_MANIFEST_BYTES)?;
    validate_structural_replay_manifest(&value, decision_closure)?;
    Ok(value)
}
