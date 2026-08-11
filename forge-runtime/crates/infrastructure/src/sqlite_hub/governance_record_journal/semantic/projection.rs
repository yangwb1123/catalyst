use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    GovernanceRecordKind, GovernanceStructuralHead, HubStoreError, governance_semantic_projection,
    validate_governance_semantic_transition,
};

use super::super::{
    error, projection as structural, rows as journal_rows, stored as journal_stored,
};
use super::rows;

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn rebuild(connection: &mut Connection) -> Result<usize, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(error::read)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(error::read)?;
    let count = rebuild_locked(&transaction)?;
    transaction.commit().map_err(error::read)?;
    Ok(count)
}

pub(crate) fn rebuild_locked(connection: &Connection) -> Result<usize, HubStoreError> {
    let structural_count = structural::rebuild_locked(connection)?;
    let semantic_count = rebuild_materialized_locked(connection)?;
    if semantic_count != structural_count {
        return Err(error::corrupt(
            "semantic rebuild count diverges from structural rebuild",
        ));
    }
    Ok(semantic_count)
}

pub(crate) fn rebuild_materialized_locked(connection: &Connection) -> Result<usize, HubStoreError> {
    rows::clear(connection)?;
    let mut statement = connection
        .prepare(journal_rows::ALL_RECORDS_SQL)
        .map_err(error::read)?;
    let mut cursor = statement.query([]).map_err(error::read)?;
    let mut current: Option<GovernanceRecord> = None;
    let mut count = 0_usize;
    while let Some(row) = cursor.next().map_err(error::read)? {
        let raw = journal_rows::raw_record(row, true).map_err(error::read)?;
        let decoded = journal_stored::decoded(raw)?;
        if current
            .as_ref()
            .is_some_and(|prior| !same_aggregate(prior, &decoded.record))
        {
            flush(connection, current.take().as_ref(), &mut count)?;
        }
        validate_governance_semantic_transition(current.as_ref(), &decoded.record)
            .map_err(|problem| error::corrupt(problem.message))?;
        current = Some(decoded.record);
    }
    flush(connection, current.as_ref(), &mut count)?;
    validate_cardinality(connection, count)?;
    super::integrity::validate_global_parity_unbounded(connection)?;
    Ok(count)
}

pub(crate) fn validate_prior_projections(
    connection: &Connection,
    heads: &[GovernanceStructuralHead],
) -> Result<(), HubStoreError> {
    let mut verifier = super::integrity::IntegrityVerifier::scan(connection);
    for head in heads {
        verifier.projection(connection, head.record_kind, &head.aggregate_id)?;
    }
    super::integrity::validate_global_parity_with(connection, &mut verifier)?;
    Ok(())
}

pub(crate) fn refresh_after_append(
    connection: &Connection,
    candidates: &[GovernanceRecord],
    updated_at_ms: u64,
) -> Result<(), HubStoreError> {
    let mut latest: BTreeMap<(GovernanceRecordKind, &str), &GovernanceRecord> = BTreeMap::new();
    for record in candidates {
        let key = (
            GovernanceRecordKind::from(record),
            record.metadata().aggregate_id.as_str(),
        );
        if latest
            .get(&key)
            .is_none_or(|prior| prior.metadata().sequence < record.metadata().sequence)
        {
            latest.insert(key, record);
        }
    }
    for record in latest.into_values() {
        let projection = governance_semantic_projection(record, updated_at_ms)
            .map_err(|problem| error::conflict(problem.message))?;
        rows::replace_projection(connection, &projection)?;
    }
    Ok(())
}

pub(crate) fn validate_current_for_candidates(
    connection: &Connection,
    candidates: &[GovernanceRecord],
) -> Result<(), HubStoreError> {
    let keys: BTreeSet<_> = candidates
        .iter()
        .map(|record| {
            (
                GovernanceRecordKind::from(record),
                record.metadata().aggregate_id.as_str(),
            )
        })
        .collect();
    let mut verifier = super::integrity::IntegrityVerifier::scan(connection);
    for (kind, aggregate_id) in keys {
        verifier.projection(connection, kind, aggregate_id)?;
    }
    super::integrity::validate_global_parity_with(connection, &mut verifier)
}

fn flush(
    connection: &Connection,
    record: Option<&GovernanceRecord>,
    count: &mut usize,
) -> Result<(), HubStoreError> {
    let Some(record) = record else {
        return Ok(());
    };
    let metadata = record.metadata();
    let kind = GovernanceRecordKind::from(record);
    let mut batches = BTreeSet::new();
    let head = structural::exact_head(connection, kind, &metadata.aggregate_id, &mut batches)?
        .ok_or_else(|| error::corrupt("semantic aggregate has no structural head"))?;
    if head.record_id != metadata.record_id || head.sequence != metadata.sequence {
        return Err(error::corrupt(
            "semantic aggregate tail diverges from structural head",
        ));
    }
    let projection = governance_semantic_projection(record, head.updated_at_ms)
        .map_err(|problem| error::corrupt(problem.message))?;
    rows::replace_projection(connection, &projection)?;
    *count = count
        .checked_add(1)
        .ok_or_else(|| error::corrupt("semantic projection count overflowed"))?;
    Ok(())
}

fn validate_cardinality(connection: &Connection, count: usize) -> Result<(), HubStoreError> {
    let projected = rows::semantic_head_count(connection)?;
    let structural = rows::structural_head_count(connection)?;
    let expected = i64::try_from(count)
        .map_err(|problem| error::corrupt(format!("semantic projection count: {problem}")))?;
    if projected == expected && structural == expected {
        Ok(())
    } else {
        Err(error::corrupt(
            "semantic and structural projection cardinality diverged",
        ))
    }
}

fn same_aggregate(left: &GovernanceRecord, right: &GovernanceRecord) -> bool {
    GovernanceRecordKind::from(left) == GovernanceRecordKind::from(right)
        && left.metadata().aggregate_id == right.metadata().aggregate_id
}
