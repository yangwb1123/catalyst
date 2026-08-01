use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use super::{
    GROUP_PANEL_SYNTHESIS_RESULT_VERSION, GroupAnalysisPanelInspection, GroupPanelSynthesisConfig,
    GroupPanelSynthesisInspection, GroupPanelSynthesisJournalCursor,
    GroupPanelSynthesisOutputTarget, GroupPanelSynthesisProvider, GroupPanelSynthesisRecord,
    GroupPanelSynthesisRecovery, GroupPanelSynthesisRequestConfig,
    GroupPanelSynthesisResultArtifact, GroupPanelSynthesisSource, GroupPanelSynthesisStatus,
    GroupPanelSynthesisWritebackTarget, HubEntity, HubStoreError, MAX_GROUP_PANEL_SYNTHESIS_EVENTS,
    MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES, MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES,
    MAX_GROUP_PANEL_SYNTHESIS_JOURNAL_BYTES, MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT, codec,
    group_analysis_panel, group_run_codec, read_error, rows,
};

pub(super) struct ValidatedSynthesis {
    pub stored: rows::RawStoredSynthesis,
    pub request_config: GroupPanelSynthesisRequestConfig,
    pub cursor: GroupPanelSynthesisJournalCursor,
    pub inspection: GroupPanelSynthesisInspection,
}

pub(super) fn inspect(
    connection: &mut Connection,
    synthesis_id: &str,
) -> Result<GroupPanelSynthesisInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let inspection = inspect_in_snapshot(&transaction, synthesis_id)?;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(in crate::sqlite_hub) fn inspect_in_snapshot(
    connection: &Connection,
    synthesis_id: &str,
) -> Result<GroupPanelSynthesisInspection, HubStoreError> {
    let stored = rows::find_by_id(connection, synthesis_id)?
        .ok_or_else(|| not_found(HubEntity::GroupPanelSynthesis, synthesis_id))?;
    Ok(validate_stored(connection, stored)?.inspection)
}

pub(super) fn list(
    connection: &Connection,
    panel_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupPanelSynthesisRecord>, HubStoreError> {
    validate_list_request(connection, panel_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    rows::query_metadata(connection, panel_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect()
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredSynthesis,
) -> Result<ValidatedSynthesis, HubStoreError> {
    validate_stored_header(&stored)?;
    let record = metadata_record(stored.metadata.clone())?;
    let panel = load_panel(connection, &record.panel_id)?;
    let request_config = validate_config_and_request(&stored, &record, &panel)?;
    let (events, journal_bytes) = load_events(connection, &record.synthesis_id)?;
    let result = load_result(connection, &record.synthesis_id)?;
    let inspection = validate_inspection(record.clone(), events, result)?;
    validate_source_binding(&panel, &inspection)?;
    let cursor = validate_journal(&stored, &record, &inspection, journal_bytes)?;
    Ok(ValidatedSynthesis {
        stored,
        request_config,
        cursor,
        inspection,
    })
}

pub(super) fn load_panel(
    connection: &Connection,
    panel_id: &str,
) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
    group_analysis_panel::read::inspect_in_snapshot(connection, panel_id).map_err(|error| {
        match error {
            HubStoreError::NotFound { .. } => {
                corrupt("synthesis references a missing frozen analysis panel")
            }
            other => other,
        }
    })
}

pub(super) fn load_panel_candidate(
    connection: &Connection,
    panel_id: &str,
) -> Result<GroupAnalysisPanelInspection, HubStoreError> {
    group_analysis_panel::read::inspect_in_snapshot(connection, panel_id).map_err(|error| {
        match error {
            HubStoreError::NotFound { .. } => {
                conflict("synthesis references a missing frozen analysis panel")
            }
            other => other,
        }
    })
}

pub(super) fn source_from_panel(panel: &GroupAnalysisPanelInspection) -> GroupPanelSynthesisSource {
    GroupPanelSynthesisSource {
        panel_version: panel.panel.v,
        panel_id: panel.panel.panel_id.clone(),
        group_run_id: panel.panel.group_run_id.clone(),
        group_id: panel.manifest.source.group_id.clone(),
        source_snapshot_sha256: panel.panel.source_snapshot_sha256.clone(),
        panel_manifest_sha256: panel.panel.manifest_sha256.clone(),
        panel_manifest_bytes: panel.panel.manifest_bytes,
        analysis_count: panel.panel.analysis_count,
    }
}

fn metadata_record(
    raw: rows::RawSynthesisMetadata,
) -> Result<GroupPanelSynthesisRecord, HubStoreError> {
    let config = metadata_config(&raw)?;
    let record = GroupPanelSynthesisRecord {
        v: convert(raw.synthesis_version, "synthesis version")?,
        synthesis_id: raw.id,
        panel_id: raw.panel_id,
        group_run_id: raw.group_run_id,
        status: parse_status(&raw.status)?,
        source_snapshot_sha256: digest(&raw.source_snapshot_sha256, "source snapshot")?,
        panel_manifest_sha256: digest(&raw.panel_manifest_sha256, "panel manifest")?,
        config,
        config_sha256: digest(&raw.config_sha256, "configuration")?,
        request_sha256: digest(&raw.request_sha256, "request")?,
        request_bytes: convert(raw.request_bytes, "request byte count")?,
        protocol_version: convert(raw.protocol_version, "protocol version")?,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    GroupPanelSynthesisJournalCursor::new(&record).map_err(|error| corrupt(&error.message))?;
    Ok(record)
}

fn metadata_config(
    raw: &rows::RawSynthesisMetadata,
) -> Result<GroupPanelSynthesisConfig, HubStoreError> {
    Ok(GroupPanelSynthesisConfig {
        v: convert(raw.synthesis_version, "configuration version")?,
        provider: parse_provider(&raw.provider)?,
        endpoint: raw.endpoint.clone(),
        model: raw.model.clone(),
        system_prompt_version: convert(raw.system_prompt_version, "system Prompt version")?,
        system_prompt_sha256: digest(&raw.system_prompt_sha256, "system Prompt")?,
        max_output_tokens: convert(raw.max_output_tokens, "output token limit")?,
        max_model_output_bytes: convert(raw.max_model_output_bytes, "model output byte limit")?,
        max_model_events: convert(raw.max_model_events, "model event limit")?,
        output_target: parse_output_target(&raw.output_target)?,
        writeback_target: parse_writeback_target(&raw.writeback_target)?,
    })
}

fn validate_stored_header(stored: &rows::RawStoredSynthesis) -> Result<(), HubStoreError> {
    let journal_bytes: usize = convert(stored.journal_bytes, "journal byte count")?;
    let key_valid = group_run_codec::valid_text(
        &stored.idempotency_key,
        MAX_GROUP_PANEL_SYNTHESIS_IDEMPOTENCY_KEY_BYTES,
    );
    if key_valid && (1..=MAX_GROUP_PANEL_SYNTHESIS_JOURNAL_BYTES).contains(&journal_bytes) {
        Ok(())
    } else {
        Err(corrupt(
            "stored Group Panel Synthesis key or journal byte count is invalid",
        ))
    }
}

fn validate_config_and_request(
    stored: &rows::RawStoredSynthesis,
    record: &GroupPanelSynthesisRecord,
    panel: &GroupAnalysisPanelInspection,
) -> Result<GroupPanelSynthesisRequestConfig, HubStoreError> {
    let config_digest = codec::digest_bytes(&record.config_sha256, "configuration")?;
    let request_config = codec::decode_config(&stored.config_json, &config_digest)?;
    if codec::project_config(&request_config).map_err(|error| as_corrupt(&error))? != record.config
    {
        return Err(corrupt(
            "stored private synthesis configuration disagrees with its metadata",
        ));
    }
    validate_request_bytes(stored, record, panel, &request_config)?;
    Ok(request_config)
}

fn validate_request_bytes(
    stored: &rows::RawStoredSynthesis,
    record: &GroupPanelSynthesisRecord,
    panel: &GroupAnalysisPanelInspection,
    config: &GroupPanelSynthesisRequestConfig,
) -> Result<(), HubStoreError> {
    if stored.request_body.len() != record.request_bytes {
        return Err(corrupt("stored synthesis request byte count disagrees"));
    }
    let expected = codec::digest_bytes(&record.request_sha256, "request")?;
    let actual = codec::request_digest(&stored.request_body).map_err(|error| as_corrupt(&error))?;
    if actual != expected {
        return Err(corrupt("stored synthesis request digest disagrees"));
    }
    codec::validate_exact_request(config, panel, &stored.request_body)
}

fn load_events(
    connection: &Connection,
    synthesis_id: &str,
) -> Result<(Vec<super::GroupPanelSynthesisEvent>, usize), HubStoreError> {
    let mut events = Vec::new();
    let mut bytes = 0_usize;
    for (sequence, json, digest) in rows::load_event_rows(connection, synthesis_id)? {
        if events.len() >= MAX_GROUP_PANEL_SYNTHESIS_EVENTS {
            return Err(corrupt("stored synthesis has too many events"));
        }
        bytes = bytes
            .checked_add(json.len())
            .ok_or_else(|| corrupt("stored synthesis journal byte count overflowed"))?;
        if bytes > MAX_GROUP_PANEL_SYNTHESIS_JOURNAL_BYTES {
            return Err(corrupt("stored synthesis journal exceeds its byte limit"));
        }
        events.push(codec::decode_event(sequence, &json, &digest)?);
    }
    Ok((events, bytes))
}

fn load_result(
    connection: &Connection,
    synthesis_id: &str,
) -> Result<Option<GroupPanelSynthesisResultArtifact>, HubStoreError> {
    let Some(raw) = rows::load_result(connection, synthesis_id)? else {
        return Ok(None);
    };
    let version: u16 = convert(raw.result_version, "result version")?;
    let bytes: usize = convert(raw.result_bytes, "result byte count")?;
    let created_at_ms = convert(raw.created_at_ms, "result creation time")?;
    if raw.synthesis_id != synthesis_id
        || version != GROUP_PANEL_SYNTHESIS_RESULT_VERSION
        || bytes != raw.result_blob.len()
    {
        return Err(corrupt("stored synthesis result row binding is invalid"));
    }
    codec::decode_result(&raw.result_blob, &raw.result_sha256, created_at_ms).map(Some)
}

fn validate_inspection(
    record: GroupPanelSynthesisRecord,
    events: Vec<super::GroupPanelSynthesisEvent>,
    result: Option<GroupPanelSynthesisResultArtifact>,
) -> Result<GroupPanelSynthesisInspection, HubStoreError> {
    if events.is_empty() {
        return Err(corrupt("stored synthesis is missing its prepared event"));
    }
    let inspection = GroupPanelSynthesisInspection::validate(record, events, result)
        .map_err(|error| corrupt(&error.message))?;
    if inspection.recovery == GroupPanelSynthesisRecovery::Unprepared {
        Err(corrupt(
            "stored synthesis has an impossible unprepared state",
        ))
    } else {
        Ok(inspection)
    }
}

fn validate_source_binding(
    panel: &GroupAnalysisPanelInspection,
    inspection: &GroupPanelSynthesisInspection,
) -> Result<(), HubStoreError> {
    let Some(receipt) = &inspection.prepared else {
        return Err(corrupt("stored synthesis has no source receipt"));
    };
    if receipt.source == source_from_panel(panel) {
        Ok(())
    } else {
        Err(corrupt(
            "stored synthesis source does not match its frozen analysis panel",
        ))
    }
}

fn validate_journal(
    stored: &rows::RawStoredSynthesis,
    record: &GroupPanelSynthesisRecord,
    inspection: &GroupPanelSynthesisInspection,
    actual_bytes: usize,
) -> Result<GroupPanelSynthesisJournalCursor, HubStoreError> {
    let persisted = codec::decode_cursor(&stored.cursor_json, record)?;
    let mut rebuilt =
        GroupPanelSynthesisJournalCursor::new(record).map_err(|error| corrupt(&error.message))?;
    for event in &inspection.events {
        rebuilt
            .append(event)
            .map_err(|error| corrupt(&error.message))?;
    }
    let declared: usize = convert(stored.journal_bytes, "journal byte count")?;
    if persisted == rebuilt && declared == actual_bytes {
        Ok(persisted)
    } else {
        Err(corrupt(
            "stored synthesis cursor or byte count disagrees with its journal",
        ))
    }
}

fn validate_list_request(
    connection: &Connection,
    panel_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_PANEL_SYNTHESIS_LIST_LIMIT).contains(&limit) {
        return Err(conflict("synthesis list limit is outside its bounds"));
    }
    let Some(id) = panel_id else {
        return Ok(());
    };
    if !group_run_codec::valid_text(id, MAX_GROUP_PANEL_SYNTHESIS_ID_BYTES) {
        return Err(conflict("analysis panel filter is outside its bounds"));
    }
    connection
        .query_row(
            "SELECT 1 FROM group_analysis_panels WHERE id = ?1",
            [id],
            |_| Ok(()),
        )
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::GroupAnalysisPanel, id))
}

fn parse_provider(value: &str) -> Result<GroupPanelSynthesisProvider, HubStoreError> {
    match value {
        "openai_responses" => Ok(GroupPanelSynthesisProvider::OpenAiResponses),
        _ => Err(corrupt("stored synthesis provider is unsupported")),
    }
}

fn parse_output_target(value: &str) -> Result<GroupPanelSynthesisOutputTarget, HubStoreError> {
    match value {
        "local_artifact" => Ok(GroupPanelSynthesisOutputTarget::LocalArtifact),
        _ => Err(corrupt("stored synthesis output target is unsupported")),
    }
}

fn parse_writeback_target(
    value: &str,
) -> Result<GroupPanelSynthesisWritebackTarget, HubStoreError> {
    match value {
        "none" => Ok(GroupPanelSynthesisWritebackTarget::None),
        _ => Err(corrupt("stored synthesis writeback target is unsupported")),
    }
}

fn parse_status(value: &str) -> Result<GroupPanelSynthesisStatus, HubStoreError> {
    match value {
        "awaiting_consent" => Ok(GroupPanelSynthesisStatus::AwaitingConsent),
        "dispatch_unknown" => Ok(GroupPanelSynthesisStatus::DispatchUnknown),
        "completed" => Ok(GroupPanelSynthesisStatus::Completed),
        _ => Err(corrupt("stored synthesis status is unsupported")),
    }
}

fn digest(value: &[u8], subject: &str) -> Result<String, HubStoreError> {
    codec::decode_digest(value, subject).map(|digest| codec::digest_hex(&digest))
}

fn convert<T>(value: i64, subject: &str) -> Result<T, HubStoreError>
where
    T: TryFrom<i64>,
    T::Error: std::fmt::Display,
{
    T::try_from(value).map_err(|error| corrupt(&format!("invalid {subject}: {error}")))
}

fn not_found(entity: HubEntity, id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn as_corrupt(error: &HubStoreError) -> HubStoreError {
    corrupt(&error.to_string())
}

fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupPanelSynthesis,
        message: message.into(),
    }
}
