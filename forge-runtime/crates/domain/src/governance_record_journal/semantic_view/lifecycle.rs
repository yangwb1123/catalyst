use std::collections::BTreeMap;

use crate::governance_contract::{ClaimState, ClaimType, GovernanceRecord, KnowledgeClaim};

use super::super::{GovernanceRecordKind, GovernanceStructuralHead};
use super::{GovernanceRecordJournalError, invalid};

type AggregateKey = (GovernanceRecordKind, String);

/// Validates semantic lifecycle transitions for one atomic append batch.
///
/// # Errors
///
/// Returns an error when a predecessor is unresolved or any aggregate starts,
/// changes identity, or transitions outside the authority-free lifecycle.
pub fn validate_governance_semantic_append(
    candidates: &[GovernanceRecord],
    dependency_closure: &[GovernanceRecord],
    structural_heads: &[GovernanceStructuralHead],
) -> Result<(), GovernanceRecordJournalError> {
    let dependencies: BTreeMap<_, _> = dependency_closure
        .iter()
        .map(|record| (record.metadata().record_id.as_str(), record))
        .collect();
    let heads: BTreeMap<_, _> = structural_heads
        .iter()
        .map(|head| ((head.record_kind, head.aggregate_id.as_str()), head))
        .collect();
    let mut groups: BTreeMap<AggregateKey, Vec<&GovernanceRecord>> = BTreeMap::new();
    for record in candidates {
        groups
            .entry((
                GovernanceRecordKind::from(record),
                record.metadata().aggregate_id.clone(),
            ))
            .or_default()
            .push(record);
    }
    for ((kind, aggregate_id), versions) in &mut groups {
        versions.sort_by_key(|record| record.metadata().sequence);
        let prior = heads
            .get(&(*kind, aggregate_id.as_str()))
            .map(|head| {
                dependencies
                    .get(head.record_id.as_str())
                    .copied()
                    .ok_or_else(|| invalid("semantic predecessor is unresolved"))
            })
            .transpose()?;
        validate_versions(prior, versions)?;
    }
    Ok(())
}

fn validate_versions<'a>(
    mut previous: Option<&'a GovernanceRecord>,
    versions: &[&'a GovernanceRecord],
) -> Result<(), GovernanceRecordJournalError> {
    for next in versions {
        validate_governance_semantic_transition(previous, next)?;
        previous = Some(next);
    }
    Ok(())
}

/// Validates one initial or successor semantic lifecycle transition.
///
/// # Errors
///
/// Returns an error when the initial state, structural predecessor, stable
/// semantic identity, or authority-free lifecycle transition is invalid.
pub fn validate_governance_semantic_transition(
    previous: Option<&GovernanceRecord>,
    next: &GovernanceRecord,
) -> Result<(), GovernanceRecordJournalError> {
    match previous {
        None => validate_initial(next),
        Some(previous) => validate_successor(previous, next),
    }
}

fn validate_initial(next: &GovernanceRecord) -> Result<(), GovernanceRecordJournalError> {
    if next.metadata().sequence != 1 || !next.metadata().supersedes_record_ids.is_empty() {
        return Err(invalid("semantic aggregate does not start at sequence one"));
    }
    if let GovernanceRecord::Claim(claim) = next
        && !super::state::authority_free_shadow_claim_state(
            claim.spec.claim_type,
            claim.status.state,
        )
    {
        return Err(invalid(
            "claim lifecycle does not start in an authority-free shadow state",
        ));
    }
    Ok(())
}

fn validate_successor(
    previous: &GovernanceRecord,
    next: &GovernanceRecord,
) -> Result<(), GovernanceRecordJournalError> {
    let prior = previous.metadata();
    let current = next.metadata();
    let structural = GovernanceRecordKind::from(previous) == GovernanceRecordKind::from(next)
        && prior.aggregate_id == current.aggregate_id
        && current.sequence == prior.sequence + 1
        && current
            .supersedes_record_ids
            .binary_search_by(|candidate| candidate.as_str().cmp(&prior.record_id))
            .is_ok();
    let common = prior.project_id == current.project_id
        && prior.scope == current.scope
        && current.created_at_unix_ms >= prior.created_at_unix_ms;
    if !structural || !common {
        return Err(invalid(
            "semantic successor changes stable aggregate metadata",
        ));
    }
    match (previous, next) {
        (GovernanceRecord::Evidence(_), GovernanceRecord::Evidence(_)) => Ok(()),
        (GovernanceRecord::Claim(previous), GovernanceRecord::Claim(next)) => {
            validate_claim_successor(previous, next)
        }
        _ => Err(invalid("semantic successor changes record kind")),
    }
}

fn validate_claim_successor(
    previous: &KnowledgeClaim,
    next: &KnowledgeClaim,
) -> Result<(), GovernanceRecordJournalError> {
    let stable = previous.spec.claim_type == next.spec.claim_type
        && previous.spec.subject == next.spec.subject
        && previous.spec.predicate == next.spec.predicate
        && previous.spec.object_type == next.spec.object_type
        && previous.spec.object_value == next.spec.object_value
        && previous.spec.owner == next.spec.owner;
    if !stable {
        return Err(invalid("claim successor changes semantic identity"));
    }
    if allowed_claim_transition(
        previous.spec.claim_type,
        previous.status.state,
        next.status.state,
    ) {
        Ok(())
    } else {
        Err(invalid(
            "claim lifecycle transition is not allowed in shadow mode",
        ))
    }
}

fn allowed_claim_transition(claim_type: ClaimType, from: ClaimState, to: ClaimState) -> bool {
    if !super::state::authority_free_shadow_claim_state(claim_type, from)
        || !super::state::authority_free_shadow_claim_state(claim_type, to)
    {
        return false;
    }
    match claim_type {
        ClaimType::Fact => matches!(
            (from, to),
            (
                ClaimState::Candidate | ClaimState::Contested,
                ClaimState::Candidate | ClaimState::Contested
            )
        ),
        ClaimType::Constraint | ClaimType::Inference | ClaimType::Lesson => {
            from == ClaimState::Candidate && to == ClaimState::Candidate
        }
        ClaimType::Decision => from == ClaimState::Proposed && to == ClaimState::Proposed,
        ClaimType::Assumption | ClaimType::Hypothesis => matches!(
            (from, to),
            (ClaimState::Open, ClaimState::Open | ClaimState::Testing)
                | (ClaimState::Testing, ClaimState::Testing)
        ),
        ClaimType::Proposal => matches!(
            (from, to),
            (ClaimState::Draft, ClaimState::Draft | ClaimState::Submitted)
                | (ClaimState::Submitted, ClaimState::Submitted)
        ),
        ClaimType::Unknown => matches!(
            (from, to),
            (
                ClaimState::Open,
                ClaimState::Open | ClaimState::Investigating
            ) | (ClaimState::Investigating, ClaimState::Investigating)
        ),
    }
}
