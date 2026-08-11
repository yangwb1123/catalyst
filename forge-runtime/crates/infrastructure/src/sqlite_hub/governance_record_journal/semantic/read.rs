use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::{
    GovernanceClaimConflictGroup, GovernanceRecordKind, GovernanceSemanticListFilter,
    GovernanceSemanticProjection, GovernanceValidationJob, GovernanceValidationJobFilter,
    HubStoreError, MAX_GOVERNANCE_CONFLICT_MEMBERS, MAX_GOVERNANCE_SEMANTIC_SCAN_RECORDS,
    governance_claim_conflict_groups, governance_validation_job, is_governance_record_identifier,
};

use super::super::error;
use super::{integrity, rows, stored};

pub(super) fn inspect(
    connection: &mut Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<GovernanceSemanticProjection, HubStoreError> {
    validate_aggregate(aggregate_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(error::read)?;
    let mut verifier = integrity::IntegrityVerifier::scan(&transaction);
    let projection = verifier.projection(&transaction, kind, aggregate_id)?;
    integrity::validate_global_parity_with(&transaction, &mut verifier)?;
    transaction.commit().map_err(error::read)?;
    Ok(projection)
}

pub(super) fn conflicts(
    connection: &mut Connection,
    filter: &GovernanceSemanticListFilter,
) -> Result<Vec<GovernanceClaimConflictGroup>, HubStoreError> {
    filter
        .validate()
        .map_err(|problem| error::conflict(problem.message))?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(error::read)?;
    let (projections, mut verifier) = all_claim_projections(&transaction)?;
    verifier.batches_spend(ordered_work(projections.len())?)?;
    validate_conflict_group_capacity(&projections, filter.as_of_unix_ms)?;
    verifier.batches_spend(ordered_work(projections.len())?)?;
    let mut groups = governance_claim_conflict_groups(projections, filter.as_of_unix_ms)
        .map_err(|problem| error::corrupt(problem.message))?;
    groups.truncate(filter.limit);
    drop(verifier);
    transaction.commit().map_err(error::read)?;
    Ok(groups)
}

pub(super) fn validation_jobs(
    connection: &mut Connection,
    filter: &GovernanceValidationJobFilter,
) -> Result<Vec<GovernanceValidationJob>, HubStoreError> {
    filter
        .validate()
        .map_err(|problem| error::conflict(problem.message))?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(error::read)?;
    let (projections, mut verifier) = all_claim_projections(&transaction)?;
    verifier.batches_spend(projections.len())?;
    let mut jobs = Vec::new();
    for projection in projections {
        let expected = governance_validation_job(&projection, filter.as_of_unix_ms)
            .map_err(|problem| error::corrupt(problem.message))?;
        let raw = rows::find_validation_job(&transaction, &projection.head.aggregate_id)?;
        match (expected, raw) {
            (Some(job), Some(raw)) => {
                stored::validate_job(&raw, &job, &projection.projection_sha256)?;
                if !filter.due_only || job.due {
                    jobs.push(job);
                }
            }
            (None, None) => {}
            _ => {
                return Err(error::corrupt(
                    "validation-job materialization is incomplete",
                ));
            }
        }
    }
    verifier.batches_spend(ordered_work(jobs.len())?)?;
    jobs.sort_by(|left, right| {
        left.due_at_unix_ms
            .cmp(&right.due_at_unix_ms)
            .then_with(|| left.job_id.as_bytes().cmp(right.job_id.as_bytes()))
    });
    jobs.truncate(filter.limit);
    drop(verifier);
    transaction.commit().map_err(error::read)?;
    Ok(jobs)
}

fn all_claim_projections(
    connection: &Connection,
) -> Result<
    (
        Vec<GovernanceSemanticProjection>,
        integrity::IntegrityVerifier<'_>,
    ),
    HubStoreError,
> {
    let query_limit = i64::try_from(MAX_GOVERNANCE_SEMANTIC_SCAN_RECORDS + 1)
        .map_err(|problem| error::corrupt(format!("semantic scan bound: {problem}")))?;
    let ids = rows::immutable_aggregate_ids(
        connection,
        GovernanceRecordKind::KnowledgeClaim,
        query_limit,
    )?;
    if ids.len() > MAX_GOVERNANCE_SEMANTIC_SCAN_RECORDS {
        return Err(HubStoreError::Unavailable {
            message: "governance semantic scan exceeds the v1 local bound".into(),
        });
    }
    let mut verifier = integrity::IntegrityVerifier::scan(connection);
    verifier.batches_spend(ids.len())?;
    integrity::validate_global_parity_with(connection, &mut verifier)?;
    let mut projections = Vec::with_capacity(ids.len());
    for aggregate_id in ids {
        projections.push(verifier.projection(
            connection,
            GovernanceRecordKind::KnowledgeClaim,
            &aggregate_id,
        )?);
    }
    Ok((projections, verifier))
}

fn ordered_work(len: usize) -> Result<usize, HubStoreError> {
    let levels = if len < 2 {
        1
    } else {
        usize::try_from(len.ilog2() + 1)
            .map_err(|problem| error::corrupt(format!("semantic ordering work: {problem}")))?
    };
    len.checked_mul(levels).ok_or(HubStoreError::Unavailable {
        message: "governance semantic ordering work overflowed".into(),
    })
}

fn validate_conflict_group_capacity(
    projections: &[GovernanceSemanticProjection],
    as_of_unix_ms: i64,
) -> Result<(), HubStoreError> {
    let mut groups: BTreeMap<&str, (usize, BTreeSet<&str>)> = BTreeMap::new();
    for projection in projections {
        let active = as_of_unix_ms >= projection.head.valid_from_unix_ms
            && projection
                .head
                .valid_until_unix_ms
                .is_none_or(|until| as_of_unix_ms < until);
        if !active {
            continue;
        }
        let claim = projection
            .claim
            .as_ref()
            .ok_or_else(|| error::corrupt("conflict candidate is not a claim projection"))?;
        let entry = groups
            .entry(&claim.conflict_key_sha256)
            .or_insert_with(|| (0, BTreeSet::new()));
        entry.0 += 1;
        entry.1.insert(&claim.object_sha256);
    }
    if groups
        .values()
        .any(|(members, objects)| *members > MAX_GOVERNANCE_CONFLICT_MEMBERS && objects.len() > 1)
    {
        Err(HubStoreError::Unavailable {
            message: "governance conflict group exceeds the v1 public member bound".into(),
        })
    } else {
        Ok(())
    }
}

fn validate_aggregate(aggregate_id: &str) -> Result<(), HubStoreError> {
    if is_governance_record_identifier(aggregate_id) {
        Ok(())
    } else {
        Err(error::conflict("governance aggregate ID is invalid"))
    }
}
