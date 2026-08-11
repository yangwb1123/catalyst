use std::collections::{BTreeMap, BTreeSet};

use crate::governance_contract::ClaimType;

use super::{
    GOVERNANCE_SEMANTIC_VIEW_VERSION, GovernanceClaimConflictGroup, GovernanceClaimConflictMember,
    GovernanceClaimSemanticFields, GovernanceRecordJournalError, GovernanceSemanticAssessment,
    GovernanceSemanticProjection, GovernanceTemporalState, GovernanceValidationJob, invalid,
};

/// Evaluates one materialized semantic projection at an explicit caller time.
///
/// # Errors
///
/// Returns an error when the projection is invalid or the evaluation time is
/// negative.
pub fn evaluate_governance_semantic_projection(
    projection: GovernanceSemanticProjection,
    as_of_unix_ms: i64,
) -> Result<GovernanceSemanticAssessment, GovernanceRecordJournalError> {
    projection.validate()?;
    if as_of_unix_ms < 0 {
        return Err(invalid("semantic evaluation time is invalid"));
    }
    let temporal_state = temporal_state(&projection, as_of_unix_ms);
    let assessment = GovernanceSemanticAssessment {
        v: GOVERNANCE_SEMANTIC_VIEW_VERSION,
        projection,
        evaluated_at_unix_ms: as_of_unix_ms,
        temporal_state,
    };
    assessment.validate()?;
    Ok(assessment)
}

/// Derives deterministic conflict candidates from active Claim projections.
///
/// # Errors
///
/// Returns an error when a candidate is invalid, is not a Claim, collides on a
/// conflict digest with divergent semantic fields, or the evaluation time is
/// negative.
pub fn governance_claim_conflict_groups(
    projections: Vec<GovernanceSemanticProjection>,
    as_of_unix_ms: i64,
) -> Result<Vec<GovernanceClaimConflictGroup>, GovernanceRecordJournalError> {
    if as_of_unix_ms < 0 {
        return Err(invalid("conflict evaluation time is invalid"));
    }
    let mut grouped: BTreeMap<String, Vec<GovernanceSemanticProjection>> = BTreeMap::new();
    for projection in projections {
        projection.validate()?;
        let Some(claim) = &projection.claim else {
            return Err(invalid("conflict candidate is not a claim projection"));
        };
        if active_at(&projection, as_of_unix_ms) {
            grouped
                .entry(claim.conflict_key_sha256.clone())
                .or_default()
                .push(projection);
        }
    }
    grouped
        .into_iter()
        .filter_map(|(key, values)| conflict_group(key, values, as_of_unix_ms).transpose())
        .collect()
}

/// Derives the validation job for an Assumption or Hypothesis projection.
///
/// # Errors
///
/// Returns an error when the projection or evaluation time is invalid, or when
/// a validation-bearing Claim has an incomplete validation plan.
pub fn governance_validation_job(
    projection: &GovernanceSemanticProjection,
    as_of_unix_ms: i64,
) -> Result<Option<GovernanceValidationJob>, GovernanceRecordJournalError> {
    projection.validate()?;
    if as_of_unix_ms < 0 {
        return Err(invalid("validation-job evaluation time is invalid"));
    }
    let Some(claim) = &projection.claim else {
        return Ok(None);
    };
    if !matches!(
        claim.claim_type,
        ClaimType::Assumption | ClaimType::Hypothesis
    ) {
        return Ok(None);
    }
    let (Some(due_at), Some(owner), Some(plan_sha256)) = (
        claim.validation_due_unix_ms,
        claim.validation_owner_id.as_ref(),
        claim.validation_plan_sha256.as_ref(),
    ) else {
        return Err(invalid("validation claim has no complete validation plan"));
    };
    let temporal = temporal_state(projection, as_of_unix_ms);
    let due = as_of_unix_ms >= due_at
        && !matches!(
            temporal,
            GovernanceTemporalState::NotYetValid | GovernanceTemporalState::ValidityExpired
        );
    let job = GovernanceValidationJob {
        v: GOVERNANCE_SEMANTIC_VIEW_VERSION,
        job_id: super::identity::validation_job_id(&projection.head.record_id, plan_sha256)?,
        aggregate_id: projection.head.aggregate_id.clone(),
        record_id: projection.head.record_id.clone(),
        claim_type: claim.claim_type,
        due_at_unix_ms: due_at,
        owner_id: owner.clone(),
        required_evidence_types: claim.required_evidence_types.clone(),
        validation_plan_sha256: plan_sha256.clone(),
        declared_state: projection.head.declared_state.clone(),
        evaluated_at_unix_ms: as_of_unix_ms,
        temporal_state: temporal,
        due,
    };
    job.validate()?;
    Ok(Some(job))
}

pub(super) fn temporal_state(
    projection: &GovernanceSemanticProjection,
    as_of_unix_ms: i64,
) -> GovernanceTemporalState {
    let head = &projection.head;
    if as_of_unix_ms < head.valid_from_unix_ms {
        return GovernanceTemporalState::NotYetValid;
    }
    if head
        .valid_until_unix_ms
        .is_some_and(|until| as_of_unix_ms >= until)
    {
        return GovernanceTemporalState::ValidityExpired;
    }
    let Some(claim) = &projection.claim else {
        return GovernanceTemporalState::Fresh;
    };
    if claim
        .validation_due_unix_ms
        .is_some_and(|due| as_of_unix_ms >= due)
        && matches!(
            claim.claim_type,
            ClaimType::Assumption | ClaimType::Hypothesis
        )
    {
        return GovernanceTemporalState::ValidationOverdue;
    }
    if claim
        .review_by_unix_ms
        .is_some_and(|review| as_of_unix_ms >= review)
    {
        return GovernanceTemporalState::ReviewOverdue;
    }
    GovernanceTemporalState::Fresh
}

fn active_at(projection: &GovernanceSemanticProjection, as_of_unix_ms: i64) -> bool {
    !matches!(
        temporal_state(projection, as_of_unix_ms),
        GovernanceTemporalState::NotYetValid | GovernanceTemporalState::ValidityExpired
    )
}

fn conflict_group(
    key: String,
    mut projections: Vec<GovernanceSemanticProjection>,
    as_of_unix_ms: i64,
) -> Result<Option<GovernanceClaimConflictGroup>, GovernanceRecordJournalError> {
    projections.sort_by(|left, right| {
        left.head
            .aggregate_id
            .as_bytes()
            .cmp(right.head.aggregate_id.as_bytes())
    });
    let values: BTreeSet<_> = projections
        .iter()
        .filter_map(|projection| projection.claim.as_ref())
        .map(|claim| claim.object_sha256.as_str())
        .collect();
    if values.len() < 2 {
        return Ok(None);
    }
    let (first, first_claim) = validate_conflict_group_consistency(&key, &projections)?;
    let members = conflict_members(&projections, as_of_unix_ms);
    let group = GovernanceClaimConflictGroup {
        v: GOVERNANCE_SEMANTIC_VIEW_VERSION,
        conflict_key_sha256: key,
        claim_type: first_claim.claim_type,
        project_id: first.head.project_id.clone(),
        scope: first.head.scope.clone(),
        subject: first_claim.subject.clone(),
        predicate: first_claim.predicate.clone(),
        evaluated_at_unix_ms: as_of_unix_ms,
        members,
    };
    group.validate()?;
    Ok(Some(group))
}

fn validate_conflict_group_consistency<'a>(
    key: &str,
    projections: &'a [GovernanceSemanticProjection],
) -> Result<
    (
        &'a GovernanceSemanticProjection,
        &'a GovernanceClaimSemanticFields,
    ),
    GovernanceRecordJournalError,
> {
    let first = projections
        .first()
        .ok_or_else(|| invalid("conflict group is empty"))?;
    let first_claim = first
        .claim
        .as_ref()
        .ok_or_else(|| invalid("conflict group member is not a claim"))?;
    let consistent = projections.iter().all(|projection| {
        projection.claim.as_ref().is_some_and(|claim| {
            claim.conflict_key_sha256.as_str() == key
                && claim.claim_type == first_claim.claim_type
                && projection.head.project_id == first.head.project_id
                && projection.head.scope == first.head.scope
                && claim.subject == first_claim.subject
                && claim.predicate == first_claim.predicate
        })
    });
    if !consistent {
        return Err(invalid(
            "conflict-key collision has divergent semantic fields",
        ));
    }
    Ok((first, first_claim))
}

fn conflict_members(
    projections: &[GovernanceSemanticProjection],
    as_of_unix_ms: i64,
) -> Vec<GovernanceClaimConflictMember> {
    projections
        .iter()
        .map(|projection| {
            let claim = projection
                .claim
                .as_ref()
                .expect("validated claim projection");
            GovernanceClaimConflictMember {
                aggregate_id: projection.head.aggregate_id.clone(),
                record_id: projection.head.record_id.clone(),
                sequence: projection.head.sequence,
                declared_state: projection.head.declared_state.clone(),
                object_sha256: claim.object_sha256.clone(),
                temporal_state: temporal_state(projection, as_of_unix_ms),
            }
        })
        .collect()
}
