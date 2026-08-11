use serde::Serialize;

use crate::governance_contract::ClaimType;

use super::{
    GOVERNANCE_CLAIM_CONFLICT_KEY_DIGEST_DOMAIN, GOVERNANCE_VALIDATION_JOB_DIGEST_DOMAIN,
    GOVERNANCE_VALIDATION_JOB_ID_PREFIX, GovernanceRecordJournalError, digest_hex, invalid,
};

#[derive(Serialize)]
struct ClaimConflictKey<'a> {
    claim_type: ClaimType,
    project_id: &'a str,
    scope: &'a str,
    subject: &'a str,
    predicate: &'a str,
}

pub(super) fn claim_conflict_key_sha256(
    claim_type: ClaimType,
    project_id: &str,
    scope: &str,
    subject: &str,
    predicate: &str,
) -> Result<String, GovernanceRecordJournalError> {
    let key = ClaimConflictKey {
        claim_type,
        project_id,
        scope,
        subject,
        predicate,
    };
    canonical_digest(GOVERNANCE_CLAIM_CONFLICT_KEY_DIGEST_DOMAIN, &key)
}

pub(super) fn validation_job_id(
    record_id: &str,
    validation_plan_sha256: &str,
) -> Result<String, GovernanceRecordJournalError> {
    let record_id = record_id.as_bytes();
    let length = u64::try_from(record_id.len())
        .map_err(|_| invalid("validation-job record identity is too long"))?;
    let digest = digest_hex(
        GOVERNANCE_VALIDATION_JOB_DIGEST_DOMAIN,
        &[
            &length.to_be_bytes(),
            record_id,
            validation_plan_sha256.as_bytes(),
        ],
    );
    Ok(format!("{GOVERNANCE_VALIDATION_JOB_ID_PREFIX}{digest}"))
}

pub(super) fn canonical_digest(
    domain: &[u8],
    value: &impl Serialize,
) -> Result<String, GovernanceRecordJournalError> {
    let canonical = crate::governance_contract::codec::canonical_json(value)
        .map_err(|problem| invalid(problem.message))?;
    Ok(digest_hex(domain, &[canonical.as_bytes()]))
}
