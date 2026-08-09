use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rusqlite::Connection;

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    GovernanceStructuralHead, HubStoreError, MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES,
    MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS, MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH,
};

use super::{error, rows, stored};

#[derive(Clone, Copy)]
enum ReferenceOrigin {
    Candidate,
    Stored,
}

#[derive(Clone, Copy)]
enum LoadClassification {
    Candidate,
    Stored,
}

struct PendingReference {
    id: String,
    origin: ReferenceOrigin,
    follow_derivations: bool,
}

pub(super) fn load(
    connection: &Connection,
    candidates: &[GovernanceRecord],
    heads: &[GovernanceStructuralHead],
) -> Result<Vec<GovernanceRecord>, HubStoreError> {
    load_as(connection, candidates, heads, LoadClassification::Candidate)
}

pub(super) fn load_stored(
    connection: &Connection,
    candidates: &[GovernanceRecord],
    heads: &[GovernanceStructuralHead],
) -> Result<Vec<GovernanceRecord>, HubStoreError> {
    load_as(connection, candidates, heads, LoadClassification::Stored)
}

fn load_as(
    connection: &Connection,
    candidates: &[GovernanceRecord],
    heads: &[GovernanceStructuralHead],
    classification: LoadClassification,
) -> Result<Vec<GovernanceRecord>, HubStoreError> {
    let candidate_ids = candidate_ids(candidates);
    let mut pending = direct_candidate_references(candidates, classification);
    pending.extend(heads.iter().map(|head| PendingReference {
        id: head.record_id.clone(),
        origin: ReferenceOrigin::Stored,
        follow_derivations: false,
    }));
    load_pending(
        connection,
        &candidate_ids,
        candidates,
        pending,
        classification,
    )
}

fn load_pending(
    connection: &Connection,
    candidate_ids: &BTreeSet<String>,
    candidates: &[GovernanceRecord],
    mut pending: VecDeque<PendingReference>,
    classification: LoadClassification,
) -> Result<Vec<GovernanceRecord>, HubStoreError> {
    let mut loaded = BTreeMap::new();
    let mut expanded = BTreeSet::new();
    let mut validated_batches = BTreeSet::new();
    let mut bytes = candidate_bytes(candidates, classification)?;
    while let Some(reference) = pending.pop_front() {
        if candidate_ids.contains(&reference.id) {
            continue;
        }
        if let Some(record) = loaded.get(&reference.id) {
            expand_if_requested(record, &reference, &mut expanded, &mut pending);
            continue;
        }
        enforce_count(loaded.len() + 1, classification)?;
        let decoded = load_one(connection, &reference, &mut validated_batches)?;
        bytes = bytes
            .checked_add(decoded.inspection.metadata.canonical_record_bytes)
            .ok_or_else(|| {
                failure(
                    classification,
                    "governance dependency byte count overflowed",
                )
            })?;
        enforce_bytes(bytes, classification)?;
        expand_if_requested(&decoded.record, &reference, &mut expanded, &mut pending);
        loaded.insert(reference.id, decoded.record);
    }
    let dependencies: Vec<_> = loaded.into_values().collect();
    enforce_candidate_depths(candidates, &dependencies, classification)?;
    Ok(dependencies)
}

fn expand_if_requested(
    record: &GovernanceRecord,
    reference: &PendingReference,
    expanded: &mut BTreeSet<String>,
    pending: &mut VecDeque<PendingReference>,
) {
    if reference.follow_derivations && expanded.insert(reference.id.clone()) {
        enqueue_derivations(record, pending);
    }
}

fn direct_candidate_references(
    records: &[GovernanceRecord],
    classification: LoadClassification,
) -> VecDeque<PendingReference> {
    let mut pending = VecDeque::new();
    for record in records {
        for id in &record.metadata().supersedes_record_ids {
            pending.push_back(root_reference(id, classification, false));
        }
        if let GovernanceRecord::Claim(claim) = record {
            for id in claim
                .spec
                .supporting_evidence_record_ids
                .iter()
                .chain(&claim.spec.contradicting_evidence_record_ids)
            {
                pending.push_back(root_reference(id, classification, false));
            }
            for id in &claim.spec.derived_from_claim_record_ids {
                pending.push_back(root_reference(id, classification, true));
            }
        }
    }
    pending
}

fn enqueue_derivations(record: &GovernanceRecord, pending: &mut VecDeque<PendingReference>) {
    let GovernanceRecord::Claim(claim) = record else {
        return;
    };
    pending.extend(
        claim
            .spec
            .derived_from_claim_record_ids
            .iter()
            .map(|id| PendingReference {
                id: id.clone(),
                origin: ReferenceOrigin::Stored,
                follow_derivations: true,
            }),
    );
}

fn load_one(
    connection: &Connection,
    reference: &PendingReference,
    validated_batches: &mut BTreeSet<String>,
) -> Result<stored::DecodedRecord, HubStoreError> {
    let raw = rows::find_record(connection, &reference.id, true)?
        .ok_or_else(|| missing(reference.origin, &reference.id))?;
    super::write::validate_stored_batch_once(connection, &raw.batch_id, validated_batches)?;
    stored::decoded(raw)
}

fn missing(origin: ReferenceOrigin, id: &str) -> HubStoreError {
    match origin {
        ReferenceOrigin::Candidate => {
            error::conflict(format!("governance append reference '{id}' is unresolved"))
        }
        ReferenceOrigin::Stored => error::corrupt(format!(
            "stored governance derivation/head reference '{id}' is unresolved"
        )),
    }
}

fn root_reference(
    id: &str,
    classification: LoadClassification,
    follow_derivations: bool,
) -> PendingReference {
    PendingReference {
        id: id.into(),
        origin: match classification {
            LoadClassification::Candidate => ReferenceOrigin::Candidate,
            LoadClassification::Stored => ReferenceOrigin::Stored,
        },
        follow_derivations,
    }
}

fn candidate_ids(records: &[GovernanceRecord]) -> BTreeSet<String> {
    records
        .iter()
        .map(|record| record.metadata().record_id.clone())
        .collect()
}

fn candidate_bytes(
    records: &[GovernanceRecord],
    classification: LoadClassification,
) -> Result<usize, HubStoreError> {
    records.iter().try_fold(0_usize, |total, record| {
        let canonical = record
            .canonical_record_json()
            .map_err(|problem| failure(classification, &problem.message))?;
        total.checked_add(canonical.len()).ok_or_else(|| {
            failure(
                classification,
                "governance dependency byte count overflowed",
            )
        })
    })
}

fn enforce_count(count: usize, classification: LoadClassification) -> Result<(), HubStoreError> {
    if count > MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS {
        return Err(failure(
            classification,
            "governance dependency closure exceeds the record limit",
        ));
    }
    Ok(())
}

fn enforce_depth(depth: usize, classification: LoadClassification) -> Result<(), HubStoreError> {
    if depth > MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH {
        return Err(failure(
            classification,
            "governance derivation exceeds the reference depth limit",
        ));
    }
    Ok(())
}

fn enforce_candidate_depths(
    candidates: &[GovernanceRecord],
    dependencies: &[GovernanceRecord],
    classification: LoadClassification,
) -> Result<(), HubStoreError> {
    let mut records = BTreeMap::new();
    for record in candidates.iter().chain(dependencies) {
        let record_id = record.metadata().record_id.as_str();
        if records.insert(record_id, record).is_some() {
            return Err(failure(
                classification,
                "governance dependency closure contains a duplicate record ID",
            ));
        }
    }
    let mut depths = BTreeMap::new();
    for record in candidates {
        let mut visiting = BTreeSet::new();
        let depth = longest_derivation_path(
            record.metadata().record_id.as_str(),
            &records,
            &mut visiting,
            &mut depths,
            classification,
        )?;
        enforce_depth(depth, classification)?;
    }
    Ok(())
}

fn longest_derivation_path<'a>(
    record_id: &'a str,
    records: &BTreeMap<&'a str, &'a GovernanceRecord>,
    visiting: &mut BTreeSet<&'a str>,
    depths: &mut BTreeMap<&'a str, usize>,
    classification: LoadClassification,
) -> Result<usize, HubStoreError> {
    if let Some(depth) = depths.get(record_id) {
        return Ok(*depth);
    }
    if !visiting.insert(record_id) {
        return Err(failure(
            classification,
            "governance derivation graph contains a cycle",
        ));
    }
    let record = records.get(record_id).ok_or_else(|| {
        failure(
            classification,
            "governance derivation reference is unresolved",
        )
    })?;
    let longest = longest_child_path(record, records, visiting, depths, classification)?;
    visiting.remove(record_id);
    depths.insert(record_id, longest);
    Ok(longest)
}

fn longest_child_path<'a>(
    record: &'a GovernanceRecord,
    records: &BTreeMap<&'a str, &'a GovernanceRecord>,
    visiting: &mut BTreeSet<&'a str>,
    depths: &mut BTreeMap<&'a str, usize>,
    classification: LoadClassification,
) -> Result<usize, HubStoreError> {
    let GovernanceRecord::Claim(claim) = record else {
        return Ok(0);
    };
    let mut longest = 0;
    for dependency in &claim.spec.derived_from_claim_record_ids {
        let child = longest_derivation_path(dependency, records, visiting, depths, classification)?;
        longest =
            longest.max(child.checked_add(1).ok_or_else(|| {
                failure(classification, "governance derivation depth overflowed")
            })?);
    }
    Ok(longest)
}

fn enforce_bytes(bytes: usize, classification: LoadClassification) -> Result<(), HubStoreError> {
    if bytes > MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES {
        Err(failure(
            classification,
            "governance dependency closure exceeds the byte limit",
        ))
    } else {
        Ok(())
    }
}

fn failure(classification: LoadClassification, message: &str) -> HubStoreError {
    match classification {
        LoadClassification::Candidate => error::conflict(message),
        LoadClassification::Stored => error::corrupt(message),
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    #[test]
    fn candidate_dependency_record_limit_is_inclusive() {
        assert!(
            enforce_count(
                MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS,
                LoadClassification::Candidate,
            )
            .is_ok()
        );
        assert_conflict(&enforce_count(
            MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS + 1,
            LoadClassification::Candidate,
        ));
    }

    #[test]
    fn candidate_derivation_depth_limit_is_inclusive() {
        assert!(
            enforce_depth(
                MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH,
                LoadClassification::Candidate,
            )
            .is_ok()
        );
        assert_conflict(&enforce_depth(
            MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH + 1,
            LoadClassification::Candidate,
        ));
    }

    #[test]
    fn candidate_dependency_byte_limit_is_inclusive() {
        assert!(
            enforce_bytes(
                MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES,
                LoadClassification::Candidate,
            )
            .is_ok()
        );
        assert_conflict(&enforce_bytes(
            MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES + 1,
            LoadClassification::Candidate,
        ));
    }

    fn assert_conflict(result: &Result<(), HubStoreError>) {
        assert!(
            matches!(result, Err(HubStoreError::Conflict { .. })),
            "{result:?}"
        );
    }
}
