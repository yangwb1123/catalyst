use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::HubStoreError;

use super::super::read_error;

pub(super) const METADATA_COLUMNS: &str = "id,group_run_id,graph_version,status,\
 source_snapshot_sha256,manifest_bytes,manifest_sha256,node_count,edge_count,wave_count,\
 created_at_ms";
const STORED_COLUMNS: &str = "id,group_run_id,graph_version,status,\
 source_snapshot_sha256,manifest_bytes,manifest_sha256,node_count,edge_count,wave_count,\
 created_at_ms,idempotency_key,manifest_blob";

#[derive(Clone)]
pub(super) struct RawGraphMetadata {
    pub id: String,
    pub group_run_id: String,
    pub graph_version: i64,
    pub status: String,
    pub source_snapshot_sha256: Vec<u8>,
    pub manifest_bytes: i64,
    pub manifest_sha256: Vec<u8>,
    pub node_count: i64,
    pub edge_count: i64,
    pub wave_count: i64,
    pub created_at_ms: i64,
}

pub(super) struct RawStoredGraph {
    pub metadata: RawGraphMetadata,
    pub idempotency_key: String,
    pub manifest_blob: Vec<u8>,
}

pub(super) fn find_by_id(
    connection: &Connection,
    graph_id: &str,
) -> Result<Option<RawStoredGraph>, HubStoreError> {
    query_stored(connection, "id", graph_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredGraph>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn query_metadata(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawGraphMetadata>, HubStoreError> {
    match group_run_id {
        Some(id) => query_metadata_with(
            connection,
            "WHERE group_run_id = ?1 ORDER BY created_at_ms DESC,id DESC LIMIT ?2",
            params![id, limit],
        ),
        None => query_metadata_with(
            connection,
            "ORDER BY created_at_ms DESC,id DESC LIMIT ?1",
            [limit],
        ),
    }
}

fn query_stored(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawStoredGraph>, HubStoreError> {
    let sql = match column {
        "id" => format!("SELECT {STORED_COLUMNS} FROM group_agent_graphs WHERE id = ?1"),
        "idempotency_key" => {
            format!("SELECT {STORED_COLUMNS} FROM group_agent_graphs WHERE idempotency_key = ?1")
        }
        _ => return Err(corrupt("unsupported Group Agent Graph lookup")),
    };
    connection
        .query_row(&sql, [value], stored_row)
        .optional()
        .map_err(read_error)
}

fn query_metadata_with<P>(
    connection: &Connection,
    suffix: &str,
    parameters: P,
) -> Result<Vec<RawGraphMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!("SELECT {METADATA_COLUMNS} FROM group_agent_graphs {suffix}");
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredGraph> {
    Ok(RawStoredGraph {
        metadata: metadata_row(row)?,
        idempotency_key: row.get(11)?,
        manifest_blob: row.get(12)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawGraphMetadata> {
    Ok(RawGraphMetadata {
        id: row.get(0)?,
        group_run_id: row.get(1)?,
        graph_version: row.get(2)?,
        status: row.get(3)?,
        source_snapshot_sha256: row.get(4)?,
        manifest_bytes: row.get(5)?,
        manifest_sha256: row.get(6)?,
        node_count: row.get(7)?,
        edge_count: row.get(8)?,
        wave_count: row.get(9)?,
        created_at_ms: row.get(10)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
