use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{HubStoreError, read_error};

pub(super) const METADATA_COLUMNS: &str = "id,group_run_id,analysis_version,status,\
 source_snapshot_sha256,provider,endpoint,model,system_prompt_version,\
 system_prompt_sha256,max_output_tokens,max_model_output_bytes,max_model_events,\
 config_sha256,request_sha256,request_bytes,protocol_version,created_at_ms";
const STORED_COLUMNS: &str = "id,group_run_id,analysis_version,status,\
 source_snapshot_sha256,provider,endpoint,model,system_prompt_version,\
 system_prompt_sha256,max_output_tokens,max_model_output_bytes,max_model_events,\
 config_sha256,request_sha256,request_bytes,protocol_version,created_at_ms,config_json,\
 request_body,cursor_json,journal_bytes,idempotency_key";

#[derive(Clone)]
pub(super) struct RawAnalysisMetadata {
    pub id: String,
    pub group_run_id: String,
    pub analysis_version: i64,
    pub status: String,
    pub source_snapshot_sha256: Vec<u8>,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub system_prompt_version: i64,
    pub system_prompt_sha256: Vec<u8>,
    pub max_output_tokens: i64,
    pub max_model_output_bytes: i64,
    pub max_model_events: i64,
    pub config_sha256: Vec<u8>,
    pub request_sha256: Vec<u8>,
    pub request_bytes: i64,
    pub protocol_version: i64,
    pub created_at_ms: i64,
}

#[derive(Clone)]
pub(super) struct RawStoredAnalysis {
    pub metadata: RawAnalysisMetadata,
    pub config_json: String,
    pub request_body: Vec<u8>,
    pub cursor_json: String,
    pub journal_bytes: i64,
    pub idempotency_key: String,
}

#[derive(Clone)]
pub(super) struct RawResult {
    pub analysis_id: String,
    pub result_version: i64,
    pub result_blob: Vec<u8>,
    pub result_bytes: i64,
    pub result_sha256: Vec<u8>,
    pub created_at_ms: i64,
}

pub(super) fn find_by_id(
    connection: &Connection,
    analysis_id: &str,
) -> Result<Option<RawStoredAnalysis>, HubStoreError> {
    query_stored(connection, "id", analysis_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredAnalysis>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn load_event_rows(
    connection: &Connection,
    analysis_id: &str,
) -> Result<Vec<(i64, String, Vec<u8>)>, HubStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT seq,event_json,event_sha256 FROM group_model_analysis_events
             WHERE analysis_id = ?1 ORDER BY seq LIMIT 4",
        )
        .map_err(read_error)?;
    statement
        .query_map([analysis_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

pub(super) fn load_result(
    connection: &Connection,
    analysis_id: &str,
) -> Result<Option<RawResult>, HubStoreError> {
    connection
        .query_row(
            "SELECT analysis_id,result_version,result_blob,result_bytes,result_sha256,created_at_ms
             FROM group_model_analysis_results WHERE analysis_id = ?1",
            [analysis_id],
            result_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn query_metadata(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawAnalysisMetadata>, HubStoreError> {
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
) -> Result<Option<RawStoredAnalysis>, HubStoreError> {
    let sql = match column {
        "id" => format!("SELECT {STORED_COLUMNS} FROM group_model_analyses WHERE id = ?1"),
        "idempotency_key" => {
            format!("SELECT {STORED_COLUMNS} FROM group_model_analyses WHERE idempotency_key = ?1")
        }
        _ => return Err(corrupt("unsupported Group model analysis lookup")),
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
) -> Result<Vec<RawAnalysisMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!("SELECT {METADATA_COLUMNS} FROM group_model_analyses {suffix}");
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredAnalysis> {
    Ok(RawStoredAnalysis {
        metadata: metadata_row(row)?,
        config_json: row.get(18)?,
        request_body: row.get(19)?,
        cursor_json: row.get(20)?,
        journal_bytes: row.get(21)?,
        idempotency_key: row.get(22)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawAnalysisMetadata> {
    Ok(RawAnalysisMetadata {
        id: row.get(0)?,
        group_run_id: row.get(1)?,
        analysis_version: row.get(2)?,
        status: row.get(3)?,
        source_snapshot_sha256: row.get(4)?,
        provider: row.get(5)?,
        endpoint: row.get(6)?,
        model: row.get(7)?,
        system_prompt_version: row.get(8)?,
        system_prompt_sha256: row.get(9)?,
        max_output_tokens: row.get(10)?,
        max_model_output_bytes: row.get(11)?,
        max_model_events: row.get(12)?,
        config_sha256: row.get(13)?,
        request_sha256: row.get(14)?,
        request_bytes: row.get(15)?,
        protocol_version: row.get(16)?,
        created_at_ms: row.get(17)?,
    })
}

fn result_row(row: &Row<'_>) -> rusqlite::Result<RawResult> {
    Ok(RawResult {
        analysis_id: row.get(0)?,
        result_version: row.get(1)?,
        result_blob: row.get(2)?,
        result_bytes: row.get(3)?,
        result_sha256: row.get(4)?,
        created_at_ms: row.get(5)?,
    })
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}
