use rusqlite::{Connection, OptionalExtension, Row, params};

use super::{HubStoreError, read_error};

pub(super) const METADATA_COLUMNS: &str = "id,panel_id,group_run_id,synthesis_version,status,\
 source_snapshot_sha256,panel_manifest_sha256,provider,endpoint,model,system_prompt_version,\
 system_prompt_sha256,output_target,writeback_target,max_output_tokens,max_model_output_bytes,\
 max_model_events,config_sha256,request_sha256,request_bytes,protocol_version,created_at_ms";
const STORED_COLUMNS: &str = "id,panel_id,group_run_id,synthesis_version,status,\
 source_snapshot_sha256,panel_manifest_sha256,provider,endpoint,model,system_prompt_version,\
 system_prompt_sha256,output_target,writeback_target,max_output_tokens,max_model_output_bytes,\
 max_model_events,config_sha256,request_sha256,request_bytes,protocol_version,created_at_ms,\
 config_json,request_body,cursor_json,journal_bytes,idempotency_key";

#[derive(Clone)]
pub(super) struct RawSynthesisMetadata {
    pub id: String,
    pub panel_id: String,
    pub group_run_id: String,
    pub synthesis_version: i64,
    pub status: String,
    pub source_snapshot_sha256: Vec<u8>,
    pub panel_manifest_sha256: Vec<u8>,
    pub provider: String,
    pub endpoint: String,
    pub model: String,
    pub system_prompt_version: i64,
    pub system_prompt_sha256: Vec<u8>,
    pub output_target: String,
    pub writeback_target: String,
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
pub(super) struct RawStoredSynthesis {
    pub metadata: RawSynthesisMetadata,
    pub config_json: String,
    pub request_body: Vec<u8>,
    pub cursor_json: String,
    pub journal_bytes: i64,
    pub idempotency_key: String,
}

#[derive(Clone)]
pub(super) struct RawResult {
    pub synthesis_id: String,
    pub result_version: i64,
    pub result_blob: Vec<u8>,
    pub result_bytes: i64,
    pub result_sha256: Vec<u8>,
    pub created_at_ms: i64,
}

pub(super) fn find_by_id(
    connection: &Connection,
    synthesis_id: &str,
) -> Result<Option<RawStoredSynthesis>, HubStoreError> {
    query_stored(connection, "id", synthesis_id)
}

pub(super) fn find_by_key(
    connection: &Connection,
    key: &str,
) -> Result<Option<RawStoredSynthesis>, HubStoreError> {
    query_stored(connection, "idempotency_key", key)
}

pub(super) fn load_event_rows(
    connection: &Connection,
    synthesis_id: &str,
) -> Result<Vec<(i64, String, Vec<u8>)>, HubStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT seq,event_json,event_sha256 FROM group_panel_synthesis_events
             WHERE synthesis_id = ?1 ORDER BY seq LIMIT 4",
        )
        .map_err(read_error)?;
    statement
        .query_map([synthesis_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

pub(super) fn load_result(
    connection: &Connection,
    synthesis_id: &str,
) -> Result<Option<RawResult>, HubStoreError> {
    connection
        .query_row(
            "SELECT synthesis_id,result_version,result_blob,result_bytes,result_sha256,created_at_ms
             FROM group_panel_synthesis_results WHERE synthesis_id = ?1",
            [synthesis_id],
            result_row,
        )
        .optional()
        .map_err(read_error)
}

pub(super) fn query_metadata(
    connection: &Connection,
    panel_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RawSynthesisMetadata>, HubStoreError> {
    match panel_id {
        Some(id) => query_metadata_with(
            connection,
            "WHERE panel_id = ?1 ORDER BY created_at_ms DESC,id DESC LIMIT ?2",
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
) -> Result<Option<RawStoredSynthesis>, HubStoreError> {
    let sql = match column {
        "id" => format!("SELECT {STORED_COLUMNS} FROM group_panel_syntheses WHERE id = ?1"),
        "idempotency_key" => {
            format!("SELECT {STORED_COLUMNS} FROM group_panel_syntheses WHERE idempotency_key = ?1")
        }
        _ => return Err(corrupt("unsupported Group Panel Synthesis lookup")),
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
) -> Result<Vec<RawSynthesisMetadata>, HubStoreError>
where
    P: rusqlite::Params,
{
    let sql = format!("SELECT {METADATA_COLUMNS} FROM group_panel_syntheses {suffix}");
    let mut statement = connection.prepare(&sql).map_err(read_error)?;
    statement
        .query_map(parameters, metadata_row)
        .map_err(read_error)?
        .map(|row| row.map_err(read_error))
        .collect()
}

fn stored_row(row: &Row<'_>) -> rusqlite::Result<RawStoredSynthesis> {
    Ok(RawStoredSynthesis {
        metadata: metadata_row(row)?,
        config_json: row.get(22)?,
        request_body: row.get(23)?,
        cursor_json: row.get(24)?,
        journal_bytes: row.get(25)?,
        idempotency_key: row.get(26)?,
    })
}

fn metadata_row(row: &Row<'_>) -> rusqlite::Result<RawSynthesisMetadata> {
    Ok(RawSynthesisMetadata {
        id: row.get(0)?,
        panel_id: row.get(1)?,
        group_run_id: row.get(2)?,
        synthesis_version: row.get(3)?,
        status: row.get(4)?,
        source_snapshot_sha256: row.get(5)?,
        panel_manifest_sha256: row.get(6)?,
        provider: row.get(7)?,
        endpoint: row.get(8)?,
        model: row.get(9)?,
        system_prompt_version: row.get(10)?,
        system_prompt_sha256: row.get(11)?,
        output_target: row.get(12)?,
        writeback_target: row.get(13)?,
        max_output_tokens: row.get(14)?,
        max_model_output_bytes: row.get(15)?,
        max_model_events: row.get(16)?,
        config_sha256: row.get(17)?,
        request_sha256: row.get(18)?,
        request_bytes: row.get(19)?,
        protocol_version: row.get(20)?,
        created_at_ms: row.get(21)?,
    })
}

fn result_row(row: &Row<'_>) -> rusqlite::Result<RawResult> {
    Ok(RawResult {
        synthesis_id: row.get(0)?,
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
