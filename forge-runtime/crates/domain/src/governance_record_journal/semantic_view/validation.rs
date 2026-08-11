use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use crate::governance_contract::ClaimType;

use super::super::{GovernanceRecordKind, is_governance_record_identifier};
use super::{
    GOVERNANCE_SEMANTIC_VIEW_VERSION, GovernanceClaimConflictGroup, GovernanceRecordJournalError,
    GovernanceSemanticAssessment, GovernanceSemanticProjection, GovernanceTemporalState,
    GovernanceValidationJob, MAX_GOVERNANCE_CONFLICT_MEMBERS, MAX_GOVERNANCE_SEMANTIC_LIST_LIMIT,
};

pub(super) fn validate_projection(
    projection: &GovernanceSemanticProjection,
) -> Result<(), GovernanceRecordJournalError> {
    let head = &projection.head;
    let interval = head.valid_from_unix_ms >= 0
        && head
            .valid_until_unix_ms
            .is_none_or(|until| until > head.valid_from_unix_ms);
    let common = projection.v == GOVERNANCE_SEMANTIC_VIEW_VERSION
        && head.v == GOVERNANCE_SEMANTIC_VIEW_VERSION
        && is_governance_record_identifier(&head.aggregate_id)
        && is_governance_record_identifier(&head.record_id)
        && is_governance_record_identifier(&head.project_id)
        && is_governance_record_identifier(&head.scope)
        && head.sequence > 0
        && lower_sha256(&head.canonical_sha256)
        && interval
        && i64::try_from(head.updated_at_ms).is_ok();
    if !common || !claim_shape(projection) || !lower_sha256(&projection.projection_sha256) {
        return Err(invalid("semantic projection fields are invalid"));
    }
    let expected = super::governance_semantic_projection_sha256(projection)?;
    if projection.projection_sha256 != expected {
        return Err(invalid("semantic projection digest diverged"));
    }
    Ok(())
}

fn claim_shape(projection: &GovernanceSemanticProjection) -> bool {
    match (projection.head.record_kind, &projection.claim) {
        (GovernanceRecordKind::EvidenceRecord, None) => {
            valid_evidence_state(&projection.head.declared_state)
        }
        (GovernanceRecordKind::KnowledgeClaim, Some(claim)) => {
            let plan_absent = claim.validation_due_unix_ms.is_none()
                && claim.validation_owner_id.is_none()
                && claim.validation_plan_sha256.is_none()
                && claim.required_evidence_types.is_empty();
            let plan_present = claim.validation_due_unix_ms.is_some_and(|value| value >= 0)
                && claim
                    .validation_owner_id
                    .as_deref()
                    .is_some_and(is_governance_record_identifier)
                && claim
                    .validation_plan_sha256
                    .as_deref()
                    .is_some_and(lower_sha256)
                && !claim.required_evidence_types.is_empty()
                && claim
                    .required_evidence_types
                    .windows(2)
                    .all(|pair| pair[0] < pair[1]);
            let plan_valid = match claim.claim_type {
                ClaimType::Assumption | ClaimType::Hypothesis => plan_present,
                _ => plan_absent,
            };
            is_governance_record_identifier(&claim.subject)
                && is_governance_record_identifier(&claim.predicate)
                && lower_sha256(&claim.object_sha256)
                && conflict_key_is_bound(projection)
                && super::state::authority_free_shadow_declared_state(
                    claim.claim_type,
                    &projection.head.declared_state,
                )
                && claim.review_by_unix_ms.is_none_or(|value| value >= 0)
                && plan_valid
        }
        (GovernanceRecordKind::EvidenceRecord, Some(_))
        | (GovernanceRecordKind::KnowledgeClaim, None) => false,
    }
}

fn conflict_key_is_bound(projection: &GovernanceSemanticProjection) -> bool {
    let Some(claim) = &projection.claim else {
        return false;
    };
    super::identity::claim_conflict_key_sha256(
        claim.claim_type,
        &projection.head.project_id,
        &projection.head.scope,
        &claim.subject,
        &claim.predicate,
    )
    .is_ok_and(|expected| expected == claim.conflict_key_sha256)
}

pub(super) fn validate_assessment(
    assessment: &GovernanceSemanticAssessment,
) -> Result<(), GovernanceRecordJournalError> {
    assessment.projection.validate()?;
    if assessment.v != GOVERNANCE_SEMANTIC_VIEW_VERSION || assessment.evaluated_at_unix_ms < 0 {
        return Err(invalid("semantic assessment fields are invalid"));
    }
    let expected =
        super::evaluate::temporal_state(&assessment.projection, assessment.evaluated_at_unix_ms);
    if expected != assessment.temporal_state {
        return Err(invalid("semantic assessment temporal state diverged"));
    }
    Ok(())
}

pub(super) fn validate_conflict_group(
    group: &GovernanceClaimConflictGroup,
) -> Result<(), GovernanceRecordJournalError> {
    let member_count = group.members.len();
    let sorted = group
        .members
        .windows(2)
        .all(|pair| pair[0].aggregate_id.as_bytes() < pair[1].aggregate_id.as_bytes());
    let distinct_values: BTreeSet<_> = group
        .members
        .iter()
        .map(|member| member.object_sha256.as_str())
        .collect();
    let members_valid = group.members.iter().all(|member| {
        is_governance_record_identifier(&member.aggregate_id)
            && is_governance_record_identifier(&member.record_id)
            && member.sequence > 0
            && super::state::authority_free_shadow_declared_state(
                group.claim_type,
                &member.declared_state,
            )
            && lower_sha256(&member.object_sha256)
            && !matches!(
                member.temporal_state,
                GovernanceTemporalState::NotYetValid | GovernanceTemporalState::ValidityExpired
            )
    });
    let key_bound = super::identity::claim_conflict_key_sha256(
        group.claim_type,
        &group.project_id,
        &group.scope,
        &group.subject,
        &group.predicate,
    )
    .is_ok_and(|expected| expected == group.conflict_key_sha256);
    let valid = group.v == GOVERNANCE_SEMANTIC_VIEW_VERSION
        && key_bound
        && is_governance_record_identifier(&group.project_id)
        && is_governance_record_identifier(&group.scope)
        && is_governance_record_identifier(&group.subject)
        && is_governance_record_identifier(&group.predicate)
        && group.evaluated_at_unix_ms >= 0
        && (2..=MAX_GOVERNANCE_CONFLICT_MEMBERS).contains(&member_count)
        && distinct_values.len() >= 2
        && sorted
        && members_valid;
    valid
        .then_some(())
        .ok_or_else(|| invalid("claim conflict group is invalid"))
}

pub(super) fn validate_validation_job(
    job: &GovernanceValidationJob,
) -> Result<(), GovernanceRecordJournalError> {
    let id_bound = super::identity::validation_job_id(&job.record_id, &job.validation_plan_sha256)
        .is_ok_and(|expected| expected == job.job_id);
    let evidence_sorted = !job.required_evidence_types.is_empty()
        && job
            .required_evidence_types
            .windows(2)
            .all(|pair| pair[0] < pair[1]);
    let due_consistent = job.due
        == (job.evaluated_at_unix_ms >= job.due_at_unix_ms
            && !matches!(
                job.temporal_state,
                GovernanceTemporalState::NotYetValid | GovernanceTemporalState::ValidityExpired
            ));
    let valid = job.v == GOVERNANCE_SEMANTIC_VIEW_VERSION
        && id_bound
        && matches!(
            job.claim_type,
            ClaimType::Assumption | ClaimType::Hypothesis
        )
        && is_governance_record_identifier(&job.aggregate_id)
        && is_governance_record_identifier(&job.record_id)
        && is_governance_record_identifier(&job.owner_id)
        && lower_sha256(&job.validation_plan_sha256)
        && super::state::authority_free_shadow_declared_state(job.claim_type, &job.declared_state)
        && job.due_at_unix_ms >= 0
        && job.evaluated_at_unix_ms >= 0
        && evidence_sorted
        && due_consistent;
    valid
        .then_some(())
        .ok_or_else(|| invalid("validation job is invalid"))
}

pub(super) fn validate_list_filter(
    as_of_unix_ms: i64,
    limit: usize,
) -> Result<(), GovernanceRecordJournalError> {
    if as_of_unix_ms >= 0 && (1..=MAX_GOVERNANCE_SEMANTIC_LIST_LIMIT).contains(&limit) {
        Ok(())
    } else {
        Err(invalid("semantic list filter is invalid"))
    }
}

pub(crate) fn digest_hex(domain: &[u8], parts: &[&[u8]]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for part in parts {
        digest.update(part);
    }
    lower_hex(&digest.finalize())
}

fn valid_evidence_state(value: &str) -> bool {
    matches!(value, "expired" | "invalid" | "unavailable" | "valid")
}

fn lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

pub(crate) fn invalid(message: impl Into<String>) -> GovernanceRecordJournalError {
    GovernanceRecordJournalError {
        message: message.into(),
    }
}
