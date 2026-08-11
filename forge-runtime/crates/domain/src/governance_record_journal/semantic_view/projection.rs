use serde::Serialize;

use crate::governance_contract::{
    ClaimObjectType, ClaimObjectValue, GovernanceRecord, KnowledgeClaim, ValidationPlan,
};

use super::super::GovernanceRecordKind;
use super::{
    GOVERNANCE_CLAIM_OBJECT_DIGEST_DOMAIN, GOVERNANCE_SEMANTIC_PROJECTION_DIGEST_DOMAIN,
    GOVERNANCE_SEMANTIC_VIEW_VERSION, GOVERNANCE_VALIDATION_PLAN_DIGEST_DOMAIN,
    GovernanceClaimSemanticFields, GovernanceRecordJournalError, GovernanceSemanticHead,
    GovernanceSemanticProjection, digest_hex, invalid,
};

#[derive(Serialize)]
struct ClaimObject<'a> {
    object_type: ClaimObjectType,
    object_value: &'a ClaimObjectValue,
}

/// Projects the current semantic state of one validated governance record.
///
/// # Errors
///
/// Returns an error when the source record, local update time, canonical
/// encoding, derived Claim fields, or resulting projection is invalid.
pub fn governance_semantic_projection(
    record: &GovernanceRecord,
    updated_at_ms: u64,
) -> Result<GovernanceSemanticProjection, GovernanceRecordJournalError> {
    record
        .validate()
        .map_err(|problem| invalid(problem.message))?;
    i64::try_from(updated_at_ms).map_err(|_| invalid("semantic projection time is invalid"))?;
    let metadata = record.metadata();
    let (declared_state, valid_from, valid_until, claim) = match record {
        GovernanceRecord::Evidence(evidence) => (
            evidence_state_name(evidence.status.state).to_owned(),
            evidence.status.valid_from_unix_ms,
            evidence.status.valid_until_unix_ms,
            None,
        ),
        GovernanceRecord::Claim(claim) => (
            super::state::claim_state_name(claim.status.state).to_owned(),
            claim.status.valid_from_unix_ms,
            claim.status.valid_until_unix_ms,
            Some(claim_fields(claim)?),
        ),
    };
    let mut projection = GovernanceSemanticProjection {
        v: GOVERNANCE_SEMANTIC_VIEW_VERSION,
        head: GovernanceSemanticHead {
            v: GOVERNANCE_SEMANTIC_VIEW_VERSION,
            record_kind: GovernanceRecordKind::from(record),
            aggregate_id: metadata.aggregate_id.clone(),
            record_id: metadata.record_id.clone(),
            sequence: metadata.sequence,
            canonical_sha256: record.integrity().canonical_sha256.clone(),
            project_id: metadata.project_id.clone(),
            scope: metadata.scope.clone(),
            declared_state,
            valid_from_unix_ms: valid_from,
            valid_until_unix_ms: valid_until,
            updated_at_ms,
        },
        claim,
        projection_sha256: String::new(),
    };
    projection.projection_sha256 = governance_semantic_projection_sha256(&projection)?;
    projection.validate()?;
    Ok(projection)
}

/// Computes the domain-separated digest of a semantic projection.
///
/// # Errors
///
/// Returns an error when the digest payload cannot be canonically encoded.
pub fn governance_semantic_projection_sha256(
    projection: &GovernanceSemanticProjection,
) -> Result<String, GovernanceRecordJournalError> {
    let mut payload = projection.clone();
    payload.projection_sha256.clear();
    let canonical = crate::governance_contract::codec::canonical_json(&payload)
        .map_err(|problem| invalid(problem.message))?;
    Ok(digest_hex(
        GOVERNANCE_SEMANTIC_PROJECTION_DIGEST_DOMAIN,
        &[canonical.as_bytes()],
    ))
}

fn claim_fields(
    claim: &KnowledgeClaim,
) -> Result<GovernanceClaimSemanticFields, GovernanceRecordJournalError> {
    let conflict_key_sha256 = super::identity::claim_conflict_key_sha256(
        claim.spec.claim_type,
        &claim.metadata.project_id,
        &claim.metadata.scope,
        &claim.spec.subject,
        &claim.spec.predicate,
    )?;
    let object = ClaimObject {
        object_type: claim.spec.object_type,
        object_value: &claim.spec.object_value,
    };
    let plan = claim.spec.validation_plan.as_ref();
    Ok(GovernanceClaimSemanticFields {
        claim_type: claim.spec.claim_type,
        subject: claim.spec.subject.clone(),
        predicate: claim.spec.predicate.clone(),
        object_sha256: super::identity::canonical_digest(
            GOVERNANCE_CLAIM_OBJECT_DIGEST_DOMAIN,
            &object,
        )?,
        conflict_key_sha256,
        review_by_unix_ms: claim.spec.review_by_unix_ms,
        validation_due_unix_ms: plan.map(|value| value.due_at_unix_ms),
        validation_owner_id: plan.map(|value| value.owner_id.clone()),
        validation_plan_sha256: plan.map(validation_plan_digest).transpose()?,
        required_evidence_types: plan
            .map(|value| value.required_evidence_types.clone())
            .unwrap_or_default(),
    })
}

fn validation_plan_digest(plan: &ValidationPlan) -> Result<String, GovernanceRecordJournalError> {
    super::identity::canonical_digest(GOVERNANCE_VALIDATION_PLAN_DIGEST_DOMAIN, plan)
}

fn evidence_state_name(state: crate::governance_contract::EvidenceState) -> &'static str {
    use crate::governance_contract::EvidenceState;
    match state {
        EvidenceState::Expired => "expired",
        EvidenceState::Invalid => "invalid",
        EvidenceState::Unavailable => "unavailable",
        EvidenceState::Valid => "valid",
    }
}
