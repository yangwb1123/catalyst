use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::governance_contract::{MAX_ARRAY_ITEMS, MAX_RECORD_SET_BYTES};
use crate::runtime_domain::{GovernanceRecordKind, HubStoreError};

use super::error;

const METADATA_COLUMNS: &str =
    "r.record_id,r.batch_id,r.batch_ordinal,r.record_kind,r.aggregate_id,r.sequence,
     r.canonical_sha256,r.canonical_record_bytes,r.created_at_unix_ms,r.appended_at_ms,
     b.journal_version,b.appended_at_ms,b.record_count";
const REVEALED_COLUMNS: &str =
    "r.record_id,r.batch_id,r.batch_ordinal,r.record_kind,r.aggregate_id,r.sequence,
     r.canonical_sha256,r.canonical_record_bytes,r.created_at_unix_ms,r.appended_at_ms,
     b.journal_version,b.appended_at_ms,b.record_count,r.canonical_record_blob";
const RECORD_JOIN: &str = " FROM governance_records r LEFT JOIN governance_record_append_batches b ON b.batch_id=r.batch_id";
const BATCH_COLUMNS: &str = "batch_id,journal_version,idempotency_key,request_sha256,
     record_set_sha256,record_count,record_set_bytes,appended_at_ms";
pub(super) const ALL_RECORDS_SQL: &str =
    "SELECT r.record_id,r.batch_id,r.batch_ordinal,r.record_kind,r.aggregate_id,r.sequence,
     r.canonical_sha256,r.canonical_record_bytes,r.created_at_unix_ms,r.appended_at_ms,
     b.journal_version,b.appended_at_ms,b.record_count,r.canonical_record_blob
     FROM governance_records r LEFT JOIN governance_record_append_batches b ON b.batch_id=r.batch_id
     ORDER BY r.record_kind,r.aggregate_id,r.sequence,r.record_id";

#[derive(Clone, Debug)]
pub(super) struct RawBatch {
    pub batch_id: String,
    pub journal_version: i64,
    pub idempotency_key: String,
    pub request_sha256: Vec<u8>,
    pub record_set_sha256: Vec<u8>,
    pub record_count: i64,
    pub record_set_bytes: i64,
    pub appended_at_ms: i64,
}

#[derive(Clone, Debug)]
pub(super) struct RawRecord {
    pub record_id: String,
    pub batch_id: String,
    pub batch_ordinal: i64,
    pub record_kind: String,
    pub aggregate_id: String,
    pub sequence: i64,
    pub canonical_sha256: Vec<u8>,
    pub canonical_record_bytes: i64,
    pub created_at_unix_ms: i64,
    pub appended_at_ms: i64,
    pub batch_version: i64,
    pub batch_appended_at_ms: i64,
    pub batch_record_count: i64,
    pub canonical_record_blob: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub(super) struct RawHead {
    pub record_kind: String,
    pub aggregate_id: String,
    pub record_id: String,
    pub sequence: i64,
    pub canonical_sha256: Vec<u8>,
    pub updated_at_ms: i64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AggregateSummary {
    pub count: i64,
    pub minimum_sequence: Option<i64>,
    pub maximum_sequence: Option<i64>,
}

pub(super) fn find_batch_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawBatch>, HubStoreError> {
    find_batch(connection, "idempotency_key", key)
}

pub(super) fn find_batch_by_id(
    connection: &Connection,
    batch_id: &str,
) -> Result<Option<RawBatch>, HubStoreError> {
    find_batch(connection, "batch_id", batch_id)
}

pub(super) fn next_batch(
    connection: &Connection,
    after: Option<&str>,
) -> Result<Option<RawBatch>, HubStoreError> {
    let result = if let Some(value) = after {
        connection
            .query_row(
                &format!(
                    "SELECT {BATCH_COLUMNS} FROM governance_record_append_batches
                     WHERE batch_id>?1 ORDER BY batch_id LIMIT 1"
                ),
                [value],
                raw_batch,
            )
            .optional()
    } else {
        connection
            .query_row(
                &format!(
                    "SELECT {BATCH_COLUMNS} FROM governance_record_append_batches
                     ORDER BY batch_id LIMIT 1"
                ),
                [],
                raw_batch,
            )
            .optional()
    };
    result.map_err(error::read)
}

fn find_batch(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawBatch>, HubStoreError> {
    let sql = format!(
        "SELECT batch_id,journal_version,idempotency_key,request_sha256,record_set_sha256,
         record_count,record_set_bytes,appended_at_ms
         FROM governance_record_append_batches WHERE {column}=?1"
    );
    connection
        .query_row(&sql, [value], raw_batch)
        .optional()
        .map_err(error::read)
}

pub(super) fn find_record(
    connection: &Connection,
    record_id: &str,
    reveal: bool,
) -> Result<Option<RawRecord>, HubStoreError> {
    let columns = if reveal {
        REVEALED_COLUMNS
    } else {
        METADATA_COLUMNS
    };
    let sql = format!("SELECT {columns}{RECORD_JOIN} WHERE r.record_id=?1");
    connection
        .query_row(&sql, [record_id], |row| raw_record(row, reveal))
        .optional()
        .map_err(error::read)
}

pub(super) fn find_record_by_identity(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
    sequence: i64,
    reveal: bool,
) -> Result<Option<RawRecord>, HubStoreError> {
    let columns = if reveal {
        REVEALED_COLUMNS
    } else {
        METADATA_COLUMNS
    };
    let sql = format!(
        "SELECT {columns}{RECORD_JOIN}
         WHERE r.record_kind=?1 AND r.aggregate_id=?2 AND r.sequence=?3"
    );
    connection
        .query_row(
            &sql,
            params![kind.as_str(), aggregate_id, sequence],
            |row| raw_record(row, reveal),
        )
        .optional()
        .map_err(error::read)
}

pub(super) fn records_for_batch(
    connection: &Connection,
    batch_id: &str,
) -> Result<Vec<RawRecord>, HubStoreError> {
    let sql = format!(
        "SELECT {REVEALED_COLUMNS}{RECORD_JOIN}
         WHERE r.batch_id=?1 ORDER BY r.batch_ordinal ASC"
    );
    let mut statement = connection.prepare(&sql).map_err(error::read)?;
    let mapped = statement
        .query_map([batch_id], |row| raw_record(row, true))
        .map_err(error::read)?;
    let mut records = Vec::new();
    let mut bytes = 0_usize;
    for raw in mapped {
        let raw = raw.map_err(error::read)?;
        let record_bytes =
            error::stored_usize(raw.canonical_record_bytes, "canonical record byte count")?;
        bytes = bytes
            .checked_add(record_bytes)
            .ok_or_else(|| error::corrupt("stored batch byte count overflowed"))?;
        if records.len() == MAX_ARRAY_ITEMS || bytes > MAX_RECORD_SET_BYTES {
            return Err(error::corrupt(
                "stored governance batch exceeds exact bounds",
            ));
        }
        records.push(raw);
    }
    Ok(records)
}

pub(super) fn list_records(
    connection: &Connection,
    kind: Option<GovernanceRecordKind>,
    aggregate_id: Option<&str>,
    limit: i64,
    reveal: bool,
) -> Result<Vec<RawRecord>, HubStoreError> {
    let columns = if reveal {
        REVEALED_COLUMNS
    } else {
        METADATA_COLUMNS
    };
    let ordered = " ORDER BY r.appended_at_ms DESC,r.record_id DESC";
    match (kind, aggregate_id) {
        (None, None) => query_records(
            connection,
            &format!("SELECT {columns}{RECORD_JOIN}{ordered} LIMIT ?1"),
            [limit],
            reveal,
        ),
        (Some(kind), None) => query_records(
            connection,
            &format!("SELECT {columns}{RECORD_JOIN} WHERE r.record_kind=?1{ordered} LIMIT ?2"),
            params![kind.as_str(), limit],
            reveal,
        ),
        (None, Some(aggregate)) => query_records(
            connection,
            &format!("SELECT {columns}{RECORD_JOIN} WHERE r.aggregate_id=?1{ordered} LIMIT ?2"),
            params![aggregate, limit],
            reveal,
        ),
        (Some(kind), Some(aggregate)) => query_records(
            connection,
            &format!(
                "SELECT {columns}{RECORD_JOIN} WHERE r.record_kind=?1 AND r.aggregate_id=?2{ordered} LIMIT ?3"
            ),
            params![kind.as_str(), aggregate, limit],
            reveal,
        ),
    }
}

fn query_records<P>(
    connection: &Connection,
    sql: &str,
    parameters: P,
    reveal: bool,
) -> Result<Vec<RawRecord>, HubStoreError>
where
    P: rusqlite::Params,
{
    let mut statement = connection.prepare(sql).map_err(error::read)?;
    let mapped = statement
        .query_map(parameters, |row| raw_record(row, reveal))
        .map_err(error::read)?;
    mapped.collect::<Result<Vec<_>, _>>().map_err(error::read)
}

pub(super) fn find_head(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<Option<RawHead>, HubStoreError> {
    connection
        .query_row(
            "SELECT record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
             FROM governance_structural_heads WHERE record_kind=?1 AND aggregate_id=?2",
            params![kind.as_str(), aggregate_id],
            raw_head,
        )
        .optional()
        .map_err(error::read)
}

pub(super) fn aggregate_summary(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
) -> Result<AggregateSummary, HubStoreError> {
    connection
        .query_row(
            "SELECT COUNT(*),MIN(sequence),MAX(sequence) FROM governance_records
             WHERE record_kind=?1 AND aggregate_id=?2",
            params![kind.as_str(), aggregate_id],
            |row| {
                Ok(AggregateSummary {
                    count: row.get(0)?,
                    minimum_sequence: row.get(1)?,
                    maximum_sequence: row.get(2)?,
                })
            },
        )
        .map_err(error::read)
}

pub(super) fn aggregate_record_ids(
    connection: &Connection,
    kind: GovernanceRecordKind,
    aggregate_id: &str,
    limit: i64,
) -> Result<Vec<String>, HubStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT record_id FROM governance_records
             WHERE record_kind=?1 AND aggregate_id=?2
             ORDER BY sequence,record_id LIMIT ?3",
        )
        .map_err(error::read)?;
    statement
        .query_map(params![kind.as_str(), aggregate_id, limit], |row| {
            row.get(0)
        })
        .map_err(error::read)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error::read)
}

pub(super) fn prepare_rebuilt_heads(connection: &Connection) -> Result<(), HubStoreError> {
    connection
        .execute_batch(
            "DROP TABLE IF EXISTS temp.governance_rebuilt_heads;
             CREATE TEMP TABLE governance_rebuilt_heads(
               record_kind TEXT NOT NULL,
               aggregate_id TEXT NOT NULL,
               record_id TEXT NOT NULL,
               sequence INTEGER NOT NULL,
               canonical_sha256 BLOB NOT NULL,
               updated_at_ms INTEGER NOT NULL,
               PRIMARY KEY(record_kind,aggregate_id)
             ) WITHOUT ROWID;",
        )
        .map_err(error::write)
}

pub(super) fn insert_rebuilt_head(
    connection: &Connection,
    head: &crate::runtime_domain::GovernanceStructuralHead,
) -> Result<(), HubStoreError> {
    let digest = error::digest_blob(&head.canonical_sha256, "head digest")?;
    connection
        .execute(
            "INSERT INTO temp.governance_rebuilt_heads(
               record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6)",
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
    Ok(())
}

pub(super) fn replace_heads_from_rebuild(connection: &Connection) -> Result<usize, HubStoreError> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM temp.governance_rebuilt_heads",
            [],
            |row| row.get(0),
        )
        .map_err(error::read)?;
    connection
        .execute("DELETE FROM governance_structural_heads", [])
        .map_err(error::write)?;
    connection
        .execute(
            "INSERT INTO governance_structural_heads(
               record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
             )
             SELECT record_kind,aggregate_id,record_id,sequence,canonical_sha256,updated_at_ms
             FROM temp.governance_rebuilt_heads",
            [],
        )
        .map_err(error::write)?;
    connection
        .execute_batch("DROP TABLE temp.governance_rebuilt_heads")
        .map_err(error::write)?;
    error::stored_usize(count, "rebuilt head count")
}

fn raw_batch(row: &Row<'_>) -> rusqlite::Result<RawBatch> {
    Ok(RawBatch {
        batch_id: row.get(0)?,
        journal_version: row.get(1)?,
        idempotency_key: row.get(2)?,
        request_sha256: row.get(3)?,
        record_set_sha256: row.get(4)?,
        record_count: row.get(5)?,
        record_set_bytes: row.get(6)?,
        appended_at_ms: row.get(7)?,
    })
}

pub(super) fn raw_record(row: &Row<'_>, reveal: bool) -> rusqlite::Result<RawRecord> {
    Ok(RawRecord {
        record_id: row.get(0)?,
        batch_id: row.get(1)?,
        batch_ordinal: row.get(2)?,
        record_kind: row.get(3)?,
        aggregate_id: row.get(4)?,
        sequence: row.get(5)?,
        canonical_sha256: row.get(6)?,
        canonical_record_bytes: row.get(7)?,
        created_at_unix_ms: row.get(8)?,
        appended_at_ms: row.get(9)?,
        batch_version: row.get(10)?,
        batch_appended_at_ms: row.get(11)?,
        batch_record_count: row.get(12)?,
        canonical_record_blob: reveal.then(|| row.get(13)).transpose()?,
    })
}

fn raw_head(row: &Row<'_>) -> rusqlite::Result<RawHead> {
    Ok(RawHead {
        record_kind: row.get(0)?,
        aggregate_id: row.get(1)?,
        record_id: row.get(2)?,
        sequence: row.get(3)?,
        canonical_sha256: row.get(4)?,
        updated_at_ms: row.get(5)?,
    })
}
