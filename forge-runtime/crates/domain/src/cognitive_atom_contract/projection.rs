use std::collections::{BTreeMap, BTreeSet};

use serde_json;
use sha2::{Digest, Sha256};

use crate::governance_contract::{
    ClaimType, GovernanceRecord, Integrity, KnowledgeClaim, decode_canonical_record_set,
};

use super::{
    API_VERSION, ATOM_DIGEST_DOMAIN, ATOM_ID_DOMAIN, ATOM_SET_DIGEST_DOMAIN, AtomMetadata,
    AtomSource, AtomType, CANONICALIZATION, CognitiveAtom, CognitiveAtomContractError,
    CognitiveAtomKind, CognitiveAtomProjection, CognitiveAtomSpec, Hardness, MAX_ATOM_BYTES,
    MAX_ATOM_SET_BYTES, ProjectionMode, Proposition, SOURCE_CLOSURE_DIGEST_DOMAIN, Validity,
    invalid,
};
use crate::governance_contract::codec::{canonical_json, canonical_record_set_json, lower_hex};

/// Projects an exact canonical shadow `GovernanceRecordSet` into `CognitiveAtom` v1.
///
/// # Errors
///
/// Returns an error for invalid source bytes, a non-projectable source, or any
/// identity, closure, canonicalization, or size violation.
pub fn project_canonical_record_set(
    source_bytes: &[u8],
    task_id: &str,
) -> Result<CognitiveAtomProjection, CognitiveAtomContractError> {
    if !super::validation::exact_identifier(task_id) {
        return Err(invalid("task_id must be a bounded identifier"));
    }
    let records = decode_canonical_record_set(source_bytes)
        .map_err(|error| invalid(format!("source GovernanceRecordSet: {error}")))?;
    let index: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.metadata().record_id.clone(), record))
        .collect();
    let mut atoms = Vec::new();
    for record in &records {
        if let GovernanceRecord::Claim(claim) = record
            && let Some(atom_type) = projectable_type(claim.spec.claim_type)
        {
            atoms.push(project_claim(claim, atom_type, task_id, &index)?);
        }
    }
    if atoms.is_empty() {
        return Err(invalid(
            "source GovernanceRecordSet has no projectable KnowledgeClaim",
        ));
    }
    atoms.sort_unstable_by(|left, right| left.metadata.atom_id.cmp(&right.metadata.atom_id));
    super::validation::validate_atom_set(&atoms)?;
    let canonical_atom_set_json = canonical_atom_set_json(&atoms)?;
    let atom_set_sha256 = digest_hex(ATOM_SET_DIGEST_DOMAIN, canonical_atom_set_json.as_bytes());
    Ok(CognitiveAtomProjection {
        atom_set_sha256,
        atoms,
        canonical_atom_set_json,
    })
}

/// Reprojects the canonical source and requires byte-for-byte Atom set parity.
///
/// # Errors
///
/// Returns an error when either input is invalid or the supplied Atom set is not
/// the exact deterministic projection of the source and task identity.
pub fn validate_projection(
    source_bytes: &[u8],
    task_id: &str,
    atom_set_bytes: &[u8],
) -> Result<CognitiveAtomProjection, CognitiveAtomContractError> {
    let decoded = decode_canonical_atom_set(atom_set_bytes)?;
    let projected = project_canonical_record_set(source_bytes, task_id)?;
    if decoded != projected.atoms || atom_set_bytes != projected.canonical_atom_set_json.as_bytes()
    {
        return Err(invalid(
            "CognitiveAtom set is not the exact deterministic source projection",
        ));
    }
    Ok(projected)
}

/// Decodes and validates one exact compact canonical `CognitiveAtom` set.
///
/// # Errors
///
/// Returns an error for malformed, noncanonical, oversized, unsorted, duplicated,
/// semantically invalid, or incorrectly sealed Atom data.
pub fn decode_canonical_atom_set(
    bytes: &[u8],
) -> Result<Vec<CognitiveAtom>, CognitiveAtomContractError> {
    if bytes.len() > MAX_ATOM_SET_BYTES {
        return Err(invalid(
            "CognitiveAtom set exceeds the canonical byte limit",
        ));
    }
    let atoms: Vec<CognitiveAtom> = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("CognitiveAtom set is invalid JSON: {error}")))?;
    super::validation::validate_atom_set(&atoms)?;
    let canonical = canonical_atom_set_json(&atoms)?;
    if bytes != canonical.as_bytes() {
        return Err(invalid(
            "input is not exact compact canonical JSON for CognitiveAtom set",
        ));
    }
    Ok(atoms)
}

/// Encodes one Atom with the frozen canonical JSON rules.
///
/// # Errors
///
/// Returns an error if a canonical JSON limit is exceeded.
pub fn canonical_atom_json(atom: &CognitiveAtom) -> Result<String, CognitiveAtomContractError> {
    let canonical = canonical_json(atom).map_err(|error| invalid(error.to_string()))?;
    if canonical.len() > MAX_ATOM_BYTES {
        return Err(invalid("CognitiveAtom exceeds the canonical byte limit"));
    }
    Ok(canonical)
}

/// Encodes the digest payload with only the self digest blanked.
///
/// # Errors
///
/// Returns an error if a canonical JSON or sealed-size limit is exceeded.
pub fn canonical_atom_payload_json(
    atom: &CognitiveAtom,
) -> Result<String, CognitiveAtomContractError> {
    let mut payload = atom.clone();
    payload.integrity.canonical_sha256.clear();
    let canonical = canonical_atom_json(&payload)?;
    if canonical.len() + 64 > MAX_ATOM_BYTES {
        return Err(invalid(
            "sealed CognitiveAtom exceeds the canonical byte limit",
        ));
    }
    Ok(canonical)
}

/// Computes the domain-separated digest of an Atom payload.
///
/// # Errors
///
/// Returns an error if the payload cannot be canonically encoded.
pub fn expected_atom_sha256(atom: &CognitiveAtom) -> Result<String, CognitiveAtomContractError> {
    Ok(digest_hex(
        ATOM_DIGEST_DOMAIN,
        canonical_atom_payload_json(atom)?.as_bytes(),
    ))
}

/// Recomputes the length-framed identity from embedded source and task bindings.
///
/// # Errors
///
/// Returns an error for invalid digest text or an unrepresentable frame length.
pub fn expected_atom_id(atom: &CognitiveAtom) -> Result<String, CognitiveAtomContractError> {
    let task = atom.metadata.task_id.as_bytes();
    let revision = atom.metadata.source_revision.as_bytes();
    let mut hasher = Sha256::new();
    hasher.update(ATOM_ID_DOMAIN);
    hasher.update(
        u64::try_from(task.len())
            .map_err(|_| invalid("task_id length cannot be framed"))?
            .to_be_bytes(),
    );
    hasher.update(task);
    for value in [
        &atom.source.canonical_sha256,
        &atom.metadata.context_sha256,
        &atom.metadata.policy_sha256,
        &atom.metadata.source_tree_sha256,
    ] {
        hasher.update(super::validation::decode_hash(value)?);
    }
    hasher.update(
        u64::try_from(revision.len())
            .map_err(|_| invalid("source_revision length cannot be framed"))?
            .to_be_bytes(),
    );
    hasher.update(revision);
    Ok(format!("atom-{}", lower_hex(&hasher.finalize())))
}

/// Encodes an Atom slice using the frozen canonical JSON rules.
///
/// # Errors
///
/// Returns an error when canonicalization or the Atom-set size limit fails.
pub fn canonical_atom_set_json(
    atoms: &[CognitiveAtom],
) -> Result<String, CognitiveAtomContractError> {
    let canonical = canonical_json(atoms).map_err(|error| invalid(error.to_string()))?;
    if canonical.len() > MAX_ATOM_SET_BYTES {
        return Err(invalid(
            "CognitiveAtom set exceeds the canonical byte limit",
        ));
    }
    Ok(canonical)
}

/// Computes the domain-separated digest of canonical Atom-set bytes.
///
/// # Errors
///
/// Returns an error when the Atom set cannot be canonically encoded.
pub fn cognitive_atom_set_sha256(
    atoms: &[CognitiveAtom],
) -> Result<String, CognitiveAtomContractError> {
    Ok(digest_hex(
        ATOM_SET_DIGEST_DOMAIN,
        canonical_atom_set_json(atoms)?.as_bytes(),
    ))
}

fn project_claim(
    claim: &KnowledgeClaim,
    atom_type: AtomType,
    task_id: &str,
    index: &BTreeMap<String, &GovernanceRecord>,
) -> Result<CognitiveAtom, CognitiveAtomContractError> {
    let mut atom = CognitiveAtom {
        api_version: API_VERSION.to_owned(),
        integrity: Integrity {
            canonical_sha256: String::new(),
            canonicalization: CANONICALIZATION.to_owned(),
        },
        kind: CognitiveAtomKind::CognitiveAtom,
        metadata: project_metadata(claim, task_id),
        source: project_source(claim, index)?,
        spec: project_spec(claim, atom_type),
    };
    atom.metadata.atom_id = expected_atom_id(&atom)?;
    atom.integrity.canonical_sha256 = expected_atom_sha256(&atom)?;
    super::validation::validate_atom(&atom)?;
    Ok(atom)
}

fn project_metadata(claim: &KnowledgeClaim, task_id: &str) -> AtomMetadata {
    let metadata = &claim.metadata;
    AtomMetadata {
        atom_id: String::new(),
        context_sha256: metadata.context_sha256.clone(),
        policy_sha256: metadata.policy_sha256.clone(),
        project_id: metadata.project_id.clone(),
        scope: metadata.scope.clone(),
        source_revision: metadata.source_revision.clone(),
        source_tree_sha256: metadata.source_tree_sha256.clone(),
        task_id: task_id.to_owned(),
    }
}

fn project_source(
    claim: &KnowledgeClaim,
    index: &BTreeMap<String, &GovernanceRecord>,
) -> Result<AtomSource, CognitiveAtomContractError> {
    let closure = source_closure(&claim.metadata.record_id, index)?;
    let closure_json = canonical_record_set_json(&closure)
        .map_err(|error| invalid(format!("source closure: {error}")))?;
    Ok(AtomSource {
        canonical_sha256: claim.integrity.canonical_sha256.clone(),
        claim_aggregate_id: claim.metadata.aggregate_id.clone(),
        claim_record_id: claim.metadata.record_id.clone(),
        claim_sequence: claim.metadata.sequence,
        closure_byte_count: i64::try_from(closure_json.len())
            .map_err(|_| invalid("source closure byte count cannot be represented"))?,
        closure_record_count: i64::try_from(closure.len())
            .map_err(|_| invalid("source closure count cannot be represented"))?,
        closure_sha256: digest_hex(SOURCE_CLOSURE_DIGEST_DOMAIN, closure_json.as_bytes()),
        record_kind: "KnowledgeClaim".to_owned(),
    })
}

fn project_spec(claim: &KnowledgeClaim, atom_type: AtomType) -> CognitiveAtomSpec {
    CognitiveAtomSpec {
        atom_type,
        authority_ref: None,
        contradicting_evidence_record_ids: claim.spec.contradicting_evidence_record_ids.clone(),
        derived_from_claim_record_ids: claim.spec.derived_from_claim_record_ids.clone(),
        epistemic_state: claim.status.state,
        hardness: Hardness::None,
        instruction_allowed: false,
        projection_confidence_micros: claim.spec.confidence_micros,
        projection_mode: ProjectionMode::Shadow,
        proposition: Proposition {
            object_type: claim.spec.object_type,
            object_value: claim.spec.object_value.clone(),
            predicate: claim.spec.predicate.clone(),
            subject: claim.spec.subject.clone(),
        },
        supporting_evidence_record_ids: claim.spec.supporting_evidence_record_ids.clone(),
        validity: Validity {
            valid_from_unix_ms: claim.status.valid_from_unix_ms,
            valid_until_unix_ms: claim.status.valid_until_unix_ms,
        },
    }
}

fn source_closure(
    source_id: &str,
    index: &BTreeMap<String, &GovernanceRecord>,
) -> Result<Vec<GovernanceRecord>, CognitiveAtomContractError> {
    let mut pending = vec![source_id.to_owned()];
    let mut included = BTreeSet::new();
    while let Some(record_id) = pending.pop() {
        if !included.insert(record_id.clone()) {
            continue;
        }
        let record = index
            .get(&record_id)
            .ok_or_else(|| invalid(format!("source closure has dangling record {record_id}")))?;
        pending.extend(record.metadata().supersedes_record_ids.iter().cloned());
        if let GovernanceRecord::Claim(claim) = record {
            pending.extend(claim.spec.supporting_evidence_record_ids.iter().cloned());
            pending.extend(claim.spec.contradicting_evidence_record_ids.iter().cloned());
            pending.extend(claim.spec.derived_from_claim_record_ids.iter().cloned());
        }
    }
    Ok(included
        .into_iter()
        .map(|record_id| index[&record_id].clone())
        .collect())
}

fn projectable_type(claim_type: ClaimType) -> Option<AtomType> {
    match claim_type {
        ClaimType::Assumption => Some(AtomType::Assumption),
        ClaimType::Constraint => Some(AtomType::Constraint),
        ClaimType::Decision => Some(AtomType::Decision),
        ClaimType::Fact => Some(AtomType::Fact),
        ClaimType::Hypothesis => Some(AtomType::Hypothesis),
        ClaimType::Inference => Some(AtomType::Inference),
        ClaimType::Unknown => Some(AtomType::Unknown),
        ClaimType::Lesson | ClaimType::Proposal => None,
    }
}

fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    lower_hex(&hasher.finalize())
}
