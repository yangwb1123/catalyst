use std::time::Duration;

use rusqlite::{Connection, Transaction, TransactionBehavior};

use super::{
    ClaimGroupPanelSynthesisDispatch, ClaimGroupPanelSynthesisDispatchResult,
    GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION, GROUP_PANEL_SYNTHESIS_VERSION,
    GroupPanelSynthesisDispatchAuthority, GroupPanelSynthesisDispatchClaim,
    GroupPanelSynthesisEvent, GroupPanelSynthesisEventKind, GroupPanelSynthesisInspection,
    GroupPanelSynthesisJournalCursor, GroupPanelSynthesisPreparedReceipt,
    GroupPanelSynthesisRecord, GroupPanelSynthesisRecovery, GroupPanelSynthesisStatus, HubEntity,
    HubStoreError, MAX_GROUP_PANEL_SYNTHESIS_JOURNAL_BYTES, PrepareGroupPanelSynthesis,
    PrepareGroupPanelSynthesisDisposition, PrepareGroupPanelSynthesisResult, codec, read,
    read_error, rows, sql,
};

const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

struct CandidateEncoding {
    config: [u8; 32],
    request: [u8; 32],
    source: [u8; 32],
    manifest: [u8; 32],
    system_prompt: [u8; 32],
}

pub(super) fn prepare(
    connection: &mut Connection,
    request: &PrepareGroupPanelSynthesis,
) -> Result<PrepareGroupPanelSynthesisResult, HubStoreError> {
    request
        .validate()
        .map_err(|error| conflict(&error.message))?;
    let transaction = immediate(connection)?;
    let result = prepare_locked(&transaction, request)?;
    transaction.commit().map_err(read_error)?;
    Ok(result)
}

pub(super) fn claim(
    connection: &mut Connection,
    request: &ClaimGroupPanelSynthesisDispatch,
) -> Result<ClaimGroupPanelSynthesisDispatchResult, HubStoreError> {
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
    request: &PrepareGroupPanelSynthesis,
) -> Result<PrepareGroupPanelSynthesisResult, HubStoreError> {
    if let Some(stored) = rows::find_by_key(transaction, &request.idempotency_key)? {
        let validated = read::validate_stored(transaction, stored)?;
        ensure_prepare_replay(&validated, request)?;
        return Ok(prepare_result(
            PrepareGroupPanelSynthesisDisposition::Replayed,
            validated.inspection,
        ));
    }
    let encoding = validate_candidate(transaction, request)?;
    if let Some(stored) = rows::find_by_id(transaction, &request.synthesis_id)? {
        read::validate_stored(transaction, stored)?;
        return Err(conflict(
            "synthesis ID already belongs to another idempotency key",
        ));
    }
    create_prepared(transaction, request, &encoding)
}

fn validate_candidate(
    transaction: &Transaction<'_>,
    request: &PrepareGroupPanelSynthesis,
) -> Result<CandidateEncoding, HubStoreError> {
    let panel = read::load_panel_candidate(transaction, &request.source.panel_id)?;
    if read::source_from_panel(&panel) != request.source {
        return Err(conflict(
            "synthesis source does not match its frozen analysis panel",
        ));
    }
    validate_candidate_config(request)?;
    let expected_body = codec::encode_exact_request(&request.request_config, &panel)?;
    if expected_body != request.request_body {
        return Err(conflict(
            "synthesis request body is not the exact frozen request",
        ));
    }
    candidate_encoding(request)
}

fn validate_candidate_config(request: &PrepareGroupPanelSynthesis) -> Result<(), HubStoreError> {
    let encoded = codec::encode_config(&request.request_config)?;
    let projected = codec::project_config(&request.request_config)?;
    if encoded.json != request.config_json
        || codec::digest_hex(&encoded.digest) != request.config_sha256
        || projected != request.config
    {
        return Err(conflict(
            "synthesis configuration projection, bytes, or digest disagrees",
        ));
    }
    Ok(())
}

fn candidate_encoding(
    request: &PrepareGroupPanelSynthesis,
) -> Result<CandidateEncoding, HubStoreError> {
    let request_digest = codec::request_digest(&request.request_body)?;
    if codec::digest_hex(&request_digest) != request.request_sha256 {
        return Err(conflict(
            "synthesis request digest disagrees with its bytes",
        ));
    }
    Ok(CandidateEncoding {
        config: candidate_digest(&request.config_sha256, "configuration")?,
        request: request_digest,
        source: candidate_digest(&request.source.source_snapshot_sha256, "source snapshot")?,
        manifest: candidate_digest(&request.source.panel_manifest_sha256, "panel manifest")?,
        system_prompt: codec::system_prompt_digest(&request.request_config.system_prompt),
    })
}

fn candidate_digest(value: &str, subject: &str) -> Result<[u8; 32], HubStoreError> {
    codec::digest_bytes(value, subject).map_err(|error| conflict(&error.to_string()))
}

fn ensure_prepare_replay(
    existing: &read::ValidatedSynthesis,
    request: &PrepareGroupPanelSynthesis,
) -> Result<(), HubStoreError> {
    let prepared = existing
        .inspection
        .prepared
        .as_ref()
        .ok_or_else(|| corrupt("stored synthesis is missing its prepared receipt"))?;
    let record = &existing.inspection.synthesis;
    let exact = existing.stored.idempotency_key == request.idempotency_key
        && prepared.source == request.source
        && existing.request_config == request.request_config
        && record.config == request.config
        && existing.stored.config_json == request.config_json
        && record.config_sha256 == request.config_sha256
        && existing.stored.request_body == request.request_body
        && record.request_sha256 == request.request_sha256;
    exact
        .then_some(())
        .ok_or_else(|| conflict("idempotency key was reused with different synthesis input"))
}

fn create_prepared(
    transaction: &Transaction<'_>,
    request: &PrepareGroupPanelSynthesis,
    encoding: &CandidateEncoding,
) -> Result<PrepareGroupPanelSynthesisResult, HubStoreError> {
    let record = prepared_record(request);
    let event = prepared_event(request);
    let encoded_event = codec::encode_event(&event)?;
    let mut cursor =
        GroupPanelSynthesisJournalCursor::new(&record).map_err(|error| conflict(&error.message))?;
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
    let expected = GroupPanelSynthesisInspection::validate(record, vec![event], None)
        .map_err(|error| corrupt(&error.message))?;
    let inspection = reread_validated(transaction, &request.synthesis_id)?.inspection;
    if inspection != expected {
        return Err(corrupt(
            "persisted synthesis preparation disagrees with its committed candidate",
        ));
    }
    Ok(prepare_result(
        PrepareGroupPanelSynthesisDisposition::Created,
        inspection,
    ))
}

fn prepared_record(request: &PrepareGroupPanelSynthesis) -> GroupPanelSynthesisRecord {
    GroupPanelSynthesisRecord {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: request.synthesis_id.clone(),
        panel_id: request.source.panel_id.clone(),
        group_run_id: request.source.group_run_id.clone(),
        status: GroupPanelSynthesisStatus::AwaitingConsent,
        source_snapshot_sha256: request.source.source_snapshot_sha256.clone(),
        panel_manifest_sha256: request.source.panel_manifest_sha256.clone(),
        config: request.config.clone(),
        config_sha256: request.config_sha256.clone(),
        request_sha256: request.request_sha256.clone(),
        request_bytes: request.request_body.len(),
        protocol_version: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        created_at_ms: request.created_at_ms,
    }
}

fn prepared_event(request: &PrepareGroupPanelSynthesis) -> GroupPanelSynthesisEvent {
    GroupPanelSynthesisEvent {
        v: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        synthesis_id: request.synthesis_id.clone(),
        seq: 1,
        kind: GroupPanelSynthesisEventKind::SynthesisPrepared {
            receipt: GroupPanelSynthesisPreparedReceipt {
                v: GROUP_PANEL_SYNTHESIS_VERSION,
                synthesis_id: request.synthesis_id.clone(),
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
    request: &PrepareGroupPanelSynthesis,
    record: &GroupPanelSynthesisRecord,
    encoding: &CandidateEncoding,
    event: &codec::EncodedJson,
    cursor_json: &str,
) -> Result<(), HubStoreError> {
    let journal_bytes = to_i64(event.json.len(), "journal byte count")?;
    let row = new_synthesis_row(request, record, encoding, cursor_json, journal_bytes)?;
    sql::insert_synthesis(transaction, &row)?;
    sql::insert_event(
        transaction,
        &sql::EventRow {
            synthesis_id: &record.synthesis_id,
            sequence: 1,
            json: &event.json,
            sha256: &event.digest,
        },
    )
}

fn new_synthesis_row<'a>(
    request: &'a PrepareGroupPanelSynthesis,
    record: &'a GroupPanelSynthesisRecord,
    encoding: &'a CandidateEncoding,
    cursor_json: &'a str,
    journal_bytes: i64,
) -> Result<sql::NewSynthesisRow<'a>, HubStoreError> {
    Ok(sql::NewSynthesisRow {
        id: &record.synthesis_id,
        panel_id: &record.panel_id,
        group_run_id: &record.group_run_id,
        synthesis_version: i64::from(record.v),
        source_snapshot_sha256: &encoding.source,
        panel_manifest_sha256: &encoding.manifest,
        provider: "openai_responses",
        endpoint: &record.config.endpoint,
        model: &record.config.model,
        system_prompt_version: i64::from(record.config.system_prompt_version),
        system_prompt_sha256: &encoding.system_prompt,
        output_target: "local_artifact",
        writeback_target: "none",
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
    request: &ClaimGroupPanelSynthesisDispatch,
) -> Result<ClaimGroupPanelSynthesisDispatchResult, HubStoreError> {
    let stored = rows::find_by_id(transaction, &request.synthesis_id)?
        .ok_or_else(|| not_found(&request.synthesis_id))?;
    let validated = read::validate_stored(transaction, stored)?;
    if validated.inspection.recovery != GroupPanelSynthesisRecovery::AwaitingConsent {
        return Ok(ClaimGroupPanelSynthesisDispatchResult::AlreadyClaimed {
            inspection: validated.inspection,
        });
    }
    claim_awaiting(transaction, request, validated)
}

fn claim_awaiting(
    transaction: &Transaction<'_>,
    request: &ClaimGroupPanelSynthesisDispatch,
    mut validated: read::ValidatedSynthesis,
) -> Result<ClaimGroupPanelSynthesisDispatchResult, HubStoreError> {
    let record = validated.inspection.synthesis.clone();
    let claim = dispatch_claim(&record, request);
    let event = GroupPanelSynthesisEvent {
        v: GROUP_PANEL_SYNTHESIS_PROTOCOL_VERSION,
        synthesis_id: record.synthesis_id.clone(),
        seq: validated.cursor.next_sequence(),
        kind: GroupPanelSynthesisEventKind::ProviderDispatchReleased {
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
    let reread = reread_validated(transaction, &record.synthesis_id)?;
    let valid = reread.inspection.dispatch.as_ref() == Some(&claim)
        && matches!(
            reread.inspection.recovery,
            GroupPanelSynthesisRecovery::DispatchUnknown { .. }
        );
    if !valid {
        return Err(corrupt(
            "persisted synthesis dispatch disagrees with its released claim",
        ));
    }
    let authority = GroupPanelSynthesisDispatchAuthority::new(
        &reread.inspection.synthesis,
        claim,
        reread.stored.request_body,
    )
    .map_err(|error| corrupt(&error.message))?;
    Ok(ClaimGroupPanelSynthesisDispatchResult::Claimed { authority })
}

fn dispatch_claim(
    record: &GroupPanelSynthesisRecord,
    request: &ClaimGroupPanelSynthesisDispatch,
) -> GroupPanelSynthesisDispatchClaim {
    GroupPanelSynthesisDispatchClaim {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
        synthesis_id: record.synthesis_id.clone(),
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
    validated: &mut read::ValidatedSynthesis,
    event: &GroupPanelSynthesisEvent,
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
            synthesis_id: &validated.inspection.synthesis.synthesis_id,
            sequence: to_i64(event.seq, "event sequence")?,
            json: &encoded.json,
            sha256: &encoded.digest,
        },
    )?;
    sql::update_journal(
        transaction,
        &validated.inspection.synthesis.synthesis_id,
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
        .ok_or_else(|| conflict("synthesis journal byte count overflowed"))?;
    if total > MAX_GROUP_PANEL_SYNTHESIS_JOURNAL_BYTES {
        return Err(conflict("synthesis journal exceeds its durable byte limit"));
    }
    to_i64(total, "journal byte count")
}

fn prepare_result(
    disposition: PrepareGroupPanelSynthesisDisposition,
    inspection: GroupPanelSynthesisInspection,
) -> PrepareGroupPanelSynthesisResult {
    PrepareGroupPanelSynthesisResult {
        v: GROUP_PANEL_SYNTHESIS_VERSION,
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

pub(super) fn reread_validated(
    transaction: &Transaction<'_>,
    synthesis_id: &str,
) -> Result<read::ValidatedSynthesis, HubStoreError> {
    let stored = rows::find_by_id(transaction, synthesis_id)?
        .ok_or_else(|| corrupt("persisted synthesis disappeared inside its transaction"))?;
    read::validate_stored(transaction, stored)
}

pub(super) fn not_found(id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::GroupPanelSynthesis,
        id: id.into(),
    }
}

pub(super) fn corrupt(message: &str) -> HubStoreError {
    HubStoreError::Corrupt {
        message: message.into(),
    }
}

pub(super) fn conflict(message: &str) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::GroupPanelSynthesis,
        message: message.into(),
    }
}
