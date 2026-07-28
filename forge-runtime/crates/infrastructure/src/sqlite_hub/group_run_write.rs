use std::time::Duration;

use crate::runtime_domain::{
    GROUP_RUN_VERSION, GroupRunRecord, GroupRunSnapshot, GroupRunStatus, HubEntity, HubStoreError,
    PrepareGroupRun, PrepareGroupRunDisposition, PrepareGroupRunResult,
};
use rusqlite::{Connection, Transaction, TransactionBehavior, params};

use super::{
    group_context_read::{load_in_snapshot, validate_policy},
    group_run_codec::{encode, encode_hex_digest, valid_text},
    group_run_read::{decode_stored, find_by_id, find_by_key},
    read_error, write_error,
};

const GROUP_RUN_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_ID_BYTES: usize = 128;
const MAX_KEY_BYTES: usize = 256;

pub(super) fn prepare(
    connection: &mut Connection,
    request: &PrepareGroupRun,
) -> Result<PrepareGroupRunResult, HubStoreError> {
    validate_request(request)?;
    connection
        .busy_timeout(GROUP_RUN_BUSY_TIMEOUT)
        .map_err(read_error)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)?;
    let result = prepare_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

fn prepare_locked(
    transaction: &Transaction<'_>,
    request: &PrepareGroupRun,
) -> Result<PrepareGroupRunResult, HubStoreError> {
    if let Some(stored) = find_by_key(transaction, &request.idempotency_key)? {
        if stored.idempotency_key != request.idempotency_key {
            return Err(HubStoreError::Corrupt {
                message: "Group Run idempotency lookup returned a different key".into(),
            });
        }
        let snapshot = decode_stored(stored)?;
        ensure_idempotent_replay(&snapshot, request)?;
        return Ok(result(PrepareGroupRunDisposition::Replayed, snapshot));
    }
    if let Some(stored) = find_by_id(transaction, &request.run_id)? {
        decode_stored(stored)?;
        return Err(conflict(
            "Group Run ID already belongs to another idempotency key",
        ));
    }
    let context = load_in_snapshot(transaction, &request.group_id, &request.policy)?;
    let encoded = encode(&context)?;
    let record = record_from(request, &context, &encoded);
    insert(transaction, &record, request, &encoded)?;
    let context_json =
        String::from_utf8(encoded.bytes).map_err(|error| HubStoreError::Corrupt {
            message: format!("generated Group Run snapshot is not UTF-8: {error}"),
        })?;
    let snapshot = GroupRunSnapshot {
        v: GROUP_RUN_VERSION,
        run: record,
        context,
        context_json,
    };
    Ok(result(PrepareGroupRunDisposition::Created, snapshot))
}

fn validate_request(request: &PrepareGroupRun) -> Result<(), HubStoreError> {
    if request.v != GROUP_RUN_VERSION {
        return Err(conflict("unsupported Group Run request version"));
    }
    for (value, max, subject) in [
        (request.run_id.as_str(), MAX_ID_BYTES, "Group Run ID"),
        (request.group_id.as_str(), MAX_ID_BYTES, "Group ID"),
        (
            request.idempotency_key.as_str(),
            MAX_KEY_BYTES,
            "idempotency key",
        ),
    ] {
        if !valid_text(value, max) {
            return Err(conflict(&format!("{subject} is outside its byte bounds")));
        }
    }
    i64::try_from(request.created_at_ms)
        .map_err(|_| conflict("Group Run creation time exceeds SQLite bounds"))?;
    validate_policy(&request.policy)
}

fn ensure_idempotent_replay(
    snapshot: &GroupRunSnapshot,
    request: &PrepareGroupRun,
) -> Result<(), HubStoreError> {
    if snapshot.run.group_id == request.group_id
        && snapshot.context.payload.policy == request.policy
    {
        return Ok(());
    }
    Err(conflict(
        "idempotency key was reused with different Group Run input",
    ))
}

fn record_from(
    request: &PrepareGroupRun,
    context: &crate::runtime_domain::GroupContextSlice,
    encoded: &super::group_run_codec::EncodedGroupRunSnapshot,
) -> GroupRunRecord {
    GroupRunRecord {
        v: GROUP_RUN_VERSION,
        run_id: request.run_id.clone(),
        group_id: request.group_id.clone(),
        status: GroupRunStatus::Prepared,
        context_version: context.v,
        context_slice_sha256: context.slice_sha256.clone(),
        snapshot_sha256: encode_hex_digest(&encoded.snapshot_digest),
        snapshot_bytes: encoded.bytes.len(),
        created_at_ms: request.created_at_ms,
    }
}

fn insert(
    transaction: &Transaction<'_>,
    record: &GroupRunRecord,
    request: &PrepareGroupRun,
    encoded: &super::group_run_codec::EncodedGroupRunSnapshot,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_runs(
               id,group_id,run_version,status,context_version,context_slice_sha256,
               context_blob,snapshot_sha256,idempotency_key,created_at_ms
             ) VALUES(?1,?2,?3,'prepared',?4,?5,?6,?7,?8,?9)",
            params![
                record.run_id,
                record.group_id,
                i64::from(record.v),
                i64::from(record.context_version),
                encoded.context_digest.as_slice(),
                encoded.bytes.as_slice(),
                encoded.snapshot_digest.as_slice(),
                request.idempotency_key.as_str(),
                i64::try_from(record.created_at_ms)
                    .map_err(|_| conflict("invalid Group Run creation time"))?
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupRun, error))?;
    Ok(())
}

fn result(
    disposition: PrepareGroupRunDisposition,
    snapshot: GroupRunSnapshot,
) -> PrepareGroupRunResult {
    PrepareGroupRunResult {
        v: GROUP_RUN_VERSION,
        disposition,
        snapshot,
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupRun,
        message: message.into(),
    }
}
