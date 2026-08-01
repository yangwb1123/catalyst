use rusqlite::{Transaction, params};

use super::{HubEntity, HubStoreError, write_error};

pub(super) struct NewSynthesisRow<'a> {
    pub id: &'a str,
    pub panel_id: &'a str,
    pub group_run_id: &'a str,
    pub synthesis_version: i64,
    pub source_snapshot_sha256: &'a [u8; 32],
    pub panel_manifest_sha256: &'a [u8; 32],
    pub provider: &'a str,
    pub endpoint: &'a str,
    pub model: &'a str,
    pub system_prompt_version: i64,
    pub system_prompt_sha256: &'a [u8; 32],
    pub output_target: &'a str,
    pub writeback_target: &'a str,
    pub max_output_tokens: i64,
    pub max_model_output_bytes: i64,
    pub max_model_events: i64,
    pub config_json: &'a str,
    pub config_sha256: &'a [u8; 32],
    pub request_body: &'a [u8],
    pub request_sha256: &'a [u8; 32],
    pub cursor_json: &'a str,
    pub journal_bytes: i64,
    pub idempotency_key: &'a str,
    pub protocol_version: i64,
    pub created_at_ms: i64,
}

pub(super) struct EventRow<'a> {
    pub synthesis_id: &'a str,
    pub sequence: i64,
    pub json: &'a str,
    pub sha256: &'a [u8; 32],
}

pub(super) struct ResultRow<'a> {
    pub synthesis_id: &'a str,
    pub result_version: i64,
    pub blob: &'a [u8],
    pub result_bytes: i64,
    pub sha256: &'a [u8; 32],
    pub created_at_ms: i64,
}

pub(super) fn insert_synthesis(
    transaction: &Transaction<'_>,
    row: &NewSynthesisRow<'_>,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_panel_syntheses(
               id,panel_id,group_run_id,synthesis_version,status,source_snapshot_sha256,
               panel_manifest_sha256,provider,endpoint,model,system_prompt_version,
               system_prompt_sha256,output_target,writeback_target,max_output_tokens,
               max_model_output_bytes,max_model_events,config_json,config_sha256,
               request_body,request_bytes,request_sha256,cursor_json,journal_bytes,
               idempotency_key,protocol_version,created_at_ms
             ) VALUES(
               ?1,?2,?3,?4,'awaiting_consent',?5,?6,?7,?8,?9,?10,?11,?12,?13,
               ?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26
             )",
            params![
                row.id,
                row.panel_id,
                row.group_run_id,
                row.synthesis_version,
                row.source_snapshot_sha256.as_slice(),
                row.panel_manifest_sha256.as_slice(),
                row.provider,
                row.endpoint,
                row.model,
                row.system_prompt_version,
                row.system_prompt_sha256.as_slice(),
                row.output_target,
                row.writeback_target,
                row.max_output_tokens,
                row.max_model_output_bytes,
                row.max_model_events,
                row.config_json,
                row.config_sha256.as_slice(),
                row.request_body,
                i64::try_from(row.request_body.len()).map_err(convert_error)?,
                row.request_sha256.as_slice(),
                row.cursor_json,
                row.journal_bytes,
                row.idempotency_key,
                row.protocol_version,
                row.created_at_ms,
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupPanelSynthesis, error))?;
    Ok(())
}

pub(super) fn insert_event(
    transaction: &Transaction<'_>,
    row: &EventRow<'_>,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_panel_synthesis_events(
               synthesis_id,seq,event_json,event_sha256
             ) VALUES(?1,?2,?3,?4)",
            params![
                row.synthesis_id,
                row.sequence,
                row.json,
                row.sha256.as_slice()
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupPanelSynthesis, error))?;
    Ok(())
}

pub(super) fn insert_result(
    transaction: &Transaction<'_>,
    row: &ResultRow<'_>,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO group_panel_synthesis_results(
               synthesis_id,result_version,result_blob,result_bytes,result_sha256,created_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                row.synthesis_id,
                row.result_version,
                row.blob,
                row.result_bytes,
                row.sha256.as_slice(),
                row.created_at_ms
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupPanelSynthesis, error))?;
    Ok(())
}

pub(super) fn update_journal(
    transaction: &Transaction<'_>,
    synthesis_id: &str,
    expected_status: &str,
    new_status: &str,
    cursor_json: &str,
    journal_bytes: i64,
) -> Result<(), HubStoreError> {
    let changed = transaction
        .execute(
            "UPDATE group_panel_syntheses
             SET status = ?1,cursor_json = ?2,journal_bytes = ?3
             WHERE id = ?4 AND status = ?5",
            params![
                new_status,
                cursor_json,
                journal_bytes,
                synthesis_id,
                expected_status
            ],
        )
        .map_err(|error| write_error(HubEntity::GroupPanelSynthesis, error))?;
    if changed == 1 {
        Ok(())
    } else {
        Err(HubStoreError::Corrupt {
            message: "Group Panel Synthesis status changed during journal update".into(),
        })
    }
}

fn convert_error(error: impl std::fmt::Display) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupPanelSynthesis,
        message: error.to_string(),
    }
}
