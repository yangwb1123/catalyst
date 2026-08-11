use std::collections::{BTreeMap, BTreeSet};

use crate::governance_contract::GovernanceRecord;

use super::{
    AppendGovernanceRecordBatch, GOVERNANCE_RECORD_JOURNAL_VERSION, GovernanceRecordJournalError,
    GovernanceRecordKind, GovernanceStructuralHead, MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES,
    MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS, MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH, invalid,
};

type RecordIndex<'a> = BTreeMap<&'a str, &'a GovernanceRecord>;
type AggregateKey = (GovernanceRecordKind, String);

/// Validates an append against its exact dependency closure and current structural heads.
///
/// The returned heads are structural sequence projections only. They do not confer truth,
/// authority, freshness, lifecycle state, or conflict resolution semantics.
///
/// # Errors
///
/// Returns an error when the request, dependency graph, reference relationships, identity
/// uniqueness, or contiguous aggregate sequences are invalid.
pub fn validate_governance_record_append(
    request: &AppendGovernanceRecordBatch,
    dependency_closure: &[GovernanceRecord],
    structural_heads: &[GovernanceStructuralHead],
) -> Result<Vec<GovernanceStructuralHead>, GovernanceRecordJournalError> {
    let candidates = request.records()?;
    validate_candidate_relations(&candidates, dependency_closure)?;
    let records = index_records(&candidates, dependency_closure)?;
    build_next_heads(request, &candidates, structural_heads, &records)
}

/// Revalidates the exact candidate batch against its stored reference dependencies.
///
/// This is useful for exact replay and recovery, where current structural heads may have
/// advanced beyond the historical batch. It does not infer truth or lifecycle state.
///
/// # Errors
///
/// Returns an error for invalid request bytes, identities, references, subjects, or cycles.
pub fn validate_governance_record_relations(
    request: &AppendGovernanceRecordBatch,
    dependency_closure: &[GovernanceRecord],
) -> Result<(), GovernanceRecordJournalError> {
    let candidates = request.records()?;
    validate_candidate_relations(&candidates, dependency_closure)
}

/// Revalidates already-decoded stored records against their exact reference closure.
///
/// This narrow recovery boundary avoids constructing a synthetic append request while
/// preserving the same budget, identity, relation, and derivation checks.
///
/// # Errors
///
/// Returns an error for invalid records, identities, references, subjects, or cycles.
pub fn validate_governance_stored_record_relations(
    candidates: &[GovernanceRecord],
    dependency_closure: &[GovernanceRecord],
) -> Result<(), GovernanceRecordJournalError> {
    validate_candidate_relations(candidates, dependency_closure)
}

fn validate_candidate_relations(
    candidates: &[GovernanceRecord],
    dependency_closure: &[GovernanceRecord],
) -> Result<(), GovernanceRecordJournalError> {
    validate_dependency_budget(candidates, dependency_closure)?;
    let records = index_records(candidates, dependency_closure)?;
    validate_relations(candidates, &records)?;
    validate_derivation_graph(candidates, &records)
}

fn validate_dependency_budget(
    candidates: &[GovernanceRecord],
    dependencies: &[GovernanceRecord],
) -> Result<(), GovernanceRecordJournalError> {
    if dependencies.len() > MAX_GOVERNANCE_RECORD_DEPENDENCY_RECORDS {
        return Err(invalid(
            "journal dependency closure exceeds the record limit",
        ));
    }
    let mut bytes = 0_usize;
    for record in candidates.iter().chain(dependencies) {
        record.validate().map_err(|error| invalid(error.message))?;
        let canonical = record
            .canonical_record_json()
            .map_err(|error| invalid(error.message))?;
        bytes = bytes
            .checked_add(canonical.len())
            .ok_or_else(|| invalid("journal dependency byte count overflowed"))?;
    }
    if bytes > MAX_GOVERNANCE_RECORD_DEPENDENCY_BYTES {
        return Err(invalid("journal dependency closure exceeds the byte limit"));
    }
    Ok(())
}

fn index_records<'a>(
    candidates: &'a [GovernanceRecord],
    dependencies: &'a [GovernanceRecord],
) -> Result<RecordIndex<'a>, GovernanceRecordJournalError> {
    let mut records = BTreeMap::new();
    let mut identities = BTreeSet::new();
    for record in candidates.iter().chain(dependencies) {
        let metadata = record.metadata();
        let identity = (
            GovernanceRecordKind::from(record),
            metadata.aggregate_id.as_str(),
            metadata.sequence,
        );
        if records
            .insert(metadata.record_id.as_str(), record)
            .is_some()
            || !identities.insert(identity)
        {
            return Err(invalid(
                "journal append contains an existing record identity",
            ));
        }
    }
    Ok(records)
}

fn validate_relations(
    candidates: &[GovernanceRecord],
    records: &RecordIndex<'_>,
) -> Result<(), GovernanceRecordJournalError> {
    for record in candidates {
        validate_supersession(record, records)?;
        if let GovernanceRecord::Claim(claim) = record {
            for evidence_id in claim
                .spec
                .supporting_evidence_record_ids
                .iter()
                .chain(&claim.spec.contradicting_evidence_record_ids)
            {
                validate_evidence_ref(&claim.spec.subject, evidence_id, records)?;
            }
            for claim_id in &claim.spec.derived_from_claim_record_ids {
                require_claim(claim_id, records)?;
            }
        }
    }
    Ok(())
}

fn validate_supersession(
    record: &GovernanceRecord,
    records: &RecordIndex<'_>,
) -> Result<(), GovernanceRecordJournalError> {
    let metadata = record.metadata();
    if metadata.sequence == 1 && !metadata.supersedes_record_ids.is_empty() {
        return Err(invalid(
            "sequence one journal record cannot supersede another",
        ));
    }
    let mut immediate = metadata.sequence == 1;
    for prior_id in &metadata.supersedes_record_ids {
        let prior = records
            .get(prior_id.as_str())
            .ok_or_else(|| invalid("superseded journal record is unresolved"))?;
        let prior_metadata = prior.metadata();
        let same_aggregate = GovernanceRecordKind::from(record)
            == GovernanceRecordKind::from(*prior)
            && metadata.aggregate_id == prior_metadata.aggregate_id;
        if !same_aggregate || prior_metadata.sequence >= metadata.sequence {
            return Err(invalid(
                "journal supersession target is structurally invalid",
            ));
        }
        immediate |= prior_metadata.sequence == metadata.sequence - 1;
    }
    immediate
        .then_some(())
        .ok_or_else(|| invalid("journal supersession omits the immediate predecessor"))
}

fn validate_evidence_ref(
    subject: &str,
    evidence_id: &str,
    records: &RecordIndex<'_>,
) -> Result<(), GovernanceRecordJournalError> {
    let record = records
        .get(evidence_id)
        .ok_or_else(|| invalid("claim evidence reference is unresolved"))?;
    let GovernanceRecord::Evidence(evidence) = record else {
        return Err(invalid(
            "claim evidence reference targets a non-evidence record",
        ));
    };
    evidence
        .spec
        .subjects
        .binary_search_by(|candidate| candidate.as_str().cmp(subject))
        .map(|_| ())
        .map_err(|_| invalid("claim evidence does not cover the claim subject"))
}

fn require_claim(
    claim_id: &str,
    records: &RecordIndex<'_>,
) -> Result<(), GovernanceRecordJournalError> {
    match records.get(claim_id) {
        Some(GovernanceRecord::Claim(_)) => Ok(()),
        Some(GovernanceRecord::Evidence(_)) => Err(invalid(
            "derived claim reference targets an evidence record",
        )),
        None => Err(invalid("derived claim reference is unresolved")),
    }
}

fn validate_derivation_graph(
    candidates: &[GovernanceRecord],
    records: &RecordIndex<'_>,
) -> Result<(), GovernanceRecordJournalError> {
    let mut depths = BTreeMap::new();
    for record in candidates {
        let mut visiting = BTreeSet::new();
        let depth = longest_derivation_path(
            record.metadata().record_id.as_str(),
            records,
            &mut visiting,
            &mut depths,
        )?;
        if depth > MAX_GOVERNANCE_RECORD_REFERENCE_DEPTH {
            return Err(invalid("claim derivation exceeds the journal depth limit"));
        }
    }
    Ok(())
}

fn longest_derivation_path(
    record_id: &str,
    records: &RecordIndex<'_>,
    visiting: &mut BTreeSet<String>,
    depths: &mut BTreeMap<String, usize>,
) -> Result<usize, GovernanceRecordJournalError> {
    if let Some(depth) = depths.get(record_id) {
        return Ok(*depth);
    }
    if !visiting.insert(record_id.to_owned()) {
        return Err(invalid("claim derivation graph contains a cycle"));
    }
    let mut longest = 0;
    if let Some(GovernanceRecord::Claim(claim)) = records.get(record_id) {
        for dependency in &claim.spec.derived_from_claim_record_ids {
            let child = longest_derivation_path(dependency, records, visiting, depths)?;
            let candidate = child
                .checked_add(1)
                .ok_or_else(|| invalid("claim derivation depth overflowed"))?;
            longest = longest.max(candidate);
        }
    }
    visiting.remove(record_id);
    depths.insert(record_id.to_owned(), longest);
    Ok(longest)
}

fn build_next_heads(
    request: &AppendGovernanceRecordBatch,
    candidates: &[GovernanceRecord],
    heads: &[GovernanceStructuralHead],
    records: &RecordIndex<'_>,
) -> Result<Vec<GovernanceStructuralHead>, GovernanceRecordJournalError> {
    let mut grouped: BTreeMap<AggregateKey, Vec<&GovernanceRecord>> = BTreeMap::new();
    for record in candidates {
        grouped
            .entry(aggregate_key(record))
            .or_default()
            .push(record);
    }
    let indexed_heads = index_heads(heads, &grouped, records)?;
    let mut next = Vec::with_capacity(grouped.len());
    for (key, mut versions) in grouped {
        versions.sort_by_key(|record| record.metadata().sequence);
        let prior = indexed_heads.get(&key).copied();
        next.push(next_head_for_group(request, key, &versions, prior)?);
    }
    Ok(next)
}

fn index_heads<'a>(
    heads: &'a [GovernanceStructuralHead],
    grouped: &BTreeMap<AggregateKey, Vec<&GovernanceRecord>>,
    records: &RecordIndex<'_>,
) -> Result<BTreeMap<AggregateKey, &'a GovernanceStructuralHead>, GovernanceRecordJournalError> {
    let mut indexed = BTreeMap::new();
    for head in heads {
        head.validate()?;
        let key = (head.record_kind, head.aggregate_id.clone());
        if !grouped.contains_key(&key) || indexed.insert(key, head).is_some() {
            return Err(invalid("structural head set is not exact for the append"));
        }
        validate_head_record(head, records)?;
    }
    Ok(indexed)
}

fn validate_head_record(
    head: &GovernanceStructuralHead,
    records: &RecordIndex<'_>,
) -> Result<(), GovernanceRecordJournalError> {
    let record = records
        .get(head.record_id.as_str())
        .ok_or_else(|| invalid("structural head record is unresolved"))?;
    let metadata = record.metadata();
    let matches = GovernanceRecordKind::from(*record) == head.record_kind
        && metadata.aggregate_id == head.aggregate_id
        && metadata.sequence == head.sequence
        && record.integrity().canonical_sha256 == head.canonical_sha256;
    matches
        .then_some(())
        .ok_or_else(|| invalid("structural head diverges from its journal record"))
}

fn next_head_for_group(
    request: &AppendGovernanceRecordBatch,
    key: AggregateKey,
    versions: &[&GovernanceRecord],
    prior_head: Option<&GovernanceStructuralHead>,
) -> Result<GovernanceStructuralHead, GovernanceRecordJournalError> {
    let mut expected_sequence = prior_head.map_or(1_i128, |head| i128::from(head.sequence) + 1);
    let mut predecessor = prior_head.map(|head| head.record_id.as_str());
    for record in versions {
        let metadata = record.metadata();
        let follows = i128::from(metadata.sequence) == expected_sequence
            && predecessor.is_none_or(|id| {
                metadata
                    .supersedes_record_ids
                    .binary_search_by(|candidate| candidate.as_str().cmp(id))
                    .is_ok()
            });
        if !follows {
            return Err(invalid(
                "append does not continue the structural head exactly",
            ));
        }
        expected_sequence += 1;
        predecessor = Some(metadata.record_id.as_str());
    }
    let record = versions
        .last()
        .ok_or_else(|| invalid("empty aggregate append"))?;
    Ok(GovernanceStructuralHead {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        record_kind: key.0,
        aggregate_id: key.1,
        record_id: record.metadata().record_id.clone(),
        sequence: record.metadata().sequence,
        canonical_sha256: record.integrity().canonical_sha256.clone(),
        updated_at_ms: request.appended_at_ms,
    })
}

fn aggregate_key(record: &GovernanceRecord) -> AggregateKey {
    (
        GovernanceRecordKind::from(record),
        record.metadata().aggregate_id.clone(),
    )
}
