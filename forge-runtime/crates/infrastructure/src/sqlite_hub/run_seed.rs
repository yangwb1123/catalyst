use crate::runtime_domain::{
    BeginRun, BeginRunDisposition, BeginRunResult, BeginRunWithPrompt, HubEntity, HubStoreError,
    Message, PROTOCOL_VERSION, PromptRecord, RunEntity, RunStoreError, RuntimeEvent,
    RuntimeEventKind,
};
use rusqlite::{Connection, OptionalExtension, Transaction};

use super::{
    rows,
    run_read::{
        StoredRun, find_run_by_key, last_event_sequence, load_event_at, validate_stored_run,
    },
    run_write::{append_event_locked, begin_run_locked, immediate, validate_begin_request},
    write,
};

const PROMPT_COLUMNS: &str = "id, conversation_id, role, content, idempotency_key, created_at_ms";

pub(super) fn begin(
    connection: &mut Connection,
    request: &BeginRunWithPrompt,
) -> Result<BeginRunResult, RunStoreError> {
    validate_request(request)?;
    let transaction = immediate(connection)?;
    let result = begin_locked(&transaction, request)?;
    transaction.commit().map_err(unavailable)?;
    Ok(result)
}

fn begin_locked(
    transaction: &Transaction<'_>,
    request: &BeginRunWithPrompt,
) -> Result<BeginRunResult, RunStoreError> {
    let result = if let Some(existing) = find_run_by_key(transaction, &request.run.idempotency_key)?
    {
        replay_existing(transaction, request, existing)?
    } else {
        let prompt = match prompt_by_key(transaction, &request.prompt_idempotency_key)? {
            Some(existing) => {
                ensure_prompt_input(&existing, request)?;
                existing
            }
            None => insert_new_prompt(transaction, request)?,
        };
        let mut run = request.run.clone();
        run.prompt_id.clone_from(&prompt.id);
        begin_run_locked(transaction, &run)?
    };
    ensure_seed_prefix(transaction, &result)?;
    Ok(result)
}

fn replay_existing(
    transaction: &Transaction<'_>,
    request: &BeginRunWithPrompt,
    existing: StoredRun,
) -> Result<BeginRunResult, RunStoreError> {
    validate_stored_run(&existing)?;
    let prompt = prompt_by_id(transaction, &existing.record.prompt_id)?
        .ok_or_else(|| corrupt("Run's bound Prompt is missing during atomic seed replay"))?;
    validate_stored_prompt(&prompt, &existing)?;
    if existing.record.conversation_id != request.run.conversation_id
        || existing.record.project_id != request.run.project_id
        || existing.record.execution != request.run.execution
        || prompt.content != request.prompt_content
        || prompt.idempotency_key != request.prompt_idempotency_key
    {
        return Err(conflict(
            RunEntity::Run,
            "idempotency key was reused with different Prompt or Run input",
        ));
    }
    let normalized = BeginRun {
        v: request.run.v,
        run_id: existing.record.run_id,
        conversation_id: existing.record.conversation_id,
        prompt_id: existing.record.prompt_id,
        project_id: existing.record.project_id,
        execution: existing.record.execution,
        idempotency_key: existing.idempotency_key,
        created_at_ms: existing.record.created_at_ms,
    };
    begin_run_locked(transaction, &normalized)
}

fn ensure_seed_prefix(
    transaction: &Transaction<'_>,
    result: &BeginRunResult,
) -> Result<(), RunStoreError> {
    let tail = last_event_sequence(transaction, &result.run.run_id)?;
    match result.disposition {
        BeginRunDisposition::Created if tail == 0 => {
            append_event_locked(transaction, &initial_event(result, 1))?;
            append_event_locked(transaction, &initial_event(result, 2))?;
        }
        BeginRunDisposition::Created => {
            return Err(corrupt(
                "new atomic Run unexpectedly contained durable journal events",
            ));
        }
        BeginRunDisposition::Replayed if tail < 2 => {
            return Err(corrupt(
                "replayed atomic Run is missing its complete durable seed prefix",
            ));
        }
        BeginRunDisposition::Replayed => {
            for sequence in 1..=2 {
                let event = seed_event(transaction, result, sequence)?;
                validate_seed_event(&event, result, sequence)?;
            }
        }
    }
    Ok(())
}

fn seed_event(
    transaction: &Transaction<'_>,
    result: &BeginRunResult,
    sequence: u64,
) -> Result<RuntimeEvent, RunStoreError> {
    load_event_at(transaction, &result.run.run_id, sequence)?
        .ok_or_else(|| corrupt("Run seed prefix references a missing durable event"))
}

fn initial_event(result: &BeginRunResult, sequence: u64) -> RuntimeEvent {
    let kind = match sequence {
        1 => RuntimeEventKind::RunStarted {
            prompt: result.prompt.content.clone(),
        },
        2 => RuntimeEventKind::MessageCommitted {
            message: Message::User {
                text: result.prompt.content.clone(),
            },
        },
        _ => unreachable!("atomic Run seed only has two initial events"),
    };
    RuntimeEvent {
        v: PROTOCOL_VERSION,
        session_id: result.run.conversation_id.clone(),
        run_id: result.run.run_id.clone(),
        seq: sequence,
        emitted_at_ms: result.run.created_at_ms,
        kind,
    }
}

fn validate_seed_event(
    event: &RuntimeEvent,
    result: &BeginRunResult,
    sequence: u64,
) -> Result<(), RunStoreError> {
    let expected = initial_event(result, sequence);
    if event == &expected {
        return Ok(());
    }
    Err(corrupt(
        "Run's durable journal does not begin with its bound Prompt seed",
    ))
}

fn insert_new_prompt(
    transaction: &Transaction<'_>,
    request: &BeginRunWithPrompt,
) -> Result<PromptRecord, RunStoreError> {
    let prompt = PromptRecord {
        id: request.run.prompt_id.clone(),
        conversation_id: request.run.conversation_id.clone(),
        role: "user".into(),
        content: request.prompt_content.clone(),
        idempotency_key: request.prompt_idempotency_key.clone(),
        created_at_ms: request.run.created_at_ms,
    };
    write::insert_prompt(transaction, &prompt).map_err(from_hub)?;
    Ok(prompt)
}

fn prompt_by_key(
    transaction: &Transaction<'_>,
    key: &str,
) -> Result<Option<PromptRecord>, RunStoreError> {
    prompt_query(transaction, "idempotency_key", key)
}

fn prompt_by_id(
    transaction: &Transaction<'_>,
    id: &str,
) -> Result<Option<PromptRecord>, RunStoreError> {
    prompt_query(transaction, "id", id)
}

fn prompt_query(
    transaction: &Transaction<'_>,
    column: &str,
    value: &str,
) -> Result<Option<PromptRecord>, RunStoreError> {
    let predicate = match column {
        "id" => "id = ?1",
        "idempotency_key" => "idempotency_key = ?1",
        _ => return Err(corrupt("unsupported Prompt seed lookup")),
    };
    transaction
        .query_row(
            &format!("SELECT {PROMPT_COLUMNS} FROM prompts WHERE {predicate}"),
            [value],
            rows::prompt,
        )
        .optional()
        .map_err(read_error)
}

fn validate_request(request: &BeginRunWithPrompt) -> Result<(), RunStoreError> {
    validate_begin_request(&request.run)?;
    if request.prompt_content.trim().is_empty() {
        return Err(conflict(
            RunEntity::Prompt,
            "Prompt content cannot be empty",
        ));
    }
    if request.prompt_idempotency_key.trim().is_empty() {
        return Err(conflict(
            RunEntity::Prompt,
            "Prompt idempotency key cannot be empty",
        ));
    }
    Ok(())
}

fn ensure_prompt_input(
    prompt: &PromptRecord,
    request: &BeginRunWithPrompt,
) -> Result<(), RunStoreError> {
    if prompt.conversation_id == request.run.conversation_id
        && prompt.role == "user"
        && prompt.content == request.prompt_content
    {
        return Ok(());
    }
    Err(conflict(
        RunEntity::Prompt,
        "idempotency key was reused with different Prompt input",
    ))
}

fn validate_stored_prompt(
    prompt: &PromptRecord,
    existing: &StoredRun,
) -> Result<(), RunStoreError> {
    if prompt.conversation_id == existing.record.conversation_id && prompt.role == "user" {
        return Ok(());
    }
    Err(corrupt(
        "Run's bound Prompt has invalid conversation or role during atomic seed replay",
    ))
}

fn from_hub(error: HubStoreError) -> RunStoreError {
    match error {
        HubStoreError::NotFound { entity, id } => match mapped_entity(entity) {
            Some(entity) => RunStoreError::NotFound { entity, id },
            None => RunStoreError::Unavailable {
                message: format!("unmapped Hub entity was not found: {id}"),
            },
        },
        HubStoreError::Conflict { entity, message } => match mapped_entity(entity) {
            Some(entity) => RunStoreError::Conflict { entity, message },
            None => RunStoreError::Unavailable { message },
        },
        HubStoreError::Unavailable { message } => RunStoreError::Unavailable { message },
        HubStoreError::Corrupt { message } => RunStoreError::Corrupt { message },
    }
}

const fn mapped_entity(entity: HubEntity) -> Option<RunEntity> {
    match entity {
        HubEntity::Project => Some(RunEntity::Project),
        HubEntity::Conversation => Some(RunEntity::Conversation),
        HubEntity::Prompt => Some(RunEntity::Prompt),
        _ => None,
    }
}

fn read_error(error: rusqlite::Error) -> RunStoreError {
    match error {
        rusqlite::Error::FromSqlConversionFailure(..)
        | rusqlite::Error::InvalidColumnType(..)
        | rusqlite::Error::IntegralValueOutOfRange(..) => corrupt(&error.to_string()),
        _ => unavailable(error),
    }
}

fn conflict(entity: RunEntity, message: &str) -> RunStoreError {
    RunStoreError::Conflict {
        entity,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> RunStoreError {
    RunStoreError::Corrupt {
        message: message.into(),
    }
}

fn unavailable(error: impl std::fmt::Display) -> RunStoreError {
    RunStoreError::Unavailable {
        message: error.to_string(),
    }
}
