use crate::runtime_domain::{
    ConversationOwner, ConversationPrompt, HubEntity, HubStoreError, PendingRunIntent,
    PendingRunIntentCursor, PendingRunIntentPage, PendingRunIntentStatus,
    PendingRunIntentTimelineEvent, PendingRunIntentTimelineEventType, PendingRunIntentTimelinePage,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{checked_timestamp, not_found_conversation};

pub(super) fn load_initial_event(
    transaction: &Transaction<'_>,
    intent_id: &str,
) -> Result<PendingRunIntentTimelineEvent, HubStoreError> {
    transaction
        .query_row(
            "SELECT event_id, event_sequence, event_kind, occurred_at_ms
             FROM pending_run_intent_events WHERE intent_id = ?1 AND event_sequence = 1",
            [intent_id],
            decode_event_row,
        )
        .optional()
        .map_err(super::super::read_error)?
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "pending Run intent is missing its initial timeline event".into(),
        })
}

#[allow(clippy::too_many_lines)] // Query, trim, and cursor are one bounded projection contract.
pub(in crate::sqlite_hub) fn owned_page(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
    before: Option<&PendingRunIntentCursor>,
    limit: usize,
) -> Result<PendingRunIntentPage, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(super::super::read_error)?;
    ensure_owned_conversation(&transaction, owner, conversation_id)?;
    let mut intents = query_owned_intents(&transaction, owner, conversation_id, before, limit)?;
    let has_more = intents.len() > limit;
    intents.truncate(limit);
    let next_cursor = page_cursor(&intents, has_more);
    transaction.commit().map_err(super::super::read_error)?;
    Ok(PendingRunIntentPage {
        conversation_id: conversation_id.into(),
        intents,
        next_cursor,
        has_more,
    })
}

fn query_owned_intents(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
    before: Option<&PendingRunIntentCursor>,
    limit: usize,
) -> Result<Vec<PendingRunIntent>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT intent_id, conversation_id, prompt_id, project_id, profile_id,
                    submitted_at_ms, aggregate_version, status
             FROM pending_run_intents
             WHERE issuer = ?1 AND subject = ?2 AND tenant_id = ?3 AND conversation_id = ?4
               AND (?5 IS NULL OR submitted_at_ms < ?5
                    OR (submitted_at_ms = ?5 AND intent_id < ?6))
             ORDER BY submitted_at_ms DESC, intent_id DESC LIMIT ?7",
        )
        .map_err(super::super::read_error)?;
    let limit_i64 = i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX);
    let cursor_time =
        before.map(|cursor| i64::try_from(cursor.submitted_at_ms).unwrap_or(i64::MAX));
    let cursor_id = before.map_or("", |cursor| cursor.intent_id.as_str());
    let mapped = statement
        .query_map(
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                conversation_id,
                cursor_time,
                cursor_id,
                limit_i64,
            ],
            decode_intent_row,
        )
        .map_err(super::super::read_error)?;
    let intents = mapped
        .collect::<Result<Vec<_>, _>>()
        .map_err(super::super::read_error)?;
    drop(statement);
    Ok(intents)
}

fn page_cursor(intents: &[PendingRunIntent], has_more: bool) -> Option<PendingRunIntentCursor> {
    has_more
        .then(|| {
            intents.last().map(|intent| PendingRunIntentCursor {
                submitted_at_ms: intent.submitted_at_ms,
                intent_id: intent.intent_id.clone(),
            })
        })
        .flatten()
}

#[allow(clippy::too_many_lines)] // Event validation and paging share one read snapshot.
pub(in crate::sqlite_hub) fn owned_timeline_page(
    connection: &mut Connection,
    owner: &ConversationOwner,
    conversation_id: &str,
    intent_id: &str,
    after_sequence: u64,
    limit: usize,
) -> Result<PendingRunIntentTimelinePage, HubStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(super::super::read_error)?;
    ensure_owned_intent(&transaction, owner, conversation_id, intent_id)?;
    // Validate the immutable initial envelope even when the cursor is past it.
    validate_initial_event(&transaction, intent_id)?;
    let mut events = query_timeline_events(&transaction, owner, intent_id, after_sequence, limit)?;
    let has_more = events.len() > limit;
    events.truncate(limit);
    let scanned_through_sequence = events
        .last()
        .map_or(after_sequence.max(1), |event| event.seq);
    transaction.commit().map_err(super::super::read_error)?;
    Ok(PendingRunIntentTimelinePage {
        conversation_id: conversation_id.into(),
        intent_id: intent_id.into(),
        after_sequence,
        scanned_through_sequence,
        has_more,
        events,
    })
}

fn validate_initial_event(
    transaction: &Transaction<'_>,
    intent_id: &str,
) -> Result<(), HubStoreError> {
    let initial = load_initial_event(transaction, intent_id)?;
    if initial.event_type == PendingRunIntentTimelineEventType::Submitted && initial.seq == 1 {
        return Ok(());
    }
    Err(HubStoreError::Corrupt {
        message: "pending Run intent has an invalid initial timeline event".into(),
    })
}

fn query_timeline_events(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    intent_id: &str,
    after_sequence: u64,
    limit: usize,
) -> Result<Vec<PendingRunIntentTimelineEvent>, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT event_id, event_sequence, event_kind, occurred_at_ms
             FROM pending_run_intent_events
             WHERE intent_id = ?1 AND issuer = ?2 AND subject = ?3 AND tenant_id = ?4
               AND event_sequence > ?5
             ORDER BY event_sequence ASC LIMIT ?6",
        )
        .map_err(super::super::read_error)?;
    let limit_i64 = i64::try_from(limit.saturating_add(1)).unwrap_or(i64::MAX);
    let mapped = statement
        .query_map(
            params![
                intent_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                i64::try_from(after_sequence).unwrap_or(i64::MAX),
                limit_i64,
            ],
            decode_event_row,
        )
        .map_err(super::super::read_error)?;
    let events = mapped
        .collect::<Result<Vec<_>, _>>()
        .map_err(super::super::read_error)?;
    drop(statement);
    Ok(events)
}

fn ensure_owned_conversation(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
) -> Result<(), HubStoreError> {
    let exists: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM conversation_owners
               WHERE conversation_id = ?1 AND issuer = ?2 AND subject = ?3 AND tenant_id = ?4
             )",
            params![
                conversation_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id
            ],
            |row| row.get(0),
        )
        .map_err(super::super::read_error)?;
    if exists {
        Ok(())
    } else {
        Err(not_found_conversation(conversation_id))
    }
}

fn ensure_owned_intent(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
    intent_id: &str,
) -> Result<(), HubStoreError> {
    let exists: bool = transaction
        .query_row(
            "SELECT EXISTS(
               SELECT 1 FROM pending_run_intents
               WHERE intent_id = ?1 AND conversation_id = ?2
                 AND issuer = ?3 AND subject = ?4 AND tenant_id = ?5
             )",
            params![
                intent_id,
                conversation_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id,
            ],
            |row| row.get(0),
        )
        .map_err(super::super::read_error)?;
    if exists {
        Ok(())
    } else {
        Err(HubStoreError::NotFound {
            entity: HubEntity::PendingRunIntent,
            id: intent_id.into(),
        })
    }
}

fn decode_intent_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<PendingRunIntent> {
    let status: String = row.get(7)?;
    if status != "pending" {
        return Err(rusqlite::Error::InvalidColumnType(
            7,
            "status".into(),
            rusqlite::types::Type::Text,
        ));
    }
    Ok(PendingRunIntent {
        intent_id: row.get(0)?,
        conversation_id: row.get(1)?,
        prompt_id: row.get(2)?,
        project_id: row.get(3)?,
        profile_id: row.get(4)?,
        submitted_at_ms: checked_timestamp(row, 5)?,
        aggregate_version: checked_timestamp(row, 6)?,
        latest_sequence: 1,
        status: PendingRunIntentStatus::Pending,
    })
}

pub(super) fn decode_event_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<PendingRunIntentTimelineEvent> {
    let event_sequence: i64 = row.get(1)?;
    let event_kind: String = row.get(2)?;
    if event_sequence != 1 || event_kind != "submitted" {
        return Err(rusqlite::Error::InvalidColumnType(
            2,
            "event_kind".into(),
            rusqlite::types::Type::Text,
        ));
    }
    Ok(PendingRunIntentTimelineEvent {
        event_id: row.get(0)?,
        seq: 1,
        emitted_at_ms: checked_timestamp(row, 3)?,
        event_type: PendingRunIntentTimelineEventType::Submitted,
    })
}

pub(super) fn prompt_projection(prompt: forge_runtime_domain::PromptRecord) -> ConversationPrompt {
    ConversationPrompt {
        id: prompt.id,
        conversation_id: prompt.conversation_id,
        role: prompt.role,
        content: prompt.content,
        created_at_ms: prompt.created_at_ms,
    }
}
