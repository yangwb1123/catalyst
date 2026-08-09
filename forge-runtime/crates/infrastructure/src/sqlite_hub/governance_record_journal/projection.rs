use std::collections::BTreeSet;
use std::time::Duration;

use rusqlite::{Connection, TransactionBehavior};

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    AppendGovernanceRecordBatch, GovernanceRecordKind, GovernanceStructuralHead, HubStoreError,
    is_governance_record_identifier, validate_governance_record_append,
};

use super::{error, rows, stored};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn inspect(
    connection: &mut Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<GovernanceStructuralHead, HubStoreError> {
    validate_aggregate_input(aggregate_id)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(error::read)?;
    let head = exact_head(&transaction, kind, aggregate_id, &mut BTreeSet::new())?
        .ok_or_else(|| error::not_found(format!("{}:{aggregate_id}", kind.as_str())))?;
    transaction.commit().map_err(error::read)?;
    Ok(head)
}

pub(super) fn heads_for_append(
    connection: &Connection,
    candidates: &[GovernanceRecord],
) -> Result<Vec<GovernanceStructuralHead>, HubStoreError> {
    let keys: BTreeSet<_> = candidates
        .iter()
        .map(|record| {
            (
                GovernanceRecordKind::from(record),
                record.metadata().aggregate_id.clone(),
            )
        })
        .collect();
    let mut heads = Vec::new();
    let mut validated_batches = BTreeSet::new();
    for (kind, aggregate_id) in keys {
        if let Some(head) = exact_head(connection, kind, &aggregate_id, &mut validated_batches)? {
            heads.push(head);
        }
    }
    Ok(heads)
}

pub(super) fn rebuild(connection: &mut Connection) -> Result<usize, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(error::read)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(error::read)?;
    validate_durable_batches(&transaction)?;
    rows::prepare_rebuilt_heads(&transaction)?;
    scan_rebuilt_heads(&transaction)?;
    let count = rows::replace_heads_from_rebuild(&transaction)?;
    transaction.commit().map_err(error::read)?;
    Ok(count)
}

fn validate_durable_batches(connection: &Connection) -> Result<(), HubStoreError> {
    let mut after = None;
    loop {
        let Some(batch) = rows::next_batch(connection, after.as_deref())? else {
            return Ok(());
        };
        super::write::decode_stored_batch(connection, &batch)?;
        after = Some(batch.batch_id);
    }
}

fn exact_head(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
    validated_batches: &mut BTreeSet<String>,
) -> Result<Option<GovernanceStructuralHead>, HubStoreError> {
    // Integrity-first range scan: COUNT/MIN/MAX catches a missing middle sequence that an O(1)
    // projection lookup cannot. ADR-0046 records the bounded-aggregate capacity assumption and
    // the trigger for replacing this with a versioned accumulator plus independent full audits.
    let summary = rows::aggregate_summary(connection, kind, aggregate_id)?;
    let raw_head = rows::find_head(connection, kind, aggregate_id)?;
    if summary.count == 0 {
        return match raw_head {
            None => Ok(None),
            Some(_) => Err(error::corrupt("structural head has no immutable record")),
        };
    }
    let head = raw_head
        .ok_or_else(|| error::corrupt("immutable aggregate is missing its structural head"))?;
    validate_sequence_summary(summary)?;
    let head = stored::head(head)?;
    if Some(head.sequence) != summary.maximum_sequence {
        return Err(error::corrupt("structural head is stale"));
    }
    validate_head_record(connection, head, kind, aggregate_id, validated_batches).map(Some)
}

fn validate_sequence_summary(summary: rows::AggregateSummary) -> Result<(), HubStoreError> {
    let contiguous = summary.minimum_sequence == Some(1)
        && summary.maximum_sequence == Some(summary.count)
        && summary.count > 0;
    contiguous
        .then_some(())
        .ok_or_else(|| error::corrupt("immutable governance aggregate sequence is not contiguous"))
}

fn validate_head_record(
    connection: &Connection,
    head: GovernanceStructuralHead,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
    validated_batches: &mut BTreeSet<String>,
) -> Result<GovernanceStructuralHead, HubStoreError> {
    let raw = rows::find_record_by_identity(connection, kind, aggregate_id, head.sequence, true)?
        .ok_or_else(|| error::corrupt("structural head record is missing"))?;
    super::write::validate_stored_batch_once(connection, &raw.batch_id, validated_batches)?;
    let record = stored::decoded(raw)?;
    let metadata = record.inspection.metadata;
    let exact = metadata.record_id == head.record_id
        && metadata.canonical_sha256 == head.canonical_sha256
        && metadata.appended_at_ms == head.updated_at_ms;
    exact
        .then_some(head)
        .ok_or_else(|| error::corrupt("structural head diverges from its immutable record"))
}

fn scan_rebuilt_heads(connection: &Connection) -> Result<(), HubStoreError> {
    let mut statement = connection
        .prepare(rows::ALL_RECORDS_SQL)
        .map_err(error::read)?;
    let mut cursor = statement.query([]).map_err(error::read)?;
    let mut builder = ProjectionBuilder::default();
    while let Some(row) = cursor.next().map_err(error::read)? {
        let raw = rows::raw_record(row, true).map_err(error::read)?;
        builder.push(connection, &stored::decoded(raw)?)?;
    }
    builder.finish(connection)
}

#[derive(Default)]
struct ProjectionBuilder {
    head: Option<GovernanceStructuralHead>,
}

impl ProjectionBuilder {
    fn push(
        &mut self,
        connection: &Connection,
        decoded: &stored::DecodedRecord,
    ) -> Result<(), HubStoreError> {
        if self
            .head
            .as_ref()
            .is_some_and(|head| !same_aggregate(head, &decoded.record))
        {
            self.flush(connection)?;
        }
        let current = self.head.clone();
        if current
            .as_ref()
            .is_some_and(|head| head.sequence == i64::MAX)
        {
            return Err(error::corrupt("stored governance sequence overflowed"));
        }
        let request = rebuild_request(decoded)?;
        let heads: Vec<_> = current.iter().cloned().collect();
        let dependencies =
            super::closure::load_stored(connection, std::slice::from_ref(&decoded.record), &heads)?;
        let mut next = validate_governance_record_append(&request, &dependencies, &heads)
            .map_err(|problem| error::corrupt(problem.message))?;
        if next.len() != 1 {
            return Err(error::corrupt("rebuild produced an inexact head set"));
        }
        self.head = next.pop();
        Ok(())
    }

    fn flush(&mut self, connection: &Connection) -> Result<(), HubStoreError> {
        if let Some(head) = self.head.take() {
            rows::insert_rebuilt_head(connection, &head)?;
        }
        Ok(())
    }

    fn finish(mut self, connection: &Connection) -> Result<(), HubStoreError> {
        self.flush(connection)
    }
}

fn same_aggregate(head: &GovernanceStructuralHead, record: &GovernanceRecord) -> bool {
    let metadata = record.metadata();
    head.record_kind == GovernanceRecordKind::from(record)
        && head.aggregate_id == metadata.aggregate_id
}

fn rebuild_request(
    decoded: &stored::DecodedRecord,
) -> Result<AppendGovernanceRecordBatch, HubStoreError> {
    let canonical = decoded
        .inspection
        .canonical_record_json
        .as_deref()
        .ok_or_else(|| error::corrupt("rebuild did not load canonical record bytes"))?;
    AppendGovernanceRecordBatch::from_canonical_record_set(
        format!("[{canonical}]"),
        format!("rebuild:{}", decoded.inspection.metadata.record_id),
        decoded.inspection.metadata.appended_at_ms,
    )
    .map_err(|problem| error::corrupt(problem.message))
}

fn validate_aggregate_input(aggregate_id: &str) -> Result<(), HubStoreError> {
    if is_governance_record_identifier(aggregate_id) {
        Ok(())
    } else {
        Err(error::conflict("governance aggregate ID is invalid"))
    }
}
