use crate::runtime_domain::{
    GovernanceClaimConflictGroup, GovernanceRecordKind, GovernanceSemanticListFilter,
    GovernanceSemanticProjection, GovernanceValidationJob, GovernanceValidationJobFilter,
    is_governance_record_identifier,
};

use super::service::GovernanceSemanticViewServiceError;

pub(super) fn invalid(message: impl Into<String>) -> GovernanceSemanticViewServiceError {
    GovernanceSemanticViewServiceError::InvalidInput {
        message: message.into(),
    }
}

pub(super) const fn inconsistent() -> GovernanceSemanticViewServiceError {
    GovernanceSemanticViewServiceError::InconsistentStoreResult
}

pub(super) fn validate_identifier(value: &str) -> Result<(), GovernanceSemanticViewServiceError> {
    is_governance_record_identifier(value)
        .then_some(())
        .ok_or_else(|| invalid("governance aggregate identifier is invalid"))
}

pub(super) fn validate_projection(
    projection: &GovernanceSemanticProjection,
    record_kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<(), GovernanceSemanticViewServiceError> {
    projection.validate().map_err(|_| inconsistent())?;
    if projection.head.record_kind != record_kind || projection.head.aggregate_id != aggregate_id {
        return Err(inconsistent());
    }
    Ok(())
}

pub(super) fn validate_conflicts(
    groups: &[GovernanceClaimConflictGroup],
    filter: &GovernanceSemanticListFilter,
) -> Result<(), GovernanceSemanticViewServiceError> {
    if groups.len() > filter.limit {
        return Err(inconsistent());
    }
    let mut previous: Option<&str> = None;
    for group in groups {
        group.validate().map_err(|_| inconsistent())?;
        let ordered =
            previous.is_none_or(|key| key.as_bytes() < group.conflict_key_sha256.as_bytes());
        if group.evaluated_at_unix_ms != filter.as_of_unix_ms || !ordered {
            return Err(inconsistent());
        }
        previous = Some(&group.conflict_key_sha256);
    }
    Ok(())
}

pub(super) fn validate_jobs(
    jobs: &[GovernanceValidationJob],
    filter: &GovernanceValidationJobFilter,
) -> Result<(), GovernanceSemanticViewServiceError> {
    if jobs.len() > filter.limit {
        return Err(inconsistent());
    }
    let mut previous: Option<(i64, &str)> = None;
    for job in jobs {
        job.validate().map_err(|_| inconsistent())?;
        let ordered = previous.is_none_or(|(due_at, job_id)| {
            job.due_at_unix_ms > due_at
                || (job.due_at_unix_ms == due_at && job.job_id.as_bytes() > job_id.as_bytes())
        });
        if job.evaluated_at_unix_ms != filter.as_of_unix_ms
            || (filter.due_only && !job.due)
            || !ordered
        {
            return Err(inconsistent());
        }
        previous = Some((job.due_at_unix_ms, &job.job_id));
    }
    Ok(())
}
