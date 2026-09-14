use crate::runtime_domain::{
    ConversationOwner, HubEntity, HubStoreError, MAX_HUB_ENTITY_ID_BYTES, PendingRunIntent,
    PendingRunIntentStatus, PendingRunIntentSubmissionResult, PendingRunIntentTimelineEvent,
    PendingRunIntentTimelineEventType,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

mod read;
mod replay;
use read::prompt_projection;
pub(super) use read::{owned_page, owned_timeline_page};

use super::{rows, write};

#[derive(Clone, Copy)]
pub(super) struct SubmitInput<'a> {
    pub owner: &'a ConversationOwner,
    pub conversation_id: &'a str,
    pub content: &'a str,
    pub idempotency_key: &'a str,
    pub expected_version: u64,
    pub profile_id: &'a str,
    pub profile_sha256: &'a [u8; 32],
}

struct SubmissionContext {
    project_id: String,
    submitted_at_ms: u64,
    submitted_at: i64,
    grant_id: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubmitStage {
    PromptRow,
    ConversationTimestamp,
    ChangeJournal,
    IntentRow,
    TimelineEvent,
}

pub(super) fn submit(
    connection: &mut Connection,
    input: SubmitInput<'_>,
) -> Result<PendingRunIntentSubmissionResult, HubStoreError> {
    submit_with_hooks(connection, input, rows::now_ms, |_| Ok(()))
}

fn submit_with_hooks(
    connection: &mut Connection,
    input: SubmitInput<'_>,
    clock: impl FnOnce() -> Result<u64, HubStoreError>,
    mut after_stage: impl FnMut(SubmitStage) -> Result<(), HubStoreError>,
) -> Result<PendingRunIntentSubmissionResult, HubStoreError> {
    validate_input(&input)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(super::read_error)?;

    // This lookup intentionally precedes current owner head, profile, and
    // consent checks: a replay is only a receipt for the original operation.
    if let Some(result) = replay::replay(&transaction, &input)? {
        transaction.commit().map_err(super::read_error)?;
        return Ok(result);
    }

    let context = prepare_submission(&transaction, &input, clock)?;
    let result = create_pending_intent(&transaction, &input, context, &mut after_stage)?;
    transaction.commit().map_err(super::read_error)?;
    Ok(result)
}

fn prepare_submission(
    transaction: &Transaction<'_>,
    input: &SubmitInput<'_>,
    clock: impl FnOnce() -> Result<u64, HubStoreError>,
) -> Result<SubmissionContext, HubStoreError> {
    let project_id = owned_project_scope(transaction, input.owner, input.conversation_id)?;
    let actual_version = conversation_change_version(transaction, input.conversation_id)?;
    if actual_version != input.expected_version {
        return Err(conflict(format!(
            "Conversation aggregate version changed: expected {}, found {actual_version}",
            input.expected_version
        )));
    }
    let submitted_at_ms = clock()?;
    let submitted_at = to_i64(submitted_at_ms)?;
    let grant_id = active_grant(
        transaction,
        input.owner,
        &project_id,
        input.profile_id,
        input.profile_sha256,
        submitted_at,
    )?;
    Ok(SubmissionContext {
        project_id,
        submitted_at_ms,
        submitted_at,
        grant_id,
    })
}

fn create_pending_intent(
    transaction: &Transaction<'_>,
    input: &SubmitInput<'_>,
    context: SubmissionContext,
    after_stage: &mut impl FnMut(SubmitStage) -> Result<(), HubStoreError>,
) -> Result<PendingRunIntentSubmissionResult, HubStoreError> {
    let intent_id = rows::new_id(transaction, "pending-intent")?;
    let prompt = create_intent_prompt(
        transaction,
        input,
        &intent_id,
        context.submitted_at_ms,
        after_stage,
    )?;
    let aggregate_version = conversation_change_version(transaction, input.conversation_id)?;
    insert_intent_row(
        transaction,
        input,
        &context,
        &intent_id,
        &prompt,
        aggregate_version,
    )?;
    after_stage(SubmitStage::IntentRow)?;
    let initial_event = PendingRunIntentTimelineEvent {
        event_id: rows::new_id(transaction, "pending-intent-event")?,
        seq: 1,
        emitted_at_ms: context.submitted_at_ms,
        event_type: PendingRunIntentTimelineEventType::Submitted,
    };
    insert_initial_event(transaction, &initial_event, &intent_id, input.owner)?;
    after_stage(SubmitStage::TimelineEvent)?;
    Ok(PendingRunIntentSubmissionResult {
        prompt: prompt_projection(prompt.clone()),
        intent: PendingRunIntent {
            intent_id,
            conversation_id: input.conversation_id.into(),
            prompt_id: prompt.id,
            project_id: context.project_id,
            profile_id: input.profile_id.into(),
            submitted_at_ms: context.submitted_at_ms,
            aggregate_version,
            latest_sequence: 1,
            status: PendingRunIntentStatus::Pending,
        },
        initial_event,
        replayed: false,
    })
}

fn create_intent_prompt(
    transaction: &Transaction<'_>,
    input: &SubmitInput<'_>,
    intent_id: &str,
    submitted_at_ms: u64,
    after_stage: &mut impl FnMut(SubmitStage) -> Result<(), HubStoreError>,
) -> Result<forge_runtime_domain::PromptRecord, HubStoreError> {
    let prompt = forge_runtime_domain::PromptRecord {
        id: rows::new_id(transaction, "prompt")?,
        conversation_id: input.conversation_id.into(),
        role: "user".into(),
        content: input.content.into(),
        // The intent ID makes this Prompt's store-level unique key independent
        // of caller key collisions across different owners.
        idempotency_key: format!("pending-run-intent-prompt:{intent_id}"),
        created_at_ms: submitted_at_ms,
    };
    write::insert_prompt_with_hook(transaction, &prompt, |stage| {
        after_stage(match stage {
            write::PromptInsertStage::PromptRow => SubmitStage::PromptRow,
            write::PromptInsertStage::ConversationTimestamp => SubmitStage::ConversationTimestamp,
            write::PromptInsertStage::ChangeJournal => SubmitStage::ChangeJournal,
        })
    })?;
    Ok(prompt)
}

fn insert_intent_row(
    transaction: &Transaction<'_>,
    input: &SubmitInput<'_>,
    context: &SubmissionContext,
    intent_id: &str,
    prompt: &forge_runtime_domain::PromptRecord,
    aggregate_version: u64,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO pending_run_intents(
               intent_id, issuer, subject, tenant_id, conversation_id, prompt_id,
               project_id, consent_grant_id, profile_id, profile_sha256,
               idempotency_key, aggregate_version, submitted_at_ms, status
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'pending')",
            params![
                intent_id,
                input.owner.issuer,
                input.owner.subject,
                input.owner.tenant_id,
                input.conversation_id,
                prompt.id,
                context.project_id,
                context.grant_id,
                input.profile_id,
                input.profile_sha256.as_slice(),
                input.idempotency_key,
                to_i64(aggregate_version)?,
                context.submitted_at,
            ],
        )
        .map_err(|error| super::write_error(HubEntity::PendingRunIntent, error))?;
    Ok(())
}

fn owned_project_scope(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    conversation_id: &str,
) -> Result<String, HubStoreError> {
    let scope = transaction
        .query_row(
            "SELECT c.scope_kind, c.scope_id
             FROM conversations AS c
             JOIN conversation_owners AS o ON o.conversation_id = c.id
             WHERE c.id = ?1 AND o.issuer = ?2 AND o.subject = ?3 AND o.tenant_id = ?4",
            params![
                conversation_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(super::read_error)?;
    let Some((scope_kind, scope_id)) = scope else {
        return Err(not_found_conversation(conversation_id));
    };
    if scope_kind != "project" {
        return Err(conflict(
            "pending Run intent requires a Project-scoped Conversation".into(),
        ));
    }
    scope_id.ok_or_else(|| HubStoreError::Corrupt {
        message: "Project-scoped Conversation has no Project ID".into(),
    })
}

fn active_grant(
    transaction: &Transaction<'_>,
    owner: &ConversationOwner,
    project_id: &str,
    profile_id: &str,
    profile_sha256: &[u8; 32],
    now_ms: i64,
) -> Result<String, HubStoreError> {
    let mut statement = transaction
        .prepare(
            "SELECT g.grant_id FROM project_execution_consent_grants AS g
             WHERE g.issuer = ?1 AND g.subject = ?2 AND g.tenant_id = ?3
               AND g.project_id = ?4 AND g.profile_id = ?5 AND g.profile_sha256 = ?6
               AND g.expires_at_ms > ?7
               AND NOT EXISTS(
                 SELECT 1 FROM project_execution_consent_events AS e
                 WHERE e.grant_id = g.grant_id AND e.event_sequence = 2
               )
             ORDER BY g.granted_at_ms DESC, g.grant_id DESC LIMIT 2",
        )
        .map_err(super::read_error)?;
    let rows = statement
        .query_map(
            params![
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                project_id,
                profile_id,
                profile_sha256.as_slice(),
                now_ms,
            ],
            |row| row.get::<_, String>(0),
        )
        .map_err(super::read_error)?;
    let grants = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(super::read_error)?;
    match grants.as_slice() {
        [grant_id] => Ok(grant_id.clone()),
        [] => Err(conflict(
            "no unexpired, unrevoked Project consent matches the server execution profile".into(),
        )),
        _ => Err(HubStoreError::Corrupt {
            message: "multiple active consent grants match one Project profile".into(),
        }),
    }
}

fn conversation_change_version(
    transaction: &Transaction<'_>,
    conversation_id: &str,
) -> Result<u64, HubStoreError> {
    let version: Option<i64> = transaction
        .query_row(
            "SELECT last_version FROM conversation_change_heads WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(super::read_error)?;
    version
        .map(u64::try_from)
        .transpose()
        .map_err(|error| HubStoreError::Corrupt {
            message: format!("invalid Conversation aggregate version: {error}"),
        })?
        .ok_or_else(|| HubStoreError::Corrupt {
            message: format!("owned Conversation '{conversation_id}' has no change head"),
        })
}

fn insert_initial_event(
    transaction: &Transaction<'_>,
    event: &PendingRunIntentTimelineEvent,
    intent_id: &str,
    owner: &ConversationOwner,
) -> Result<(), HubStoreError> {
    transaction
        .execute(
            "INSERT INTO pending_run_intent_events(
               event_id, intent_id, event_sequence, event_kind, issuer, subject,
               tenant_id, occurred_at_ms
             ) VALUES(?1, ?2, 1, 'submitted', ?3, ?4, ?5, ?6)",
            params![
                event.event_id,
                intent_id,
                owner.issuer,
                owner.subject,
                owner.tenant_id,
                to_i64(event.emitted_at_ms)?,
            ],
        )
        .map_err(|error| super::write_error(HubEntity::PendingRunIntent, error))?;
    Ok(())
}

fn validate_input(input: &SubmitInput<'_>) -> Result<(), HubStoreError> {
    for (value, maximum, label) in [
        (
            input.conversation_id,
            MAX_HUB_ENTITY_ID_BYTES,
            "Conversation",
        ),
        (
            input.profile_id,
            MAX_HUB_ENTITY_ID_BYTES,
            "execution profile",
        ),
        (input.idempotency_key, 128, "idempotency key"),
    ] {
        if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
            return Err(conflict(format!("{label} identifier is invalid")));
        }
    }
    if input.content.trim().is_empty()
        || input.content.len() > forge_runtime_domain::MAX_PROMPT_CONTENT_BYTES
    {
        return Err(conflict("Prompt content is invalid".into()));
    }
    for (value, maximum) in [
        (&input.owner.issuer, 2048),
        (&input.owner.subject, 255),
        (&input.owner.tenant_id, 256),
    ] {
        if value.trim().is_empty() || value.len() > maximum || value.chars().any(char::is_control) {
            return Err(conflict("verified owner tuple is invalid".into()));
        }
    }
    to_i64(input.expected_version)?;
    Ok(())
}

fn checked_timestamp(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = row.get(index)?;
    u64::try_from(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Integer,
            Box::new(error),
        )
    })
}

fn decode_timestamp(value: i64) -> Result<u64, HubStoreError> {
    u64::try_from(value).map_err(|error| HubStoreError::Corrupt {
        message: format!("invalid pending Run intent timestamp/version: {error}"),
    })
}

fn to_i64(value: u64) -> Result<i64, HubStoreError> {
    i64::try_from(value).map_err(|error| conflict(format!("value exceeds SQLite range: {error}")))
}

fn conflict(message: String) -> HubStoreError {
    HubStoreError::Conflict {
        entity: HubEntity::PendingRunIntent,
        message,
    }
}

fn not_found_conversation(conversation_id: &str) -> HubStoreError {
    HubStoreError::NotFound {
        entity: HubEntity::Conversation,
        id: conversation_id.into(),
    }
}

#[cfg(test)]
#[path = "tests/pending_run_intent_atomicity.rs"]
mod atomicity_tests;
