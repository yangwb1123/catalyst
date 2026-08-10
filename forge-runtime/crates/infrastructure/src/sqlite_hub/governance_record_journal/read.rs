use std::collections::BTreeSet;

use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::{
    GovernanceRecordInspection, GovernanceRecordListFilter, HubStoreError,
    is_governance_record_identifier,
};

use super::{error, rows, stored};

pub(super) fn inspect(
    connection: &mut Connection,
    record_id: &str,
    include_record: bool,
) -> Result<GovernanceRecordInspection, HubStoreError> {
    if !is_governance_record_identifier(record_id) {
        return Err(error::conflict("governance record ID is invalid"));
    }
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(error::read)?;
    let raw = rows::find_record(&transaction, record_id, true)?
        .ok_or_else(|| error::not_found(record_id))?;
    super::write::validate_stored_batch_once(&transaction, &raw.batch_id, &mut BTreeSet::new())?;
    let inspection = stored::validated_inspection(raw, include_record)?;
    transaction.commit().map_err(error::read)?;
    Ok(inspection)
}

pub(super) fn list(
    connection: &mut Connection,
    filter: &GovernanceRecordListFilter,
) -> Result<Vec<GovernanceRecordInspection>, HubStoreError> {
    filter
        .validate()
        .map_err(|problem| error::conflict(problem.message))?;
    let limit = error::input_i64(filter.limit, "governance list limit")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(error::read)?;
    let raw_records = rows::list_records(
        &transaction,
        filter.record_kind,
        filter.aggregate_id.as_deref(),
        limit,
        true,
    )?;
    let mut batches = BTreeSet::new();
    let mut records = Vec::with_capacity(raw_records.len());
    for raw in raw_records {
        super::write::validate_stored_batch_once(&transaction, &raw.batch_id, &mut batches)?;
        records.push(stored::validated_inspection(raw, filter.include_record)?);
    }
    transaction.commit().map_err(error::read)?;
    Ok(records)
}
