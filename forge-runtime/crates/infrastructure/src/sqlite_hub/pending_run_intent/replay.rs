use crate::runtime_domain::{
    HubStoreError, PendingRunIntent, PendingRunIntentStatus, PendingRunIntentSubmissionResult,
    PendingRunIntentTimelineEvent,
};
use rusqlite::{OptionalExtension, Transaction, params};

use super::{SubmitInput, conflict, decode_timestamp, read, rows};

struct ReplayIntentRow {
    intent_id: String,
    conversation_id: String,
    prompt_id: String,
    project_id: String,
    aggregate_version: i64,
    submitted_at_ms: i64,
    profile_id: String,
    status: String,
}

pub(super) fn replay(
    transaction: &Transaction<'_>,
    input: &SubmitInput<'_>,
) -> Result<Option<PendingRunIntentSubmissionResult>, HubStoreError> {
    let Some(existing) = find_replay_intent(transaction, input)? else {
        return Ok(None);
    };
    let prompt = replay_prompt(transaction, input, &existing)?;
    let event = read::load_initial_event(transaction, &existing.intent_id)?;
    Ok(Some(replay_result(existing, prompt, event)?))
}

fn find_replay_intent(
    transaction: &Transaction<'_>,
    input: &SubmitInput<'_>,
) -> Result<Option<ReplayIntentRow>, HubStoreError> {
    transaction
        .query_row(
            "SELECT i.intent_id, i.conversation_id, i.prompt_id, i.project_id,
                    i.aggregate_version, i.submitted_at_ms, i.profile_id, i.status
             FROM pending_run_intents AS i
             WHERE i.issuer = ?1 AND i.subject = ?2 AND i.tenant_id = ?3
               AND i.idempotency_key = ?4",
            params![
                input.owner.issuer,
                input.owner.subject,
                input.owner.tenant_id,
                input.idempotency_key,
            ],
            |row| {
                Ok(ReplayIntentRow {
                    intent_id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    prompt_id: row.get(2)?,
                    project_id: row.get(3)?,
                    aggregate_version: row.get(4)?,
                    submitted_at_ms: row.get(5)?,
                    profile_id: row.get(6)?,
                    status: row.get(7)?,
                })
            },
        )
        .optional()
        .map_err(super::super::read_error)
}

fn replay_prompt(
    transaction: &Transaction<'_>,
    input: &SubmitInput<'_>,
    existing: &ReplayIntentRow,
) -> Result<forge_runtime_domain::PromptRecord, HubStoreError> {
    if existing.conversation_id != input.conversation_id {
        return Err(conflict(
            "pending Run intent idempotency key was reused for another Conversation".into(),
        ));
    }
    let stored_prompt = transaction
        .query_row(
            "SELECT id, conversation_id, role, content, idempotency_key, created_at_ms
             FROM prompts WHERE id = ?1 AND conversation_id = ?2",
            params![existing.prompt_id, existing.conversation_id],
            rows::prompt,
        )
        .optional()
        .map_err(super::super::read_error)?
        .ok_or_else(|| HubStoreError::Corrupt {
            message: "pending Run intent references a missing Prompt".into(),
        })?;
    if stored_prompt.role != "user" || stored_prompt.content != input.content {
        return Err(conflict(
            "pending Run intent idempotency key was reused with different Prompt content".into(),
        ));
    }
    if existing.status != "pending" {
        return Err(HubStoreError::Corrupt {
            message: "stored pending Run intent has an unknown status".into(),
        });
    }
    Ok(stored_prompt)
}

fn replay_result(
    existing: ReplayIntentRow,
    stored_prompt: forge_runtime_domain::PromptRecord,
    event: PendingRunIntentTimelineEvent,
) -> Result<PendingRunIntentSubmissionResult, HubStoreError> {
    Ok(PendingRunIntentSubmissionResult {
        prompt: read::prompt_projection(stored_prompt),
        intent: PendingRunIntent {
            intent_id: existing.intent_id,
            conversation_id: existing.conversation_id,
            prompt_id: existing.prompt_id,
            project_id: existing.project_id,
            profile_id: existing.profile_id,
            submitted_at_ms: decode_timestamp(existing.submitted_at_ms)?,
            aggregate_version: decode_timestamp(existing.aggregate_version)?,
            latest_sequence: 1,
            status: PendingRunIntentStatus::Pending,
        },
        initial_event: event,
        replayed: true,
    })
}
