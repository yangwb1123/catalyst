use crate::runtime_domain::{GovernanceSemanticProjection, GovernanceValidationJob, HubStoreError};

use super::super::error;
use super::rows::{RawProjection, RawValidationJob, claim_type_name};

pub(super) fn validate_projection(
    raw: &RawProjection,
    expected: &GovernanceSemanticProjection,
) -> Result<(), HubStoreError> {
    expected
        .validate()
        .map_err(|problem| error::corrupt(problem.message))?;
    let head = &expected.head;
    let common = raw.record_kind == head.record_kind.as_str()
        && raw.aggregate_id == head.aggregate_id
        && raw.semantic_view_version == i64::from(expected.v)
        && raw.record_id == head.record_id
        && raw.sequence == head.sequence
        && error::stored_digest(&raw.canonical_sha256, "semantic canonical digest")?
            == head.canonical_sha256
        && raw.project_id == head.project_id
        && raw.scope == head.scope
        && raw.declared_state == head.declared_state
        && raw.valid_from_unix_ms == head.valid_from_unix_ms
        && raw.valid_until_unix_ms == head.valid_until_unix_ms
        && error::stored_digest(&raw.projection_sha256, "semantic projection digest")?
            == expected.projection_sha256
        && error::stored_u64(raw.updated_at_ms, "semantic update time")? == head.updated_at_ms;
    if !common || !claim_matches(raw, expected)? {
        return Err(error::corrupt(
            "materialized governance semantic projection diverged",
        ));
    }
    Ok(())
}

fn claim_matches(
    raw: &RawProjection,
    expected: &GovernanceSemanticProjection,
) -> Result<bool, HubStoreError> {
    let Some(claim) = &expected.claim else {
        return Ok(raw.claim_type.is_none()
            && raw.subject.is_none()
            && raw.predicate.is_none()
            && raw.object_sha256.is_none()
            && raw.conflict_key_sha256.is_none()
            && raw.review_by_unix_ms.is_none()
            && raw.validation_due_unix_ms.is_none()
            && raw.validation_owner_id.is_none()
            && raw.validation_plan_sha256.is_none()
            && raw.required_evidence_types_json.is_none()
            && raw.claim_projection_sha256.is_none());
    };
    let evidence = serde_json::to_string(&claim.required_evidence_types)
        .map_err(|problem| error::corrupt(format!("cannot encode evidence types: {problem}")))?;
    let object = optional_digest(raw.object_sha256.as_deref(), "claim object digest")?;
    let conflict = optional_digest(raw.conflict_key_sha256.as_deref(), "claim conflict digest")?;
    let plan = optional_digest(
        raw.validation_plan_sha256.as_deref(),
        "validation plan digest",
    )?;
    let projection = optional_digest(
        raw.claim_projection_sha256.as_deref(),
        "claim projection digest",
    )?;
    Ok(
        raw.claim_type.as_deref() == Some(claim_type_name(claim.claim_type))
            && raw.subject.as_deref() == Some(claim.subject.as_str())
            && raw.predicate.as_deref() == Some(claim.predicate.as_str())
            && object.as_deref() == Some(claim.object_sha256.as_str())
            && conflict.as_deref() == Some(claim.conflict_key_sha256.as_str())
            && raw.review_by_unix_ms == claim.review_by_unix_ms
            && raw.validation_due_unix_ms == claim.validation_due_unix_ms
            && raw.validation_owner_id == claim.validation_owner_id
            && plan == claim.validation_plan_sha256
            && raw.required_evidence_types_json.as_deref() == Some(evidence.as_str())
            && projection.as_deref() == Some(expected.projection_sha256.as_str()),
    )
}

pub(super) fn validate_job(
    raw: &RawValidationJob,
    expected: &GovernanceValidationJob,
    projection_sha256: &str,
) -> Result<(), HubStoreError> {
    expected
        .validate()
        .map_err(|problem| error::corrupt(problem.message))?;
    let evidence = serde_json::to_string(&expected.required_evidence_types)
        .map_err(|problem| error::corrupt(format!("cannot encode evidence types: {problem}")))?;
    let exact = raw.job_id == expected.job_id
        && raw.aggregate_id == expected.aggregate_id
        && raw.record_id == expected.record_id
        && raw.claim_type == claim_type_name(expected.claim_type)
        && raw.due_at_unix_ms == expected.due_at_unix_ms
        && raw.owner_id == expected.owner_id
        && raw.required_evidence_types_json == evidence
        && error::stored_digest(&raw.validation_plan_sha256, "validation plan digest")?
            == expected.validation_plan_sha256
        && error::stored_digest(&raw.projection_sha256, "validation projection digest")?
            == projection_sha256;
    exact
        .then_some(())
        .ok_or_else(|| error::corrupt("materialized validation job diverged"))
}

fn optional_digest(value: Option<&[u8]>, subject: &str) -> Result<Option<String>, HubStoreError> {
    value
        .map(|bytes| error::stored_digest(bytes, subject))
        .transpose()
}
