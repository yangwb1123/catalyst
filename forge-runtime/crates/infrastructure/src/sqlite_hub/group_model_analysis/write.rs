use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior};

use super::{
    ClaimGroupModelAnalysisDispatch, ClaimGroupModelAnalysisDispatchResult,
    GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION, GROUP_MODEL_ANALYSIS_VERSION,
    GroupModelAnalysisDispatchAuthority, GroupModelAnalysisDispatchClaim, GroupModelAnalysisEvent,
    GroupModelAnalysisEventKind, GroupModelAnalysisInspection, GroupModelAnalysisJournalCursor,
    GroupModelAnalysisPreparedReceipt, GroupModelAnalysisRecord, GroupModelAnalysisRecovery,
    GroupModelAnalysisStatus, HubEntity, HubStoreError, MAX_GROUP_MODEL_ANALYSIS_JOURNAL_BYTES,
    PrepareGroupModelAnalysis, PrepareGroupModelAnalysisDisposition,
    PrepareGroupModelAnalysisResult, codec, read, read_error, rows, sql,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

struct CandidateEncoding {
    config: [u8; 32],
    request: [u8; 32],
    source: [u8; 32],
    system_prompt: [u8; 32],
}

pub(super) fn prepare(
    connection: &mut Connection,
    request: &PrepareGroupModelAnalysis,
) -> Result<PrepareGroupModelAnalysisResult, HubStoreError> {
    validate_prepare_request(request)?;
    let transaction = immediate(connection)?;
    let result = prepare_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

pub(super) fn claim(
    connection: &mut Connection,
    request: &ClaimGroupModelAnalysisDispatch,
) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.message))?;
    let transaction = immediate(connection)?;
    let result = claim_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

pub(super) fn immediate(connection: &mut Connection) -> Result<Transaction<'_>, HubStoreError> {
    connection.busy_timeout(BUSY_TIMEOUT).map_err(read_error)?;
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(read_error)
}

fn prepare_locked(
    transaction: &Transaction<'_>,
    request: &PrepareGroupModelAnalysis,
) -> Result<PrepareGroupModelAnalysisResult, HubStoreError> {
    let encoding = validate_candidate(transaction, request)?;
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let validated = read::validate_stored(transaction, stored)?;
        ensure_prepare_replay(&validated, request)?;
        return Ok(prepare_result(
            PrepareGroupModelAnalysisDisposition::Replayed,
            validated.inspection,
        ));
    }
    if let Some(stored) = rows::find_by_id(transaction, &request.analysis_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "analysis ID already belongs to another idempotency key",
        ));
    }
    create_prepared(transaction, request, &encoding)
}

fn validate_prepare_request(request: &PrepareGroupModelAnalysis) -> Result<(), HubStoreError> {
    request.validate().map_err(|error| conflict(&error.message))
}

fn validate_candidate(
    transaction: &Transaction<'_>,
    request: &PrepareGroupModelAnalysis,
) -> Result<CandidateEncoding, HubStoreError> {
    let source = read::load_source(transaction, &request.source.group_run_id)?;
    if read::source_from_snapshot(&source) != request.source {
        return Err(conflict(
            "analysis source does not match its frozen Group Run",
        ));
    }
    validate_candidate_config(request)?;
    let expected_body = codec::encode_exact_request(&request.request_config, &source)?;
    if expected_body != request.request_body {
        return Err(conflict(
            "analysis request body is not the exact frozen request",
        ));
    }
    candidate_encoding(request)
}

fn validate_candidate_config(request: &PrepareGroupModelAnalysis) -> Result<(), HubStoreError> {
    let encoded = codec::encode_config(&request.request_config)?;
    let projected = codec::project_config(&request.request_config)?;
    if encoded.json != request.config_json
        || codec::digest_hex(&encoded.digest) != request.config_sha256
        || projected != request.config
    {
        return Err(conflict(
            "analysis configuration projection, bytes, or digest disagrees",
        ));
    }
    Ok(())
}

fn candidate_encoding(
    request: &PrepareGroupModelAnalysis,
) -> Result<CandidateEncoding, HubStoreError> {
    let request_digest = codec::request_digest(&request.request_body)?;
    if codec::digest_hex(&request_digest) != request.request_sha256 {
        return Err(conflict("analysis request digest disagrees with its bytes"));
    }
    Ok(CandidateEncoding {
        config: codec::digest_bytes(&request.config_sha256, "configuration")
            .map_err(|error| as_conflict(&error))?,
        request: request_digest,
        source: codec::digest_bytes(&request.source.snapshot_sha256, "source snapshot")
            .map_err(|error| as_conflict(&error))?,
        system_prompt: codec::system_prompt_digest(&request.request_config.system_prompt),
    })
}

fn ensure_prepare_replay(
    existing: &read::ValidatedAnalysis,
    request: &PrepareGroupModelAnalysis,
) -> Result<(), HubStoreError> {
    let prepared = existing
        .inspection
        .prepared
        .as_ref()
        .ok_or_else(|| corrupt("stored analysis is missing its prepared receipt"))?;
    let exact = existing.stored.idempotency_key == request.idempotency_key
        && prepared.source == request.source
        && existing.request_config == request.request_config
        && existing.inspection.analysis.config == request.config
        && existing.stored.config_json == request.config_json
        && existing.inspection.analysis.config_sha256 == request.config_sha256
        && existing.stored.request_body == request.request_body
        && existing.inspection.analysis.request_sha256 == request.request_sha256;
    exact
        .then_some(())
        .ok_or_else(|| conflict("idempotency key was reused with different analysis input"))
}

fn create_prepared(
    transaction: &Transaction<'_>,
    request: &PrepareGroupModelAnalysis,
    encoding: &CandidateEncoding,
) -> Result<PrepareGroupModelAnalysisResult, HubStoreError> {
    let record = prepared_record(request);
    let event = prepared_event(request);
    let encoded_event = codec::encode_event(&event)?;
    let mut cursor =
        GroupModelAnalysisJournalCursor::new(&record).map_err(|error| conflict(&error.message))?;
    cursor
        .append(&event)
        .map_err(|error| conflict(&error.message))?;
    let cursor_json = codec::encode_cursor(&cursor)?;
    insert_prepared(
        transaction,
        request,
        &record,
        encoding,
        &encoded_event,
        &cursor_json,
    )?;
    let inspection = GroupModelAnalysisInspection::validate(record, vec![event], None)
        .map_err(|error| corrupt(&error.message))?;
    Ok(prepare_result(
        PrepareGroupModelAnalysisDisposition::Created,
        inspection,
    ))
}

fn prepared_record(request: &PrepareGroupModelAnalysis) -> GroupModelAnalysisRecord {
    GroupModelAnalysisRecord {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: request.analysis_id.clone(),
        group_run_id: request.source.group_run_id.clone(),
        status: GroupModelAnalysisStatus::AwaitingConsent,
        source_snapshot_sha256: request.source.snapshot_sha256.clone(),
        config: request.config.clone(),
        config_sha256: request.config_sha256.clone(),
        request_sha256: request.request_sha256.clone(),
        request_bytes: request.request_body.len(),
        protocol_version: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        created_at_ms: request.created_at_ms,
    }
}

fn prepared_event(request: &PrepareGroupModelAnalysis) -> GroupModelAnalysisEvent {
    GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: request.analysis_id.clone(),
        seq: 1,
        kind: GroupModelAnalysisEventKind::AnalysisPrepared {
            receipt: GroupModelAnalysisPreparedReceipt {
                v: GROUP_MODEL_ANALYSIS_VERSION,
                analysis_id: request.analysis_id.clone(),
                source: request.source.clone(),
                config_sha256: request.config_sha256.clone(),
                request_sha256: request.request_sha256.clone(),
                request_bytes: request.request_body.len(),
            },
        },
    }
}

fn insert_prepared(
    transaction: &Transaction<'_>,
    request: &PrepareGroupModelAnalysis,
    record: &GroupModelAnalysisRecord,
    encoding: &CandidateEncoding,
    event: &codec::EncodedJson,
    cursor_json: &str,
) -> Result<(), HubStoreError> {
    let journal_bytes = to_i64(event.json.len(), "journal byte count")?;
    let row = new_analysis_row(request, record, encoding, cursor_json, journal_bytes)?;
    sql::insert_analysis(transaction, &row)?;
    sql::insert_event(
        transaction,
        &sql::EventRow {
            analysis_id: &record.analysis_id,
            sequence: 1,
            json: &event.json,
            sha256: &event.digest,
        },
    )
}

fn new_analysis_row<'a>(
    request: &'a PrepareGroupModelAnalysis,
    record: &'a GroupModelAnalysisRecord,
    encoding: &'a CandidateEncoding,
    cursor_json: &'a str,
    journal_bytes: i64,
) -> Result<sql::NewAnalysisRow<'a>, HubStoreError> {
    Ok(sql::NewAnalysisRow {
        id: &record.analysis_id,
        group_run_id: &record.group_run_id,
        analysis_version: i64::from(record.v),
        source_snapshot_sha256: &encoding.source,
        provider: "openai_responses",
        endpoint: &record.config.endpoint,
        model: &record.config.model,
        system_prompt_version: i64::from(record.config.system_prompt_version),
        system_prompt_sha256: &encoding.system_prompt,
        max_output_tokens: i64::from(record.config.max_output_tokens),
        max_model_output_bytes: to_i64(record.config.max_model_output_bytes, "output byte limit")?,
        max_model_events: i64::from(record.config.max_model_events),
        config_json: &request.config_json,
        config_sha256: &encoding.config,
        request_body: &request.request_body,
        request_sha256: &encoding.request,
        cursor_json,
        journal_bytes,
        idempotency_key: &request.idempotency_key,
        protocol_version: i64::from(record.protocol_version),
        created_at_ms: to_i64(record.created_at_ms, "creation time")?,
    })
}

fn claim_locked(
    transaction: &Transaction<'_>,
    request: &ClaimGroupModelAnalysisDispatch,
) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError> {
    let stored = rows::find_by_id(transaction, &request.analysis_id)?
        .ok_or_else(|| not_found(&request.analysis_id))?;
    let validated = read::validate_stored(transaction, stored)?;
    if validated.inspection.recovery != GroupModelAnalysisRecovery::AwaitingConsent {
        return Ok(ClaimGroupModelAnalysisDispatchResult::AlreadyClaimed {
            inspection: validated.inspection,
        });
    }
    claim_awaiting(transaction, request, validated)
}

fn claim_awaiting(
    transaction: &Transaction<'_>,
    request: &ClaimGroupModelAnalysisDispatch,
    mut validated: read::ValidatedAnalysis,
) -> Result<ClaimGroupModelAnalysisDispatchResult, HubStoreError> {
    let record = validated.inspection.analysis.clone();
    let claim = dispatch_claim(&record, request);
    let event = GroupModelAnalysisEvent {
        v: GROUP_MODEL_ANALYSIS_PROTOCOL_VERSION,
        analysis_id: record.analysis_id.clone(),
        seq: validated.cursor.next_sequence(),
        kind: GroupModelAnalysisEventKind::ProviderDispatchReleased {
            claim: claim.clone(),
        },
    };
    append_transition(
        transaction,
        &mut validated,
        &event,
        "awaiting_consent",
        "dispatch_unknown",
    )?;
    let authority = GroupModelAnalysisDispatchAuthority::new(
        &record,
        claim,
        validated.stored.request_body.clone(),
    )
    .map_err(|error| corrupt(&error.message))?;
    Ok(ClaimGroupModelAnalysisDispatchResult::Claimed { authority })
}

fn dispatch_claim(
    record: &GroupModelAnalysisRecord,
    request: &ClaimGroupModelAnalysisDispatch,
) -> GroupModelAnalysisDispatchClaim {
    GroupModelAnalysisDispatchClaim {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        analysis_id: record.analysis_id.clone(),
        dispatch_id: request.dispatch_id.clone(),
        request_sha256: record.request_sha256.clone(),
        config_sha256: record.config_sha256.clone(),
        provider: record.config.provider,
        endpoint: record.config.endpoint.clone(),
        model: record.config.model.clone(),
        consent_version: request.consent_version,
        released_at_ms: request.released_at_ms,
    }
}

pub(super) fn append_transition(
    transaction: &Transaction<'_>,
    validated: &mut read::ValidatedAnalysis,
    event: &GroupModelAnalysisEvent,
    expected_status: &str,
    new_status: &str,
) -> Result<(), HubStoreError> {
    validated
        .cursor
        .append(event)
        .map_err(|error| conflict(&error.message))?;
    let encoded = codec::encode_event(event)?;
    let journal_bytes = extended_journal_bytes(validated.stored.journal_bytes, encoded.json.len())?;
    let cursor_json = codec::encode_cursor(&validated.cursor)?;
    sql::insert_event(
        transaction,
        &sql::EventRow {
            analysis_id: &validated.inspection.analysis.analysis_id,
            sequence: to_i64(event.seq, "event sequence")?,
            json: &encoded.json,
            sha256: &encoded.digest,
        },
    )?;
    sql::update_journal(
        transaction,
        &validated.inspection.analysis.analysis_id,
        expected_status,
        new_status,
        &cursor_json,
        journal_bytes,
    )
}

fn extended_journal_bytes(current: i64, added: usize) -> Result<i64, HubStoreError> {
    let current: usize = usize::try_from(current)
        .map_err(|error| corrupt(&format!("invalid journal byte count: {error}")))?;
    let total = current
        .checked_add(added)
        .ok_or_else(|| conflict("analysis journal byte count overflowed"))?;
    if total > MAX_GROUP_MODEL_ANALYSIS_JOURNAL_BYTES {
        return Err(conflict("analysis journal exceeds its durable byte limit"));
    }
    to_i64(total, "journal byte count")
}

fn prepare_result(
    disposition: PrepareGroupModelAnalysisDisposition,
    inspection: GroupModelAnalysisInspection,
) -> PrepareGroupModelAnalysisResult {
    PrepareGroupModelAnalysisResult {
        v: GROUP_MODEL_ANALYSIS_VERSION,
        disposition,
        inspection,
    }
}

pub(super) fn to_i64<T>(value: T, subject: &str) -> Result<i64, HubStoreError>
where
    i64: TryFrom<T>,
    <i64 as TryFrom<T>>::Error: std::fmt::Display,
{
    i64::try_from(value).map_err(|error| conflict(&format!("invalid {subject}: {error}")))
}

pub(super) fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupModelAnalysis,
        id: id.into(),
    }
}

fn as_conflict(error: &HubStoreError) -> HubStoreError {
    conflict(&error.to_string())
}

pub(super) fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

pub(super) fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupModelAnalysis,
        message: message.into(),
    }
}
