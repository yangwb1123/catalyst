use crate::runtime_domain::{
    GroupRunRecord, GroupRunSnapshot, GroupRunStatus, HubEntity, HubStoreError,
    MAX_GROUP_RUN_LIST_LIMIT,
};
use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{
    group_run_codec::{decode, encode_hex_digest, valid_text, validate_record_metadata},
    read_error,
};

const RECORD_COLUMNS: &str = "id,group_id,run_version,status,context_version,\
 context_slice_sha256,snapshot_sha256,length(context_blob),created_at_ms";
const STORED_COLUMNS: &str = "id,group_id,run_version,status,context_version,\
 context_slice_sha256,snapshot_sha256,length(context_blob),created_at_ms,\
 idempotency_key,context_blob";
const MAX_KEY_BYTES: usize = 256;

pub(super) struct StoredGroupRun {
    pub record: GroupRunRecord,
    pub idempotency_key: String,
    pub context_blob: Vec<u8>,
}

pub(super) fn inspect(
    connection: &Connection,
    run_id: &str,
) -> Result<GroupRunSnapshot, HubStoreError> {
    let stored =
        find_by_id(connection, run_id)?.ok_or_else(|| not_found(HubEntity::GroupRun, run_id))?;
    decode_stored(stored)
}

pub(super) fn list(
    connection: &Connection,
    group_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupRunRecord>, HubStoreError> {
    validate_list_limit(limit)?;
    if let Some(id) = group_id {
        ensure_group_exists(connection, id)?;
    }
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    match group_id {
        Some(id) => query_records(
            connection,
            "WHERE group_id = ?1 ORDER BY created_at_ms DESC,id DESC LIMIT ?2",
            params![id, limit],
        ),
        None => query_records(
            connection,
            "ORDER BY created_at_ms DESC,id DESC LIMIT ?1",
            [limit],
        ),
    }
}

pub(super) fn find_by_id(
    connection: &Connection,
    run_id: &str,
) -> Result<Option<StoredGroupRun>, HubStoreError> {
    query_stored(connection, "id", run_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<StoredGroupRun>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn decode_stored(stored: StoredGroupRun) -> Result<GroupRunSnapshot, HubStoreError> {
    if !valid_text(&stored.idempotency_key, MAX_KEY_BYTES) {
        return Err(HubStoreError::Corrupt {
            message: "stored Group Run idempotency key violates its byte bounds".into(),
        });
    }
    decode(stored.record, stored.context_blob)
}

fn query_stored(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<StoredGroupRun>, HubStoreError> {
    let sql = match column {
        "id" => format!("SELECT {STORED_COLUMNS} FROM group_runs WHERE id = ?1"),
        "idempotency_key" => {
            format!("SELECT {STORED_COLUMNS} FROM group_runs WHERE idempotency_key = ?1")
        }
        _ => return Err(conflict("unsupported Group Run lookup")),
    };
    connection
        .query_row(&sql, [value], stored_row)
        .optional()
        .map_err(read_error)
}

fn query_records<P>(
    connection: &Connection,
    suffix: &str,
    parameters: P,
) -> Result<Vec<GroupRunRecord>, HubStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection
        .prepare(&format!("SELECT {RECORD_COLUMNS} FROM group_runs {suffix}"))
        .map_err(read_error)?;
    statement
        .query_map(parameters, record_row)
        .map_err(read_error)?
        .map(|row| {
            let record = row.map_err(read_error)?;
            validate_record_metadata(&record)?;
            Ok(record)
        })
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<StoredGroupRun> {
    Ok(StoredGroupRun {
        record: record_row(row)?,
        idempotency_key: row.get(9)?,
        context_blob: row.get(10)?,
    })
}

fn record_row(row: &Row<'_>) -> rusqlite::Result<GroupRunRecord> {
    let status_value = row.get::<_, String>(3)?;
    let status = parse_status(&status_value)?;
    let context_digest = parse_digest(row, 5)?;
    let snapshot_digest = parse_digest(row, 6)?;
    Ok(GroupRunRecord {
        v: convert(row, 2, "Group Run version")?,
        run_id: row.get(0)?,
        group_id: row.get(1)?,
        status,
        context_version: convert(row, 4, "Group context version")?,
        context_slice_sha256: context_digest,
        snapshot_sha256: snapshot_digest,
        snapshot_bytes: convert(row, 7, "Group Run snapshot byte count")?,
        created_at_ms: convert(row, 8, "Group Run creation time")?,
    })
}

fn parse_status(value: &str) -> rusqlite::Result<GroupRunStatus> {
    match value {
        "prepared" => Ok(GroupRunStatus::Prepared),
        _ => Err(conversion_error(3, "unsupported Group Run status")),
    }
}

fn parse_digest(row: &Row<'_>, index: usize) -> rusqlite::Result<String> {
    let value: Vec<u8> = row.get(index)?;
    let digest: [u8; 32] = value
        .try_into()
        .map_err(|_| conversion_error(index, "stored digest must contain 32 bytes"))?;
    Ok(encode_hex_digest(&digest))
}

fn convert<T>(row: &Row<'_>, index: usize, subject: &str) -> rusqlite::Result<T>
where
    T: TryFrom<i64>,
    T::Error: std::error::Error + Send + Sync + 'static,
{
    T::try_from(row.get::<_, i64>(index)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid {subject}: {error}"),
            )),
        )
    })
}

fn ensure_group_exists(connection: &Connection, id: &str) -> Result<(), HubStoreError> {
    connection
        .query_row("SELECT 1 FROM groups WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::Group, id))
}

fn validate_list_limit(limit: usize) -> Result<(), HubStoreError> {
    if (1..=MAX_GROUP_RUN_LIST_LIMIT).contains(&limit) {
        return Ok(());
    }
    Err(conflict("Group Run list limit is outside its bounds"))
}

fn conversion_error(index: usize, message: &str) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        index,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.to_owned(),
        )),
    )
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupRun,
        message: message.into(),
    }
}
