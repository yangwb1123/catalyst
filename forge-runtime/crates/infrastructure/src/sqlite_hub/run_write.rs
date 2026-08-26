use crate::runtime_domain::{
    BeginRun, BeginRunDisposition, BeginRunResult, BoundRunPrompt, MAX_RUN_CURSOR_JSON_BYTES,
    MAX_RUN_EXECUTION_JSON_BYTES, PROTOCOL_VERSION, RUN_STORE_VERSION, RunEntity, RunJournalCursor,
    RunRecord, RunStoreError, RuntimeEvent,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{
    run_integrity::validate_candidate_capacity,
    run_read::{
        StoredRun, find_run, find_run_by_key, last_event_sequence, load_cursor, load_event_at,
        validate_stored_run,
    },
};

pub(super) fn begin_run(
    connection: &mut Connection,
    request: &BeginRun,
) -> Result<BeginRunResult, RunStoreError> {
    validate_begin_request(request)?;
    let transaction = immediate(connection)?;
    let record = begin_run_locked(&transaction, request)?;
    transaction.commit().map_err(unavailable)?;
    Ok(record)
}

pub(super) fn append_event(
    connection: &mut Connection,
    event: &RuntimeEvent,
) -> Result<(), RunStoreError> {
    let transaction = immediate(connection)?;
    append_event_locked(&transaction, event)?;
    transaction.commit().map_err(unavailable)
}

pub(super) fn begin_run_locked(
    transaction: &Transaction<'_>,
    request: &BeginRun,
) -> Result<BeginRunResult, RunStoreError> {
    if let Some(existing) = find_run_by_key(transaction, &request.idempotency_key)? {
        validate_stored_run(&existing)?;
        ensure_idempotent_begin(&existing, request)?;
        verify_project_conversation(transaction, request)?;
        let prompt = load_user_prompt(transaction, request)?;
        return Ok(begin_result(
            BeginRunDisposition::Replayed,
            existing.record,
            prompt,
        ));
    }
    if find_run(transaction, &request.run_id)?.is_some() {
        return Err(conflict(
            RunEntity::Run,
            "Run ID already belongs to another idempotency key",
        ));
    }
    verify_project_conversation(transaction, request)?;
    let prompt = load_user_prompt(transaction, request)?;
    let record = record_from(request);
    insert_run(transaction, &record, &request.idempotency_key)?;
    Ok(begin_result(BeginRunDisposition::Created, record, prompt))
}

pub(super) fn append_event_locked(
    transaction: &Transaction<'_>,
    event: &RuntimeEvent,
) -> Result<(), RunStoreError> {
    let stored = find_run(transaction, &event.run_id)?
        .ok_or_else(|| not_found(RunEntity::Run, &event.run_id))?;
    let mut journal = load_cursor(transaction, &stored.record)?;
    let expected = journal.cursor.next_sequence();
    let tail = last_event_sequence(transaction, &event.run_id)?;
    if tail != expected.saturating_sub(1) {
        return Err(corrupt("Run cursor does not match its durable event tail"));
    }
    if event.seq < expected {
        let committed = load_event_at(transaction, &event.run_id, event.seq)?
            .ok_or_else(|| corrupt("Run cursor references a missing durable event"))?;
        return ensure_exact_event_replay(&committed, event);
    }
    if event.seq != expected {
        return Err(conflict(
            RunEntity::Event,
            "event sequence is not contiguous",
        ));
    }
    let encoded = encode_event(event)?;
    let event_count = usize::try_from(expected.saturating_sub(1))
        .map_err(|error| corrupt(&format!("invalid Run event count: {error}")))?;
    validate_candidate_capacity(event_count, journal.total_bytes, encoded.len())?;
    verify_bound_start_prompt(transaction, &stored.record, event)?;
    journal.cursor.append(event).map_err(|error| {
        conflict(
            RunEntity::Event,
            &format!("event violates Run journal contract: {}", error.message),
        )
    })?;
    journal.total_bytes = journal
        .total_bytes
        .checked_add(encoded.len())
        .ok_or_else(|| conflict(RunEntity::Event, "Run journal byte count overflowed"))?;
    let cursor_json = encode_cursor(&journal.cursor)?;
    insert_event(transaction, event, &encoded)?;
    update_journal(
        transaction,
        &event.run_id,
        &cursor_json,
        journal.total_bytes,
    )
}

pub(super) fn validate_begin_request(request: &BeginRun) -> Result<(), RunStoreError> {
    if request.v != RUN_STORE_VERSION {
        return Err(conflict(
            RunEntity::Run,
            "unsupported Run begin request version",
        ));
    }
    for (entity, value) in [
        (RunEntity::Run, request.run_id.as_str()),
        (RunEntity::Conversation, request.conversation_id.as_str()),
        (RunEntity::Prompt, request.prompt_id.as_str()),
        (RunEntity::Project, request.project_id.as_str()),
        (RunEntity::Run, request.idempotency_key.as_str()),
    ] {
        if value.trim().is_empty() {
            return Err(conflict(entity, "identifier cannot be empty"));
        }
    }
    encode_execution(&request.execution)?;
    Ok(())
}

fn verify_project_conversation(
    transaction: &Transaction<'_>,
    request: &BeginRun,
) -> Result<(), RunStoreError> {
    ensure_exists(
        transaction,
        "projects",
        &request.project_id,
        RunEntity::Project,
    )?;
    let scope = transaction
        .query_row(
            "SELECT scope_kind, scope_id FROM conversations WHERE id = ?1",
            [&request.conversation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()
        .map_err(unavailable)?
        .ok_or_else(|| not_found(RunEntity::Conversation, &request.conversation_id))?;
    if scope != ("project".into(), Some(request.project_id.clone())) {
        return Err(conflict(
            RunEntity::Conversation,
            "Run requires a Conversation in the selected Project",
        ));
    }
    Ok(())
}

fn load_user_prompt(
    transaction: &Transaction<'_>,
    request: &BeginRun,
) -> Result<BoundRunPrompt, RunStoreError> {
    let prompt = transaction
        .query_row(
            "SELECT conversation_id, role, content, created_at_ms FROM prompts WHERE id = ?1",
            [&request.prompt_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(unavailable)?
        .ok_or_else(|| not_found(RunEntity::Prompt, &request.prompt_id))?;
    if prompt.0 != request.conversation_id || prompt.1 != "user" {
        return Err(conflict(
            RunEntity::Prompt,
            "Run Prompt must be a user Prompt in the selected Conversation",
        ));
    }
    let created_at_ms = u64::try_from(prompt.3).map_err(|error| RunStoreError::Corrupt {
        message: format!("invalid Prompt creation time: {error}"),
    })?;
    Ok(BoundRunPrompt {
        v: RUN_STORE_VERSION,
        prompt_id: request.prompt_id.clone(),
        conversation_id: prompt.0,
        content: prompt.2,
        created_at_ms,
    })
}

fn verify_bound_start_prompt(
    transaction: &Transaction<'_>,
    record: &RunRecord,
    event: &RuntimeEvent,
) -> Result<(), RunStoreError> {
    if event.seq != 1 {
        return Ok(());
    }
    let forge_runtime_domain::RuntimeEventKind::RunStarted { prompt } = &event.kind else {
        return Ok(());
    };
    let bound: String = transaction
        .query_row(
            "SELECT content FROM prompts WHERE id = ?1 AND conversation_id = ?2",
            params![record.prompt_id, record.conversation_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(unavailable)?
        .ok_or_else(|| RunStoreError::Corrupt {
            message: "Run's bound Prompt is missing or belongs to another Conversation".into(),
        })?;
    if bound == *prompt {
        Ok(())
    } else {
        Err(conflict(
            RunEntity::Event,
            "run_started Prompt does not match the Run's bound Prompt",
        ))
    }
}

fn ensure_exists(
    transaction: &Transaction<'_>,
    table: &str,
    id: &str,
    entity: RunEntity,
) -> Result<(), RunStoreError> {
    let sql = match table {
        "projects" => "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1)",
        _ => return Err(conflict(entity, "unsupported existence check")),
    };
    let exists: bool = transaction
        .query_row(sql, [id], |row| row.get(0))
        .map_err(unavailable)?;
    exists.then_some(()).ok_or_else(|| not_found(entity, id))
}

fn ensure_idempotent_begin(existing: &StoredRun, request: &BeginRun) -> Result<(), RunStoreError> {
    let record = &existing.record;
    if record.conversation_id == request.conversation_id
        && record.prompt_id == request.prompt_id
        && record.project_id == request.project_id
        && record.execution == request.execution
        && existing.idempotency_key == request.idempotency_key
    {
        return Ok(());
    }
    Err(conflict(
        RunEntity::Run,
        "idempotency key was reused with different Run input",
    ))
}

fn ensure_exact_event_replay(
    committed: &RuntimeEvent,
    candidate: &RuntimeEvent,
) -> Result<(), RunStoreError> {
    if committed == candidate {
        Ok(())
    } else {
        Err(conflict(
            RunEntity::Event,
            "event sequence was replayed with different content",
        ))
    }
}

fn record_from(request: &BeginRun) -> RunRecord {
    RunRecord {
        v: RUN_STORE_VERSION,
        run_id: request.run_id.clone(),
        conversation_id: request.conversation_id.clone(),
        prompt_id: request.prompt_id.clone(),
        project_id: request.project_id.clone(),
        execution: request.execution.clone(),
        protocol_version: PROTOCOL_VERSION,
        created_at_ms: request.created_at_ms,
    }
}

fn begin_result(
    disposition: BeginRunDisposition,
    run: RunRecord,
    prompt: BoundRunPrompt,
) -> BeginRunResult {
    BeginRunResult {
        v: RUN_STORE_VERSION,
        disposition,
        run,
        prompt,
    }
}

fn insert_run(
    transaction: &Transaction<'_>,
    record: &RunRecord,
    key: &str,
) -> Result<(), RunStoreError> {
    let execution = encode_execution(&record.execution)?;
    let cursor =
        RunJournalCursor::new(record).map_err(|error| conflict(RunEntity::Run, &error.message))?;
    let cursor = encode_cursor(&cursor)?;
    transaction
        .execute(
            "INSERT INTO runs(
               id,conversation_id,prompt_id,project_id,execution_json,cursor_json,
               journal_bytes,idempotency_key,protocol_version,created_at_ms
             ) VALUES(?1,?2,?3,?4,?5,?6,0,?7,?8,?9)",
            params![
                record.run_id,
                record.conversation_id,
                record.prompt_id,
                record.project_id,
                execution,
                cursor,
                key,
                i64::from(record.protocol_version),
                to_i64(record.created_at_ms)?
            ],
        )
        .map_err(write_error)?;
    Ok(())
}

fn encode_execution(
    execution: &crate::runtime_domain::RunExecution,
) -> Result<String, RunStoreError> {
    let encoded = serde_json::to_string(execution)
        .map_err(|error| conflict(RunEntity::Run, &format!("execution cannot encode: {error}")))?;
    if encoded.len() > MAX_RUN_EXECUTION_JSON_BYTES {
        return Err(conflict(
            RunEntity::Run,
            "execution configuration exceeds its durable byte limit",
        ));
    }
    Ok(encoded)
}

fn encode_cursor(cursor: &RunJournalCursor) -> Result<String, RunStoreError> {
    let encoded = serde_json::to_string(cursor).map_err(|error| {
        conflict(
            RunEntity::Run,
            &format!("Run cursor cannot encode: {error}"),
        )
    })?;
    if encoded.len() > MAX_RUN_CURSOR_JSON_BYTES {
        return Err(conflict(
            RunEntity::Event,
            "Run cursor exceeds its durable byte limit",
        ));
    }
    Ok(encoded)
}

fn insert_event(
    transaction: &Transaction<'_>,
    event: &RuntimeEvent,
    json: &str,
) -> Result<(), RunStoreError> {
    transaction
        .execute(
            "INSERT INTO run_events(run_id,seq,event_json) VALUES(?1,?2,?3)",
            params![event.run_id, to_i64(event.seq)?, json],
        )
        .map_err(write_error)?;
    Ok(())
}

fn update_journal(
    transaction: &Transaction<'_>,
    run_id: &str,
    cursor_json: &str,
    journal_bytes: usize,
) -> Result<(), RunStoreError> {
    let journal_bytes = i64::try_from(journal_bytes)
        .map_err(|error| conflict(RunEntity::Event, &error.to_string()))?;
    let changed = transaction
        .execute(
            "UPDATE runs SET cursor_json = ?1,journal_bytes = ?2 WHERE id = ?3",
            params![cursor_json, journal_bytes, run_id],
        )
        .map_err(write_error)?;
    if changed != 1 {
        return Err(corrupt("Run disappeared while updating its journal cursor"));
    }
    Ok(())
}

fn encode_event(event: &RuntimeEvent) -> Result<String, RunStoreError> {
    serde_json::to_string(event).map_err(|error| {
        conflict(
            RunEntity::Event,
            &format!("event cannot be serialized: {error}"),
        )
    })
}

pub(super) fn immediate(connection: &mut Connection) -> Result<Transaction<'_>, RunStoreError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)
}

fn corrupt(message: &str) -> RunStoreError {
    RunStoreError::Corrupt {
        message: message.into(),
    }
}

fn to_i64(value: u64) -> Result<i64, RunStoreError> {
    i64::try_from(value).map_err(|error| conflict(RunEntity::Run, &error.to_string()))
}

fn write_error(error: rusqlite::Error) -> RunStoreError {
    if matches!(
        error,
        rusqlite::Error::SqliteFailure(problem, _)
            if problem.code == rusqlite::ErrorCode::ConstraintViolation
    ) {
        return conflict(RunEntity::Run, &error.to_string());
    }
    unavailable(error)
}

fn not_found(entity: RunEntity, id: &str) -> RunStoreError {
    RunStoreError::NotFound {
        entity,
        id: id.into(),
    }
}

fn conflict(entity: RunEntity, message: &str) -> RunStoreError {
    RunStoreError::Conflict {
        entity,
        message: message.into(),
    }
}

fn unavailable(error: impl std::fmt::Display) -> RunStoreError {
    RunStoreError::Unavailable {
        message: error.to_string(),
    }
}
