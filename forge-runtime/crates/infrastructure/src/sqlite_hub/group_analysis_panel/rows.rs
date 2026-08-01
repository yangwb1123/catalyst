use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::runtime_domain::HubStoreError;

use super::super::read_error;

pub(super) const METADATA_COLUMNS: &str = "id,group_run_id,panel_version,status,\
 source_snapshot_sha256,analysis_count,manifest_bytes,manifest_sha256,created_at_ms";
const STORED_COLUMNS: &str = "id,group_run_id,panel_version,status,\
 source_snapshot_sha256,analysis_count,manifest_bytes,manifest_sha256,created_at_ms,\
 idempotency_key,manifest_blob";

#[derive(Clone)]
pub(super) struct RawPanelMetadata {
    pub id: String,
    pub group_run_id: String,
    pub panel_version: i64,
    pub status: String,
    pub source_snapshot_sha256: Vec<u8>,
    pub analysis_count: i64,
    pub manifest_bytes: i64,
    pub manifest_sha256: Vec<u8>,
    pub created_at_ms: i64,
}

pub(super) struct RawStoredPanel {
    pub metadata: RawPanelMetadata,
    pub idempotency_key: String,
    pub manifest_blob: Vec<u8>,
}

pub(super) struct RawPanelAnalysis {
    pub position: i64,
    pub analysis_id: String,
    pub result_sha256: Vec<u8>,
}

pub(super) fn find_by_id(
    connection: &Connection,
    panel_id: &str,
) -> Result<Option<RawStoredPanel>, HubStoreError> {
    query_stored(connection, "id", panel_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredPanel>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn query_metadata(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawPanelMetadata>, HubStoreError> {
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

pub(super) fn load_analyses(
    connection: &Connection,
    panel_id: &str,
) -> Result<Vec<RawPanelAnalysis>, HubStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT position,analysis_id,result_sha256
             FROM group_analysis_panel_analyses
             WHERE panel_id = ?1 ORDER BY position LIMIT 9",
        )
        .map_err(read_error)?;
    statement
        .query_map([panel_id], |row| {
            Ok(RawPanelAnalysis {
                position: row.get(0)?,
                analysis_id: row.get(1)?,
                result_sha256: row.get(2)?,
            })
        })
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn query_stored(
    connection: &Connection,
    column: &str,
    value: &str,
) -> Result<Option<RawStoredPanel>, HubStoreError> {
    let sql = match column {
        "id" => format!("SELECT {STORED_COLUMNS} FROM group_analysis_panels WHERE id = ?1"),
        "idempotency_key" => {
            format!("SELECT {STORED_COLUMNS} FROM group_analysis_panels WHERE idempotency_key = ?1")
        }
        _ => return Err(corrupt("unsupported Group Analysis Panel lookup")),
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
) -> Result<Vec<RawPanelMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!("SELECT {METADATA_COLUMNS} FROM group_analysis_panels {suffix}");
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredPanel> {
    Ok(RawStoredPanel {
        metadata: metadata_row(row)?,
        idempotency_key: row.get(9)?,
        manifest_blob: row.get(10)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawPanelMetadata> {
    Ok(RawPanelMetadata {
        id: row.get(0)?,
        group_run_id: row.get(1)?,
        panel_version: row.get(2)?,
        status: row.get(3)?,
        source_snapshot_sha256: row.get(4)?,
        analysis_count: row.get(5)?,
        manifest_bytes: row.get(6)?,
        manifest_sha256: row.get(7)?,
        created_at_ms: row.get(8)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
