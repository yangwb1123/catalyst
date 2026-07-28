use forge_runtime_domain::{
    MAX_RUN_EVENT_JSON_BYTES, MAX_RUN_EVENTS, MAX_RUN_JOURNAL_BYTES, RunEntity, RunRecord,
    RunStoreError, RuntimeEvent, RuntimeEventKind,
};
use rusqlite::{Connection, OptionalExtension};

pub(super) fn validate_bound_prompt(
    connection: &Connection,
    run: &RunRecord,
    events: &[RuntimeEvent],
) -> Result<(), RunStoreError> {
    let Some(RuntimeEvent {
        kind: RuntimeEventKind::RunStarted { prompt },
        ..
    }) = events.first()
    else {
        return Ok(());
    };
    let expected = connection
        .query_row(
            "SELECT content FROM prompts WHERE id = ?1 AND conversation_id = ?2",
            [&run.prompt_id, &run.conversation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| unavailable(&error))?
        .ok_or_else(|| RunStoreError::Corrupt {
            message: "Run references a missing or mismatched Prompt".into(),
        })?;
    if prompt == &expected {
        return Ok(());
    }
    Err(RunStoreError::Corrupt {
        message: "run_started prompt does not match the bound Prompt".into(),
    })
}

pub(super) fn validate_candidate_capacity(
    event_count: usize,
    journal_bytes: usize,
    candidate_bytes: usize,
) -> Result<(), RunStoreError> {
    if candidate_bytes > MAX_RUN_EVENT_JSON_BYTES {
        return Err(conflict("Run event exceeds the durable JSON byte limit"));
    }
    if event_count >= MAX_RUN_EVENTS {
        return Err(conflict("Run event count exceeds the durable limit"));
    }
    if candidate_bytes > MAX_RUN_JOURNAL_BYTES.saturating_sub(journal_bytes) {
        return Err(conflict("Run journal exceeds the durable byte limit"));
    }
    Ok(())
}

fn conflict(message: &str) -> RunStoreError {
    RunStoreError::Conflict {
        entity: RunEntity::Event,
        message: message.into(),
    }
}

fn unavailable(error: &rusqlite::Error) -> RunStoreError {
    RunStoreError::Unavailable {
        message: error.to_string(),
    }
}
