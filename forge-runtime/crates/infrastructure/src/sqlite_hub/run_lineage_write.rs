use crate::runtime_domain::{
    BeginRun, BeginRunBranch, BeginRunBranchResult, BeginRunDisposition, PROTOCOL_VERSION,
    ROOT_INPUT_SOURCE_EVENT_SEQ, RUN_LINEAGE_VERSION, RUN_STORE_VERSION, RunBranchMode, RunEntity,
    RunLineageRecord, RunRecoveryState, RunStoreError, RuntimeEvent, RuntimeEventKind,
    expected_lineage_sha256, source_event_sha256,
};
use rusqlite::{Connection, Transaction, params};

use super::{run_lineage_read, run_read, run_write};

pub(super) fn begin(
    connection: &mut Connection,
    request: &BeginRunBranch,
) -> Result<BeginRunBranchResult, RunStoreError> {
    validate_request(request)?;
    let transaction = run_write::immediate(connection)?;
    let result = begin_locked(&transaction, request)?;
    transaction.commit().map_err(unavailable)?;
    Ok(result)
}

fn begin_locked(
    transaction: &Transaction<'_>,
    request: &BeginRunBranch,
) -> Result<BeginRunBranchResult, RunStoreError> {
    let parent = run_read::inspect(transaction, &request.parent_run_id)?;
    require_terminal_parent(&parent)?;
    let source = parent
        .events
        .first()
        .ok_or_else(|| corrupt("terminal parent has no root event"))?;
    let RuntimeEventKind::RunStarted { prompt } = &source.kind else {
        return Err(corrupt("terminal parent does not begin with run_started"));
    };
    let begin = child_begin(request, &parent.run);
    run_write::validate_begin_request(&begin)?;
    let started = run_write::begin_run_locked(transaction, &begin)?;
    if started.run.run_id != request.child_run_id {
        return Err(conflict("idempotency key belongs to another Run operation"));
    }
    let lineage = lineage_record(request, &parent.run, source, started.run.created_at_ms)?;
    finish_branch(
        transaction,
        started.disposition,
        &lineage,
        &child_seed(&started.run, prompt),
    )?;
    Ok(BeginRunBranchResult {
        disposition: started.disposition,
        run: started.run,
        prompt: started.prompt,
        lineage,
    })
}

fn child_begin(request: &BeginRunBranch, parent: &crate::runtime_domain::RunRecord) -> BeginRun {
    BeginRun {
        v: RUN_STORE_VERSION,
        run_id: request.child_run_id.clone(),
        conversation_id: parent.conversation_id.clone(),
        prompt_id: parent.prompt_id.clone(),
        project_id: parent.project_id.clone(),
        execution: parent.execution.clone(),
        idempotency_key: request.idempotency_key.clone(),
        created_at_ms: request.created_at_ms,
    }
}

fn lineage_record(
    request: &BeginRunBranch,
    parent: &crate::runtime_domain::RunRecord,
    source: &RuntimeEvent,
    created_at_ms: u64,
) -> Result<RunLineageRecord, RunStoreError> {
    let mut record = RunLineageRecord {
        v: RUN_LINEAGE_VERSION,
        child_run_id: request.child_run_id.clone(),
        parent_run_id: request.parent_run_id.clone(),
        branch_mode: RunBranchMode::RootInput,
        source_event_seq: ROOT_INPUT_SOURCE_EVENT_SEQ,
        source_event_sha256: source_event_sha256(parent, source)
            .map_err(|error| lineage_error(&error))?,
        lineage_sha256: String::new(),
        created_at_ms,
    };
    record.lineage_sha256 = expected_lineage_sha256(&record);
    record.validate().map_err(|error| lineage_error(&error))?;
    Ok(record)
}

fn finish_branch(
    transaction: &Transaction<'_>,
    disposition: BeginRunDisposition,
    record: &RunLineageRecord,
    seed: &RuntimeEvent,
) -> Result<(), RunStoreError> {
    match disposition {
        BeginRunDisposition::Created => {
            insert_lineage(transaction, record)?;
            run_write::append_event_locked(transaction, seed)
        }
        BeginRunDisposition::Replayed => validate_replayed_branch(transaction, record, seed),
    }
}

fn validate_replayed_branch(
    transaction: &Transaction<'_>,
    record: &RunLineageRecord,
    seed: &RuntimeEvent,
) -> Result<(), RunStoreError> {
    let stored = run_lineage_read::find(transaction, &record.child_run_id)?
        .ok_or_else(|| corrupt("replayed Run branch is missing its lineage record"))?;
    if stored != *record {
        return Err(conflict(
            "branch lineage replay diverges from committed input",
        ));
    }
    let child = run_read::inspect_base(transaction, &record.child_run_id)?;
    let committed = child
        .events
        .first()
        .ok_or_else(|| corrupt("replayed Run branch is missing its root run_started seed event"))?;
    if committed != seed {
        return Err(corrupt(
            "replayed Run branch root run_started seed event diverged",
        ));
    }
    Ok(())
}

fn insert_lineage(
    transaction: &Transaction<'_>,
    record: &RunLineageRecord,
) -> Result<(), RunStoreError> {
    transaction
        .execute(
            "INSERT INTO run_lineages(
               child_run_id,lineage_version,relation_kind,branch_mode,parent_run_id,
               source_event_seq,source_event_sha256,lineage_sha256,created_at_ms
             ) VALUES(?1,?2,'branch','root_input',?3,?4,?5,?6,?7)",
            params![
                record.child_run_id,
                i64::from(record.v),
                record.parent_run_id,
                to_i64(record.source_event_seq)?,
                decode_digest(&record.source_event_sha256)?,
                decode_digest(&record.lineage_sha256)?,
                to_i64(record.created_at_ms)?,
            ],
        )
        .map_err(write_error)?;
    Ok(())
}

fn child_seed(run: &crate::runtime_domain::RunRecord, prompt: &str) -> RuntimeEvent {
    RuntimeEvent {
        v: PROTOCOL_VERSION,
        session_id: run.conversation_id.clone(),
        run_id: run.run_id.clone(),
        seq: 1,
        emitted_at_ms: run.created_at_ms,
        kind: RuntimeEventKind::RunStarted {
            prompt: prompt.into(),
        },
    }
}

fn require_terminal_parent(
    parent: &crate::runtime_domain::RunInspection,
) -> Result<(), RunStoreError> {
    if matches!(parent.recovery.state, RunRecoveryState::Terminal { .. }) {
        Ok(())
    } else {
        Err(conflict("run branch requires a terminal parent Run"))
    }
}

fn validate_request(request: &BeginRunBranch) -> Result<(), RunStoreError> {
    if request.v != RUN_LINEAGE_VERSION
        || request.child_run_id.trim().is_empty()
        || request.parent_run_id.trim().is_empty()
        || request.idempotency_key.trim().is_empty()
        || request.child_run_id == request.parent_run_id
    {
        return Err(conflict("invalid Run branch request"));
    }
    Ok(())
}

fn decode_digest(value: &str) -> Result<Vec<u8>, RunStoreError> {
    if value.len() != 64 {
        return Err(corrupt("Run lineage digest is not lowercase SHA-256"));
    }
    (0..32)
        .map(|index| u8::from_str_radix(&value[index * 2..index * 2 + 2], 16))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| corrupt("Run lineage digest is not lowercase SHA-256"))
}

fn to_i64(value: u64) -> Result<i64, RunStoreError> {
    i64::try_from(value).map_err(|error| conflict(&error.to_string()))
}

fn lineage_error(error: &crate::runtime_domain::RunLineageError) -> RunStoreError {
    corrupt(&error.message)
}

fn conflict(message: &str) -> RunStoreError {
    RunStoreError::Conflict {
        entity: RunEntity::Lineage,
        message: message.into(),
    }
}

fn corrupt(message: &str) -> RunStoreError {
    RunStoreError::Corrupt {
        message: message.into(),
    }
}

fn write_error(error: rusqlite::Error) -> RunStoreError {
    if matches!(
        error,
        rusqlite::Error::SqliteFailure(problem, _)
            if problem.code == rusqlite::ErrorCode::ConstraintViolation
    ) {
        return conflict(&error.to_string());
    }
    unavailable(error)
}

fn unavailable(error: impl std::fmt::Display) -> RunStoreError {
    RunStoreError::Unavailable {
        message: error.to_string(),
    }
}
