use crate::runtime_domain::{
    MAX_RUN_EVENT_JSON_BYTES, PROTOCOL_VERSION, RunOutcome, RuntimeEvent, RuntimeEventKind,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{
    ConversationOwner, HubEntity, HubStoreError, MAX_HUB_ENTITY_ID_BYTES, MAX_OWNED_RUN_PAGE_LIMIT,
    MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT, OwnedRunCursor, OwnedRunPage, OwnedRunStatus,
    OwnedRunSummary, OwnedRunTimelineEvent, OwnedRunTimelineEventType, OwnedRunTimelinePage,
    read_error,
};
mod query;
use query::{RUN_SUMMARY_QUERY, corrupt, invalid_page, not_found_conversation};

type RunSummaryRow = (String, String, i64, i64, i64);

// Keep owner check, scalar scan, aggregate event-byte budget, payload projection,
// and cursor construction together over one deferred snapshot.
#[allow(clippy::too_many_lines)]
pub(super) fn owned_run_page(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
    before: Option<&OwnedRunCursor>,
    limit: usize,
) -> Result<OwnedRunPage, HubStoreError> {
    let scan_limit = validate_run_page(limit)?;
    validate_identifier(conversation_id, "Conversation")?;
    let (cursor_time, cursor_id) = validate_cursor(before)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;

    ensure_owned_conversation(&transaction, owner, conversation_id)?;
    let candidates = query_run_summaries(
        &transaction,
        owner,
        conversation_id,
        cursor_time,
        cursor_id,
        scan_limit,
    )?;
    let (runs, has_more) = summarize_candidates(&transaction, conversation_id, candidates, limit)?;
    let next_cursor = has_more
        .then(|| {
            runs.last().map(|run| OwnedRunCursor {
                created_at_ms: run.created_at_ms,
                run_id: run.run_id.clone(),
            })
        })
        .flatten();
    transaction.commit().map_err(read_error)?;
    Ok(OwnedRunPage {
        conversation_id: conversation_id.to_owned(),
        runs,
        next_cursor,
        has_more,
    })
}

/// Reads one owner-visible Run summary directly by its durable ID. The owner
/// check and Run membership check happen in the same deferred snapshot, while
/// the returned value remains the existing payload-free scalar summary.
pub(super) fn owned_run_observation(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
    run_id: &str,
) -> Result<OwnedRunSummary, HubStoreError> {
    validate_identifier(conversation_id, "Conversation")?;
    validate_identifier(run_id, "Run")?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;

    ensure_owned_conversation(&transaction, owner, conversation_id)?;
    ensure_run_in_conversation(&transaction, conversation_id, run_id)?;
    let (run_id, prompt_id, created_at, latest_sequence, latest_event_bytes) = transaction
        .query_row(
            "SELECT r.id, r.prompt_id, r.created_at_ms,
                    COALESCE((SELECT MAX(e.seq) FROM run_events AS e
                              WHERE e.run_id = r.id), 0),
                    COALESCE((SELECT length(CAST(e.event_json AS BLOB))
                              FROM run_events AS e WHERE e.run_id = r.id
                              ORDER BY e.seq DESC LIMIT 1), 0)
             FROM runs AS r
             WHERE r.id = ?1 AND r.conversation_id = ?2",
            params![run_id, conversation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .map_err(read_error)?;
    let mut source_event_bytes = 0_usize;
    let summary = decode_run_summary(
        &transaction,
        conversation_id,
        run_id,
        prompt_id,
        created_at,
        latest_sequence,
        latest_event_bytes,
        &mut source_event_bytes,
    )?
    .ok_or_else(|| corrupt("Run observation exceeds its durable size limit"))?;
    transaction.commit().map_err(read_error)?;
    Ok(summary)
}

fn summarize_candidates(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    candidates: Vec<RunSummaryRow>,
    limit: usize,
) -> Result<(Vec<OwnedRunSummary>, bool), HubStoreError> {
    let mut has_more = candidates.len() > limit;
    let mut source_event_bytes = 0_usize;
    let mut runs = Vec::with_capacity(limit.min(candidates.len()));
    for (run_id, prompt_id, created_at, latest_sequence, latest_event_bytes) in candidates {
        if runs.len() == limit {
            has_more = true;
            break;
        }
        let summary = decode_run_summary(
            transaction,
            conversation_id,
            run_id,
            prompt_id,
            created_at,
            latest_sequence,
            latest_event_bytes,
            &mut source_event_bytes,
        )?;
        let Some(summary) = summary else {
            has_more = true;
            break;
        };
        runs.push(summary);
    }
    Ok((runs, has_more))
}

#[allow(clippy::too_many_arguments)]
fn decode_run_summary(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    run_id: String,
    prompt_id: String,
    created_at: i64,
    latest_sequence: i64,
    latest_event_bytes: i64,
    source_event_bytes: &mut usize,
) -> Result<Option<OwnedRunSummary>, HubStoreError> {
    validate_identifier(&run_id, "Run")?;
    validate_identifier(&prompt_id, "Prompt")?;
    let created_at_ms = decode_timestamp(created_at, "Run creation timestamp")?;
    let latest_sequence = decode_sequence(latest_sequence)?;
    if latest_sequence == 0 {
        return Err(corrupt("stored Run has no initial event"));
    }
    let event_bytes = decode_event_bytes(latest_event_bytes)?;
    if source_event_bytes.saturating_add(event_bytes) > MAX_RUN_EVENT_JSON_BYTES {
        return Ok(None);
    }
    *source_event_bytes += event_bytes;
    let event_json = latest_event_json(transaction, &run_id, latest_sequence, event_bytes)?;
    let (event_type, event) =
        decode_event_envelope(&event_json, &run_id, conversation_id, latest_sequence)?;
    let status = if event_type == OwnedRunTimelineEventType::RunFinished {
        decode_terminal_status(&event)?
    } else {
        OwnedRunStatus::Nonterminal
    };
    Ok(Some(OwnedRunSummary {
        run_id,
        prompt_id,
        created_at_ms,
        latest_sequence,
        status,
    }))
}

pub(super) fn owned_run_timeline_page(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
    run_id: &str,
    after_sequence: u64,
    limit: usize,
) -> Result<OwnedRunTimelinePage, HubStoreError> {
    let scan_limit = validate_timeline_page(limit)?;
    validate_identifier(conversation_id, "Conversation")?;
    validate_identifier(run_id, "Run")?;
    let after =
        i64::try_from(after_sequence).map_err(|_| invalid_page("invalid Run timeline cursor"))?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(read_error)?;

    ensure_owned_conversation(&transaction, owner, conversation_id)?;
    ensure_run_in_conversation(&transaction, conversation_id, run_id)?;
    let rows = query_event_skeletons(&transaction, run_id, after, scan_limit)?;
    let (events, has_more) = project_timeline_events(
        &transaction,
        conversation_id,
        run_id,
        after_sequence,
        rows,
        limit,
    )?;
    let scanned_through_sequence = events.last().map_or(after_sequence, |event| event.seq);
    transaction.commit().map_err(read_error)?;
    Ok(OwnedRunTimelinePage {
        conversation_id: conversation_id.to_owned(),
        run_id: run_id.to_owned(),
        after_sequence,
        scanned_through_sequence,
        has_more,
        events,
    })
}

fn project_timeline_events(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    run_id: &str,
    after_sequence: u64,
    rows: Vec<(i64, i64)>,
    limit: usize,
) -> Result<(Vec<OwnedRunTimelineEvent>, bool), HubStoreError> {
    let mut has_more = rows.len() > limit;
    let mut source_event_bytes = 0_usize;
    let mut events = Vec::with_capacity(limit.min(rows.len()));
    let mut expected_sequence = after_sequence.checked_add(1);
    for (sequence, raw_event_bytes) in rows {
        if events.len() == limit {
            has_more = true;
            break;
        }
        let Some(event) = decode_timeline_event(
            transaction,
            conversation_id,
            run_id,
            sequence,
            raw_event_bytes,
            &mut expected_sequence,
            &mut source_event_bytes,
        )?
        else {
            has_more = true;
            break;
        };
        events.push(event);
    }
    Ok((events, has_more))
}

#[allow(clippy::too_many_arguments)]
fn decode_timeline_event(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    run_id: &str,
    raw_sequence: i64,
    raw_event_bytes: i64,
    expected_sequence: &mut Option<u64>,
    source_event_bytes: &mut usize,
) -> Result<Option<OwnedRunTimelineEvent>, HubStoreError> {
    let sequence = decode_sequence(raw_sequence)?;
    if *expected_sequence != Some(sequence) {
        return Err(corrupt("Run event sequence has a gap"));
    }
    *expected_sequence = sequence.checked_add(1);
    let event_bytes = decode_event_bytes(raw_event_bytes)?;
    if source_event_bytes.saturating_add(event_bytes) > MAX_RUN_EVENT_JSON_BYTES {
        return Ok(None);
    }
    *source_event_bytes += event_bytes;
    let event_json = event_json_at(transaction, run_id, sequence, event_bytes)?;
    let (event_type, event) =
        decode_event_envelope(&event_json, run_id, conversation_id, sequence)?;
    Ok(Some(OwnedRunTimelineEvent {
        seq: sequence,
        emitted_at_ms: event.emitted_at_ms,
        event_type,
    }))
}

fn query_run_summaries(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
    cursor_time: Option<i64>,
    cursor_id: Option<&str>,
    limit: i64,
) -> Result<Vec<RunSummaryRow>, HubStoreError> {
    let mut statement = transaction.prepare(RUN_SUMMARY_QUERY).map_err(read_error)?;
    let rows = statement
        .query_map(
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                conversation_id,
                cursor_time,
                cursor_id,
                limit,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .map_err(read_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(read_error)
}

fn ensure_owned_conversation(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let owner_matches = transaction
        .query_row(
            "SELECT 1 FROM conversation_owners
             WHERE issuer = ?1 AND subject = ?2 AND tenant_id = ?3
               AND conversation_id = ?4",
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                conversation_id
            ],
            |_| Ok(()),
        )
        .optional()
        .map_err(read_error)?
        .is_some();
    if owner_matches {
        Ok(())
    } else {
        Err(not_found_conversation(conversation_id))
    }
}

fn ensure_run_in_conversation(
    transaction: &Transaction<'_>,
    conversation_id: &str,
    run_id: &str,
) -> Result<(), HubStoreError> {
    let present = transaction
        .query_row(
            "SELECT 1 FROM runs WHERE id = ?1 AND conversation_id = ?2",
            params![run_id, conversation_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(read_error)?
        .is_some();
    if present {
        Ok(())
    } else {
        Err(not_found_conversation(conversation_id))
    }
}

fn latest_event_json(
    transaction: &Transaction<'_>,
    run_id: &str,
    sequence: u64,
    expected_bytes: usize,
) -> Result<String, HubStoreError> {
    event_json_at(transaction, run_id, sequence, expected_bytes)
}

fn event_json_at(
    transaction: &Transaction<'_>,
    run_id: &str,
    sequence: u64,
    expected_bytes: usize,
) -> Result<String, HubStoreError> {
    let sequence = i64::try_from(sequence).map_err(|_| corrupt("invalid Run event sequence"))?;
    let event = transaction
        .query_row(
            "SELECT event_json FROM run_events WHERE run_id = ?1 AND seq = ?2",
            params![run_id, sequence],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(read_error)?
        .ok_or_else(|| corrupt("Run page references a missing event row"))?;
    validate_event_size(&event)?;
    if event.len() != expected_bytes {
        return Err(corrupt(
            "Run event byte count disagrees with its indexed row",
        ));
    }
    Ok(event)
}

fn query_event_skeletons(
    transaction: &Transaction<'_>,
    run_id: &str,
    after_sequence: i64,
    limit: i64,
) -> Result<Vec<(i64, i64)>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT seq, length(CAST(event_json AS BLOB)) FROM run_events
             WHERE run_id = ?1 AND seq > ?2 ORDER BY seq ASC LIMIT ?3",
        )
        .map_err(read_error)?;
    let rows = statement
        .query_map(params![run_id, after_sequence, limit], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(read_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(read_error)
}

fn decode_event_envelope(
    event_json: &str,
    run_id: &str,
    conversation_id: &str,
    expected_sequence: u64,
) -> Result<(OwnedRunTimelineEventType, RuntimeEvent), HubStoreError> {
    validate_event_size(event_json)?;
    let event: RuntimeEvent = serde_json::from_str(event_json)
        .map_err(|_| corrupt("stored Run event envelope is invalid"))?;
    if event.v != PROTOCOL_VERSION
        || event.run_id != run_id
        || event.session_id != conversation_id
        || event.seq != expected_sequence
    {
        return Err(corrupt("stored Run event envelope disagrees with its row"));
    }
    let event_type = match &event.kind {
        RuntimeEventKind::RunStarted { .. } => OwnedRunTimelineEventType::RunStarted,
        RuntimeEventKind::TurnStarted { .. } => OwnedRunTimelineEventType::TurnStarted,
        RuntimeEventKind::RunFinished { .. } => OwnedRunTimelineEventType::RunFinished,
        RuntimeEventKind::AssistantDelta { .. }
        | RuntimeEventKind::MessageCommitted { .. }
        | RuntimeEventKind::ToolStarted { .. }
        | RuntimeEventKind::ToolFinished { .. }
        | RuntimeEventKind::ToolRejected { .. }
        | RuntimeEventKind::RuntimeError { .. } => OwnedRunTimelineEventType::Activity,
    };
    Ok((event_type, event))
}

fn decode_terminal_status(event: &RuntimeEvent) -> Result<OwnedRunStatus, HubStoreError> {
    match &event.kind {
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Completed { .. },
        } => Ok(OwnedRunStatus::Completed),
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Cancelled,
        } => Ok(OwnedRunStatus::Cancelled),
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::LimitExceeded { .. },
        } => Ok(OwnedRunStatus::LimitExceeded),
        RuntimeEventKind::RunFinished {
            outcome: RunOutcome::Failed { .. },
        } => Ok(OwnedRunStatus::Failed),
        _ => Err(corrupt("latest Run event is not terminal")),
    }
}

fn validate_run_page(limit: usize) -> Result<i64, HubStoreError> {
    if !(1..=MAX_OWNED_RUN_PAGE_LIMIT).contains(&limit) {
        return Err(invalid_page("invalid owner Run page limit"));
    }
    i64::try_from(limit + 1).map_err(|_| invalid_page("owner Run page limit is out of range"))
}

fn validate_timeline_page(limit: usize) -> Result<i64, HubStoreError> {
    if !(1..=MAX_OWNED_RUN_TIMELINE_PAGE_LIMIT).contains(&limit) {
        return Err(invalid_page("invalid owner Run timeline page limit"));
    }
    i64::try_from(limit + 1)
        .map_err(|_| invalid_page("owner Run timeline page limit is out of range"))
}

fn validate_cursor(
    before: Option<&OwnedRunCursor>,
) -> Result<(Option<i64>, Option<&str>), HubStoreError> {
    before.map_or(Ok((None, None)), |cursor| {
        validate_identifier(&cursor.run_id, "Run cursor")?;
        let timestamp = i64::try_from(cursor.created_at_ms)
            .map_err(|_| invalid_page("owner Run cursor timestamp is out of range"))?;
        Ok((Some(timestamp), Some(cursor.run_id.as_str())))
    })
}

fn validate_identifier(value: &str, entity: &str) -> Result<(), HubStoreError> {
    if value.is_empty() || value.trim() != value || value.len() > MAX_HUB_ENTITY_ID_BYTES {
        return Err(invalid_page(&format!("invalid {entity} identifier")));
    }
    Ok(())
}

fn decode_sequence(sequence: i64) -> Result<u64, HubStoreError> {
    u64::try_from(sequence).map_err(|_| corrupt("stored Run sequence is out of range"))
}

fn decode_timestamp(timestamp: i64, label: &str) -> Result<u64, HubStoreError> {
    u64::try_from(timestamp).map_err(|_| corrupt(&format!("stored {label} is out of range")))
}

fn validate_event_size(event_json: &str) -> Result<(), HubStoreError> {
    if event_json.len() > MAX_RUN_EVENT_JSON_BYTES {
        Err(corrupt("stored Run event exceeds its durable size limit"))
    } else {
        Ok(())
    }
}

fn decode_event_bytes(event_bytes: i64) -> Result<usize, HubStoreError> {
    let event_bytes = usize::try_from(event_bytes)
        .map_err(|_| corrupt("stored Run event byte count is out of range"))?;
    if event_bytes == 0 || event_bytes > MAX_RUN_EVENT_JSON_BYTES {
        return Err(corrupt("stored Run event exceeds its durable size limit"));
    }
    Ok(event_bytes)
}
