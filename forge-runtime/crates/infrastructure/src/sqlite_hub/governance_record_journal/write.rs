use std::collections::BTreeSet;
use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use crate::runtime_domain::governance_contract::GovernanceRecord;
use crate::runtime_domain::{
    AppendGovernanceRecordBatch, AppendGovernanceRecordBatchResult,
    GOVERNANCE_RECORD_JOURNAL_VERSION, GovernanceRecordAppendDisposition,
    GovernanceRecordAppendReceipt, GovernanceStructuralHead, HubStoreError,
    validate_governance_record_append, validate_governance_record_relations,
    validate_governance_semantic_append,
};

use super::{closure, error, projection, rows, semantic, stored};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct DecodedStoredBatch {
    pub records: Vec<stored::DecodedRecord>,
    request: AppendGovernanceRecordBatch,
    receipt: GovernanceRecordAppendReceipt,
}

pub(super) fn append(
    connection: &mut Connection,
    request: &AppendGovernanceRecordBatch,
) -> Result<AppendGovernanceRecordBatchResult, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(error::read)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(error::read)?;
    let result = append_locked(&transaction, request)?;
    transaction.commit().map_err(error::read)?;
    Ok(result)
}

fn append_locked(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
) -> Result<AppendGovernanceRecordBatchResult, HubStoreError> {
    request
        .validate()
        .map_err(|problem| error::conflict(problem.message))?;
    if let Some(batch) = rows::find_batch_by_key(transaction, &request.idempotency_key)? {
        return exact_replay(transaction, request, &batch);
    }
    reject_batch_id_collision(transaction, request)?;
    let candidates = request
        .records()
        .map_err(|problem| error::conflict(problem.message))?;
    reject_record_collisions(transaction, &candidates)?;
    create(transaction, request, &candidates)
}

fn create(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
    candidates: &[GovernanceRecord],
) -> Result<AppendGovernanceRecordBatchResult, HubStoreError> {
    let heads = projection::heads_for_append(transaction, candidates)?;
    if heads.iter().any(|head| head.sequence == i64::MAX) {
        return Err(error::conflict(
            "governance aggregate sequence space is exhausted",
        ));
    }
    semantic::validate_prior_projections(transaction, &heads)?;
    let dependencies = closure::load(transaction, candidates, &heads)?;
    validate_governance_semantic_append(candidates, &dependencies, &heads)
        .map_err(|problem| error::conflict(problem.message))?;
    let next_heads = validate_governance_record_append(request, &dependencies, &heads)
        .map_err(|problem| error::conflict(problem.message))?;
    insert_batch(transaction, request, candidates.len())?;
    insert_records(transaction, request, candidates)?;
    upsert_heads(transaction, &next_heads)?;
    semantic::refresh_after_append(transaction, candidates, request.appended_at_ms)?;
    let stored = rows::find_batch_by_id(transaction, &request.batch_id)?
        .ok_or_else(|| error::corrupt("created governance append batch disappeared"))?;
    let (_, receipt) = decode_stored_batch(transaction, &stored)?;
    Ok(result(GovernanceRecordAppendDisposition::Stored, receipt))
}

fn exact_replay(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
    raw: &rows::RawBatch,
) -> Result<AppendGovernanceRecordBatchResult, HubStoreError> {
    let (stored_request, receipt) = decode_stored_batch(transaction, raw)?;
    let exact = stored_request.batch_id == request.batch_id
        && stored_request.request_sha256 == request.request_sha256
        && stored_request.record_set_sha256 == request.record_set_sha256
        && stored_request.idempotency_key == request.idempotency_key
        && stored_request.canonical_record_set_json == request.canonical_record_set_json;
    if !exact {
        return Err(error::conflict(
            "governance idempotency key was reused with different exact bytes",
        ));
    }
    validate_replay_relations(transaction, &stored_request)?;
    Ok(result(
        GovernanceRecordAppendDisposition::ExactReplay,
        receipt,
    ))
}

fn validate_replay_relations(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
) -> Result<(), HubStoreError> {
    let candidates = request
        .records()
        .map_err(|problem| error::corrupt(problem.message))?;
    projection::heads_for_append(transaction, &candidates)?;
    let dependencies = closure::load_stored(transaction, &candidates, &[])?;
    validate_governance_record_relations(request, &dependencies)
        .map_err(|problem| error::corrupt(problem.message))?;
    semantic::validate_current_for_candidates(transaction, &candidates)
}

pub(super) fn decode_stored_batch(
    connection: &Connection,
    raw: &rows::RawBatch,
) -> Result<(AppendGovernanceRecordBatch, GovernanceRecordAppendReceipt), HubStoreError> {
    let decoded = decode_stored_batch_full(connection, raw)?;
    Ok((decoded.request, decoded.receipt))
}

pub(super) fn decode_stored_batch_full(
    connection: &Connection,
    raw: &rows::RawBatch,
) -> Result<DecodedStoredBatch, HubStoreError> {
    let stored_records = rows::records_for_batch(connection, &raw.batch_id)?;
    let decoded = decode_batch_records(stored_records, &raw.batch_id)?;
    let canonical = format!(
        "[{}]",
        decoded
            .iter()
            .map(|record| record
                .inspection
                .canonical_record_json
                .as_deref()
                .unwrap_or_default())
            .collect::<Vec<_>>()
            .join(",")
    );
    let appended_at = error::stored_u64(raw.appended_at_ms, "batch append time")?;
    let request = AppendGovernanceRecordBatch::from_canonical_record_set(
        canonical,
        raw.idempotency_key.clone(),
        appended_at,
    )
    .map_err(|problem| error::corrupt(problem.message))?;
    validate_batch_columns(raw, &request, decoded.len())?;
    let receipt = receipt(&request, &decoded)?;
    Ok(DecodedStoredBatch {
        records: decoded,
        request,
        receipt,
    })
}

pub(super) fn validate_stored_batch_once(
    connection: &Connection,
    batch_id: &str,
    validated: &mut BTreeSet<String>,
) -> Result<(), HubStoreError> {
    if validated.contains(batch_id) {
        return Ok(());
    }
    let batch = rows::find_batch_by_id(connection, batch_id)?
        .ok_or_else(|| error::corrupt("governance record has no owning append batch"))?;
    decode_stored_batch(connection, &batch)?;
    validated.insert(batch_id.to_owned());
    Ok(())
}

fn decode_batch_records(
    records: Vec<rows::RawRecord>,
    batch_id: &str,
) -> Result<Vec<stored::DecodedRecord>, HubStoreError> {
    let mut decoded = Vec::with_capacity(records.len());
    for (ordinal, raw) in records.into_iter().enumerate() {
        let record = stored::decoded(raw)?;
        let metadata = &record.inspection.metadata;
        if metadata.batch_id != batch_id || metadata.batch_ordinal != ordinal {
            return Err(error::corrupt(
                "stored governance batch record ordinals are not contiguous",
            ));
        }
        decoded.push(record);
    }
    Ok(decoded)
}

fn validate_batch_columns(
    raw: &rows::RawBatch,
    request: &AppendGovernanceRecordBatch,
    record_count: usize,
) -> Result<(), HubStoreError> {
    let request_digest = error::stored_digest(&raw.request_sha256, "request digest")?;
    let set_digest = error::stored_digest(&raw.record_set_sha256, "record-set digest")?;
    let expected_count = error::stored_usize(raw.record_count, "batch record count")?;
    let expected_bytes = error::stored_usize(raw.record_set_bytes, "record-set byte count")?;
    let exact = raw.journal_version == i64::from(GOVERNANCE_RECORD_JOURNAL_VERSION)
        && raw.batch_id == request.batch_id
        && request_digest == request.request_sha256
        && set_digest == request.record_set_sha256
        && expected_count == record_count
        && expected_bytes == request.canonical_record_set_json.len();
    exact
        .then_some(())
        .ok_or_else(|| error::corrupt("stored governance append batch metadata diverged"))
}

fn receipt(
    request: &AppendGovernanceRecordBatch,
    records: &[stored::DecodedRecord],
) -> Result<GovernanceRecordAppendReceipt, HubStoreError> {
    let receipt = GovernanceRecordAppendReceipt {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        batch_id: request.batch_id.clone(),
        request_sha256: request.request_sha256.clone(),
        record_set_sha256: request.record_set_sha256.clone(),
        record_count: records.len(),
        record_ids: records
            .iter()
            .map(|record| record.inspection.metadata.record_id.clone())
            .collect(),
        appended_at_ms: request.appended_at_ms,
    };
    receipt
        .validate()
        .map_err(|problem| error::corrupt(problem.message))?;
    Ok(receipt)
}

fn reject_batch_id_collision(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
) -> Result<(), HubStoreError> {
    let Some(raw) = rows::find_batch_by_id(transaction, &request.batch_id)? else {
        return Ok(());
    };
    decode_stored_batch(transaction, &raw)?;
    Err(error::conflict(
        "governance batch ID belongs to another idempotency key",
    ))
}

fn reject_record_collisions(
    transaction: &Transaction<'_>,
    records: &[GovernanceRecord],
) -> Result<(), HubStoreError> {
    let mut validated_batches = BTreeSet::new();
    for candidate in records {
        let metadata = candidate.metadata();
        if let Some(raw) = rows::find_record(transaction, &metadata.record_id, true)? {
            validate_stored_batch_once(transaction, &raw.batch_id, &mut validated_batches)?;
            let stored = stored::decoded(raw)?;
            let same = stored.record == *candidate;
            let message = if same {
                "governance record already belongs to another append batch"
            } else {
                "governance record ID is already bound to different exact bytes"
            };
            return Err(error::conflict(message));
        }
        let kind = crate::runtime_domain::GovernanceRecordKind::from(candidate);
        if let Some(raw) = rows::find_record_by_identity(
            transaction,
            kind,
            &metadata.aggregate_id,
            metadata.sequence,
            true,
        )? {
            validate_stored_batch_once(transaction, &raw.batch_id, &mut validated_batches)?;
            stored::decoded(raw)?;
            return Err(error::conflict(
                "governance aggregate sequence is already bound to another record",
            ));
        }
    }
    Ok(())
}

fn insert_batch(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
    record_count: usize,
) -> Result<(), HubStoreError> {
    let request_digest = error::digest_blob(&request.request_sha256, "request digest")?;
    let set_digest = error::digest_blob(&request.record_set_sha256, "record-set digest")?;
    transaction
        .execute(
            "INSERT INTO governance_record_append_batches(
               batch_id,journal_version,idempotency_key,request_sha256,record_set_sha256,
               record_count,record_set_bytes,appended_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                request.batch_id,
                i64::from(request.v),
                request.idempotency_key,
                request_digest.as_slice(),
                set_digest.as_slice(),
                error::input_i64(record_count, "record count")?,
                error::input_i64(request.canonical_record_set_json.len(), "record-set bytes")?,
                error::input_i64(request.appended_at_ms, "append time")?
            ],
        )
        .map_err(error::write)?;
    Ok(())
}

fn insert_records(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
    records: &[GovernanceRecord],
) -> Result<(), HubStoreError> {
    for (ordinal, record) in records.iter().enumerate() {
        insert_record(transaction, request, record, ordinal)?;
    }
    Ok(())
}

fn insert_record(
    transaction: &Transaction<'_>,
    request: &AppendGovernanceRecordBatch,
    record: &GovernanceRecord,
    ordinal: usize,
) -> Result<(), HubStoreError> {
    let metadata = record.metadata();
    let canonical = record
        .canonical_record_json()
        .map_err(|problem| error::conflict(problem.message))?;
    let digest = error::digest_blob(&record.integrity().canonical_sha256, "record digest")?;
    transaction
        .execute(
            "INSERT INTO governance_records(
               record_id,batch_id,batch_ordinal,record_kind,aggregate_id,sequence,
               canonical_sha256,canonical_record_blob,canonical_record_bytes,
               created_at_unix_ms,appended_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                metadata.record_id,
                request.batch_id,
                error::input_i64(ordinal, "batch ordinal")?,
                crate::runtime_domain::GovernanceRecordKind::from(record).as_str(),
                metadata.aggregate_id,
                metadata.sequence,
                digest.as_slice(),
                canonical.as_bytes(),
                error::input_i64(canonical.len(), "canonical record bytes")?,
                metadata.created_at_unix_ms,
                error::input_i64(request.appended_at_ms, "append time")?
            ],
        )
        .map_err(error::write)?;
    Ok(())
}

fn upsert_heads(
    transaction: &Transaction<'_>,
    heads: &[GovernanceStructuralHead],
) -> Result<(), HubStoreError> {
    for head in heads {
        let digest = error::digest_blob(&head.canonical_sha256, "head digest")?;
        transaction
            .execute(
                "INSERT INTO governance_structural_heads(
                   record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
                 ) VALUES(?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(record_kind,aggregate_id) DO UPDATE SET
                   record_id=excluded.record_id,sequence=excluded.sequence,
                   canonical_sha256=excluded.canonical_sha256,updated_at_ms=excluded.updated_at_ms",
                params![
                    head.record_kind.as_str(),
                    head.aggregate_id,
                    head.record_id,
                    head.sequence,
                    digest.as_slice(),
                    error::input_i64(head.updated_at_ms, "head update time")?
                ],
            )
            .map_err(error::write)?;
    }
    Ok(())
}

fn result(
    disposition: GovernanceRecordAppendDisposition,
    receipt: GovernanceRecordAppendReceipt,
) -> AppendGovernanceRecordBatchResult {
    AppendGovernanceRecordBatchResult {
        v: GOVERNANCE_RECORD_JOURNAL_VERSION,
        disposition,
        receipt,
    }
}
