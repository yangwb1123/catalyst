use rusqlite::{Connection, OptionalExtension, TransactionBehavior};

use super::{
    GroupModelAnalysisConfig, GroupModelAnalysisInspection, GroupModelAnalysisJournalCursor,
    GroupModelAnalysisProvider, GroupModelAnalysisRecord, GroupModelAnalysisRecovery,
    GroupModelAnalysisRequestConfig, GroupModelAnalysisResultArtifact, GroupModelAnalysisSource,
    GroupModelAnalysisStatus, GroupRunSnapshot, HubEntity, HubStoreError,
    MAX_GROUP_MODEL_ANALYSIS_EVENTS, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES, MAX_GROUP_MODEL_ANALYSIS_JOURNAL_BYTES,
    MAX_GROUP_MODEL_ANALYSIS_LIST_LIMIT, codec, group_run_codec, group_run_read, read_error, rows,
};

pub(super) struct ValidatedAnalysis {
    pub stored: rows::RawStoredAnalysis,
    pub request_config: GroupModelAnalysisRequestConfig,
    pub cursor: GroupModelAnalysisJournalCursor,
    pub inspection: GroupModelAnalysisInspection,
}

pub(super) fn inspect(
    connection: &mut Connection,
    analysis_id: &str,
) -> Result<GroupModelAnalysisInspection, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;
    let stored = rows::find_by_id(&transaction, analysis_id)?
        .ok_or_else(|| not_found(HubEntity::GroupModelAnalysis, analysis_id))?;
    let inspection = validate_stored(&transaction, stored)?.inspection;
    transaction.commit().map_err(read_error)?;
    Ok(inspection)
}

pub(super) fn list(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<Vec<GroupModelAnalysisRecord>, HubStoreError> {
    validate_list_request(connection, group_run_id, limit)?;
    let limit = i64::try_from(limit).map_err(|error| conflict(&error.to_string()))?;
    rows::query_metadata(connection, group_run_id, limit)?
        .into_iter()
        .map(metadata_record)
        .collect()
}

pub(super) fn validate_stored(
    connection: &Connection,
    stored: rows::RawStoredAnalysis,
) -> Result<ValidatedAnalysis, HubStoreError> {
    validate_stored_header(&stored)?;
    let record = metadata_record(stored.metadata.clone())?;
    let source = load_source(connection, &record.group_run_id)?;
    let request_config = validate_config_and_request(&stored, &record, &source)?;
    let (events, journal_bytes) = load_events(connection, &record.analysis_id)?;
    let result = load_result(connection, &record.analysis_id)?;
    let inspection = validate_inspection(record.clone(), events, result)?;
    validate_source_binding(&source, &inspection)?;
    let cursor = validate_journal(&stored, &record, &inspection, journal_bytes)?;
    Ok(ValidatedAnalysis {
        stored,
        request_config,
        cursor,
        inspection,
    })
}

pub(super) fn source_from_snapshot(source: &GroupRunSnapshot) -> GroupModelAnalysisSource {
    GroupModelAnalysisSource {
        group_run_version: source.run.v,
        group_run_id: source.run.run_id.clone(),
        group_id: source.run.group_id.clone(),
        context_version: source.run.context_version,
        context_slice_sha256: source.run.context_slice_sha256.clone(),
        snapshot_sha256: source.run.snapshot_sha256.clone(),
        snapshot_bytes: source.run.snapshot_bytes,
    }
}

fn metadata_record(
    raw: rows::RawAnalysisMetadata,
) -> Result<GroupModelAnalysisRecord, HubStoreError> {
    let config = metadata_config(&raw)?;
    let record = GroupModelAnalysisRecord {
        v: convert(raw.analysis_version, "analysis version")?,
        analysis_id: raw.id,
        group_run_id: raw.group_run_id,
        status: parse_status(&raw.status)?,
        source_snapshot_sha256: digest(&raw.source_snapshot_sha256, "source snapshot")?,
        config,
        config_sha256: digest(&raw.config_sha256, "configuration")?,
        request_sha256: digest(&raw.request_sha256, "request")?,
        request_bytes: convert(raw.request_bytes, "request byte count")?,
        protocol_version: convert(raw.protocol_version, "protocol version")?,
        created_at_ms: convert(raw.created_at_ms, "creation time")?,
    };
    GroupModelAnalysisJournalCursor::new(&record).map_err(|error| corrupt(&error.message))?;
    Ok(record)
}

fn metadata_config(
    raw: &rows::RawAnalysisMetadata,
) -> Result<GroupModelAnalysisConfig, HubStoreError> {
    Ok(GroupModelAnalysisConfig {
        v: convert(raw.analysis_version, "configuration version")?,
        provider: parse_provider(&raw.provider)?,
        endpoint: raw.endpoint.clone(),
        model: raw.model.clone(),
        system_prompt_version: convert(raw.system_prompt_version, "system Prompt version")?,
        system_prompt_sha256: digest(&raw.system_prompt_sha256, "system Prompt")?,
        max_output_tokens: convert(raw.max_output_tokens, "output token limit")?,
        max_model_output_bytes: convert(raw.max_model_output_bytes, "model output byte limit")?,
        max_model_events: convert(raw.max_model_events, "model event limit")?,
    })
}

fn validate_stored_header(stored: &rows::RawStoredAnalysis) -> Result<(), HubStoreError> {
    let journal_bytes: usize = convert(stored.journal_bytes, "journal byte count")?;
    let key_valid = group_run_codec::valid_text(
        &stored.idempotency_key,
        MAX_GROUP_MODEL_ANALYSIS_IDEMPOTENCY_KEY_BYTES,
    );
    if key_valid && (1..=MAX_GROUP_MODEL_ANALYSIS_JOURNAL_BYTES).contains(&journal_bytes) {
        Ok(())
    } else {
        Err(corrupt(
            "stored Group Model Analysis key or journal byte count is invalid",
        ))
    }
}

fn validate_config_and_request(
    stored: &rows::RawStoredAnalysis,
    record: &GroupModelAnalysisRecord,
    source: &GroupRunSnapshot,
) -> Result<GroupModelAnalysisRequestConfig, HubStoreError> {
    let config_digest = codec::digest_bytes(&record.config_sha256, "configuration")?;
    let request_config = codec::decode_config(&stored.config_json, &config_digest)?;
    if codec::project_config(&request_config).map_err(|error| as_corrupt(&error))? != record.config
    {
        return Err(corrupt(
            "stored private configuration disagrees with its metadata",
        ));
    }
    validate_request_bytes(stored, record, source, &request_config)?;
    Ok(request_config)
}

fn validate_request_bytes(
    stored: &rows::RawStoredAnalysis,
    record: &GroupModelAnalysisRecord,
    source: &GroupRunSnapshot,
    config: &GroupModelAnalysisRequestConfig,
) -> Result<(), HubStoreError> {
    if stored.request_body.len() != record.request_bytes {
        return Err(corrupt("stored analysis request byte count disagrees"));
    }
    let expected = codec::digest_bytes(&record.request_sha256, "request")?;
    let actual = codec::request_digest(&stored.request_body).map_err(|error| as_corrupt(&error))?;
    if actual != expected {
        return Err(corrupt("stored analysis request digest disagrees"));
    }
    codec::validate_exact_request(config, source, &stored.request_body)
}

pub(super) fn load_source(
    connection: &Connection,
    group_run_id: &str,
) -> Result<GroupRunSnapshot, HubStoreError> {
    let stored = group_run_read::find_by_id(connection, group_run_id)?
        .ok_or_else(|| corrupt("analysis references a missing frozen Group Run"))?;
    group_run_read::decode_stored(stored)
}

fn load_events(
    connection: &Connection,
    analysis_id: &str,
) -> Result<(Vec<super::GroupModelAnalysisEvent>, usize), HubStoreError> {
    let mut events = Vec::new();
    let mut bytes = 0_usize;
    for (sequence, json, digest) in rows::load_event_rows(connection, analysis_id)? {
        if events.len() >= MAX_GROUP_MODEL_ANALYSIS_EVENTS {
            return Err(corrupt("stored analysis has too many events"));
        }
        bytes = bytes
            .checked_add(json.len())
            .ok_or_else(|| corrupt("stored analysis journal byte count overflowed"))?;
        if bytes > MAX_GROUP_MODEL_ANALYSIS_JOURNAL_BYTES {
            return Err(corrupt("stored analysis journal exceeds its byte limit"));
        }
        events.push(codec::decode_event(sequence, &json, &digest)?);
    }
    Ok((events, bytes))
}

fn load_result(
    connection: &Connection,
    analysis_id: &str,
) -> Result<Option<GroupModelAnalysisResultArtifact>, HubStoreError> {
    let Some(raw) = rows::load_result(connection, analysis_id)? else {
        return Ok(None);
    };
    let version: u16 = convert(raw.result_version, "result version")?;
    let bytes: usize = convert(raw.result_bytes, "result byte count")?;
    let created_at_ms = convert(raw.created_at_ms, "result creation time")?;
    if raw.analysis_id != analysis_id
        || version != super::GROUP_MODEL_ANALYSIS_RESULT_VERSION
        || bytes != raw.result_blob.len()
    {
        return Err(corrupt("stored analysis result row binding is invalid"));
    }
    codec::decode_result(&raw.result_blob, &raw.result_sha256, created_at_ms).map(Some)
}

fn validate_inspection(
    record: GroupModelAnalysisRecord,
    events: Vec<super::GroupModelAnalysisEvent>,
    result: Option<GroupModelAnalysisResultArtifact>,
) -> Result<GroupModelAnalysisInspection, HubStoreError> {
    if events.is_empty() {
        return Err(corrupt("stored analysis is missing its prepared event"));
    }
    let inspection = GroupModelAnalysisInspection::validate(record, events, result)
        .map_err(|error| corrupt(&error.message))?;
    if inspection.recovery == GroupModelAnalysisRecovery::Unprepared {
        Err(corrupt(
            "stored analysis has an impossible unprepared state",
        ))
    } else {
        Ok(inspection)
    }
}

fn validate_source_binding(
    source: &GroupRunSnapshot,
    inspection: &GroupModelAnalysisInspection,
) -> Result<(), HubStoreError> {
    let Some(receipt) = &inspection.prepared else {
        return Err(corrupt("stored analysis has no source receipt"));
    };
    if receipt.source == source_from_snapshot(source) {
        Ok(())
    } else {
        Err(corrupt(
            "stored analysis source does not match its frozen Group Run",
        ))
    }
}

fn validate_journal(
    stored: &rows::RawStoredAnalysis,
    record: &GroupModelAnalysisRecord,
    inspection: &GroupModelAnalysisInspection,
    actual_bytes: usize,
) -> Result<GroupModelAnalysisJournalCursor, HubStoreError> {
    let persisted = codec::decode_cursor(&stored.cursor_json, record)?;
    let mut rebuilt =
        GroupModelAnalysisJournalCursor::new(record).map_err(|error| corrupt(&error.message))?;
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
            "stored analysis cursor or byte count disagrees with its journal",
        ))
    }
}

fn validate_list_request(
    connection: &Connection,
    group_run_id: Option<&str>,
    limit: usize,
) -> Result<(), HubStoreError> {
    if !(1..=MAX_GROUP_MODEL_ANALYSIS_LIST_LIMIT).contains(&limit) {
        return Err(conflict("analysis list limit is outside its bounds"));
    }
    let Some(id) = group_run_id else {
        return Ok(());
    };
    if !group_run_codec::valid_text(id, MAX_GROUP_MODEL_ANALYSIS_ID_BYTES) {
        return Err(conflict("Group Run filter is outside its bounds"));
    }
    connection
        .query_row("SELECT 1 FROM group_runs WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| not_found(HubEntity::GroupRun, id))
}

fn parse_provider(value: &str) -> Result<GroupModelAnalysisProvider, HubStoreError> {
    match value {
        "openai_responses" => Ok(GroupModelAnalysisProvider::OpenAiResponses),
        _ => Err(corrupt("stored analysis provider is unsupported")),
    }
}

fn parse_status(value: &str) -> Result<GroupModelAnalysisStatus, HubStoreError> {
    match value {
        "awaiting_consent" => Ok(GroupModelAnalysisStatus::AwaitingConsent),
        "dispatch_unknown" => Ok(GroupModelAnalysisStatus::DispatchUnknown),
        "completed" => Ok(GroupModelAnalysisStatus::Completed),
        _ => Err(corrupt("stored analysis status is unsupported")),
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
        entity: HubEntity::GroupModelAnalysis,
        message: message.into(),
    }
}
