use crate::governance_contract::{ClaimObjectType, ClaimObjectValue, ClaimState};

use super::{
    API_VERSION, CANONICALIZATION, CognitiveAtom, CognitiveAtomContractError, Hardness,
    MAX_ATOM_BYTES, MAX_ATOM_SET_BYTES, MAX_ATOMS, ProjectionMode, invalid,
};

pub(super) fn validate_atom(atom: &CognitiveAtom) -> Result<(), CognitiveAtomContractError> {
    validate_header_and_identity(atom)?;
    validate_source(atom)?;
    validate_shadow_spec(atom)?;
    validate_proposition_and_validity(atom)?;
    validate_seal(atom)
}

fn validate_header_and_identity(atom: &CognitiveAtom) -> Result<(), CognitiveAtomContractError> {
    if atom.api_version != API_VERSION
        || atom.integrity.canonicalization != CANONICALIZATION
        || !digest(&atom.integrity.canonical_sha256)
    {
        return Err(invalid("unsupported API, canonicalization, or digest"));
    }
    if !atom.metadata.atom_id.starts_with("atom-")
        || atom.metadata.atom_id.len() != 69
        || !digest(&atom.metadata.atom_id[5..])
    {
        return Err(invalid("atom_id must be atom- plus a lowercase SHA-256"));
    }
    for value in [
        &atom.metadata.project_id,
        &atom.metadata.scope,
        &atom.metadata.source_revision,
        &atom.metadata.task_id,
        &atom.source.claim_aggregate_id,
        &atom.source.claim_record_id,
    ] {
        if !identifier(value) {
            return Err(invalid("CognitiveAtom contains an invalid identifier"));
        }
    }
    for value in [
        &atom.metadata.context_sha256,
        &atom.metadata.policy_sha256,
        &atom.metadata.source_tree_sha256,
        &atom.source.canonical_sha256,
        &atom.source.closure_sha256,
    ] {
        if !digest(value) {
            return Err(invalid("CognitiveAtom contains an invalid SHA-256"));
        }
    }
    Ok(())
}

fn validate_source(atom: &CognitiveAtom) -> Result<(), CognitiveAtomContractError> {
    if atom.source.record_kind != "KnowledgeClaim"
        || atom.source.claim_sequence < 1
        || !(1..=i64::try_from(MAX_ATOMS).expect("limit fits i64"))
            .contains(&atom.source.closure_record_count)
        || !(1..=i64::try_from(MAX_ATOM_SET_BYTES).expect("limit fits i64"))
            .contains(&atom.source.closure_byte_count)
    {
        return Err(invalid(
            "CognitiveAtom source identity or closure limits are invalid",
        ));
    }
    Ok(())
}

fn validate_shadow_spec(atom: &CognitiveAtom) -> Result<(), CognitiveAtomContractError> {
    if atom.spec.authority_ref.is_some()
        || atom.spec.hardness != Hardness::None
        || atom.spec.instruction_allowed
        || atom.spec.projection_mode != ProjectionMode::Shadow
    {
        return Err(invalid(
            "CognitiveAtom v1 must remain authority-free shadow projection",
        ));
    }
    validate_sorted(&atom.spec.supporting_evidence_record_ids)?;
    validate_sorted(&atom.spec.contradicting_evidence_record_ids)?;
    validate_sorted(&atom.spec.derived_from_claim_record_ids)?;
    if atom
        .spec
        .supporting_evidence_record_ids
        .iter()
        .any(|value| {
            atom.spec
                .contradicting_evidence_record_ids
                .binary_search(value)
                .is_ok()
        })
    {
        return Err(invalid(
            "supporting and contradicting evidence must be disjoint",
        ));
    }
    if !state_allowed(atom) {
        return Err(invalid("epistemic_state is not admissible for atom_type"));
    }
    let uncertain = matches!(
        atom.spec.atom_type,
        super::AtomType::Assumption | super::AtomType::Hypothesis | super::AtomType::Inference
    );
    match atom.spec.projection_confidence_micros {
        Some(value) if uncertain && (0..=1_000_000).contains(&value) => {}
        None if !uncertain => {}
        _ => return Err(invalid("projection confidence does not match atom_type")),
    }
    Ok(())
}

fn validate_proposition_and_validity(
    atom: &CognitiveAtom,
) -> Result<(), CognitiveAtomContractError> {
    if !object_matches(
        atom.spec.proposition.object_type,
        &atom.spec.proposition.object_value,
    ) || !identifier(&atom.spec.proposition.subject)
        || !identifier(&atom.spec.proposition.predicate)
    {
        return Err(invalid("CognitiveAtom proposition is invalid"));
    }
    if atom.spec.validity.valid_from_unix_ms < 0
        || atom
            .spec
            .validity
            .valid_until_unix_ms
            .is_some_and(|until| until <= atom.spec.validity.valid_from_unix_ms)
    {
        return Err(invalid("CognitiveAtom validity interval is invalid"));
    }
    Ok(())
}

fn validate_seal(atom: &CognitiveAtom) -> Result<(), CognitiveAtomContractError> {
    let canonical = super::canonical_atom_json(atom)?;
    if canonical.len() > MAX_ATOM_BYTES {
        return Err(invalid("CognitiveAtom exceeds the canonical byte limit"));
    }
    if atom.integrity.canonical_sha256 != super::expected_atom_sha256(atom)? {
        return Err(invalid("CognitiveAtom canonical_sha256 mismatch"));
    }
    if atom.metadata.atom_id != super::expected_atom_id(atom)? {
        return Err(invalid("CognitiveAtom atom_id mismatch"));
    }
    Ok(())
}

pub(super) fn validate_atom_set(atoms: &[CognitiveAtom]) -> Result<(), CognitiveAtomContractError> {
    if atoms.is_empty() || atoms.len() > MAX_ATOMS {
        return Err(invalid("CognitiveAtom set must contain 1..256 atoms"));
    }
    for atom in atoms {
        validate_atom(atom)?;
    }
    if atoms
        .windows(2)
        .any(|pair| pair[0].metadata.atom_id >= pair[1].metadata.atom_id)
    {
        return Err(invalid(
            "CognitiveAtom set must be sorted by unique atom_id",
        ));
    }
    if super::canonical_atom_set_json(atoms)?.len() > MAX_ATOM_SET_BYTES {
        return Err(invalid(
            "CognitiveAtom set exceeds the canonical byte limit",
        ));
    }
    Ok(())
}

fn state_allowed(atom: &CognitiveAtom) -> bool {
    use super::AtomType;
    matches!(
        (atom.spec.atom_type, atom.spec.epistemic_state),
        (
            AtomType::Fact,
            ClaimState::Candidate | ClaimState::Contested
        ) | (
            AtomType::Constraint | AtomType::Inference,
            ClaimState::Candidate
        ) | (AtomType::Decision, ClaimState::Proposed)
            | (
                AtomType::Assumption | AtomType::Hypothesis,
                ClaimState::Open | ClaimState::Testing
            )
            | (
                AtomType::Unknown,
                ClaimState::Open | ClaimState::Investigating
            )
    )
}

fn object_matches(kind: ClaimObjectType, value: &ClaimObjectValue) -> bool {
    match (kind, value) {
        (ClaimObjectType::ArtifactRef, ClaimObjectValue::String(value)) => identifier(value),
        (ClaimObjectType::String, ClaimObjectValue::String(_))
        | (ClaimObjectType::Boolean, ClaimObjectValue::Boolean(_))
        | (ClaimObjectType::Integer, ClaimObjectValue::Integer(_))
        | (ClaimObjectType::Null, ClaimObjectValue::Null) => true,
        _ => false,
    }
}

fn validate_sorted(values: &[String]) -> Result<(), CognitiveAtomContractError> {
    if values.iter().any(|value| !identifier(value))
        || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "reference arrays must contain sorted unique identifiers",
        ));
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    exact_identifier(value)
}

pub(super) fn exact_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=160).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

pub(super) fn decode_hash(value: &str) -> Result<[u8; 32], CognitiveAtomContractError> {
    if !digest(value) {
        return Err(invalid("invalid lowercase SHA-256"));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    Ok(output)
}

fn nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("digest was validated"),
    }
}
