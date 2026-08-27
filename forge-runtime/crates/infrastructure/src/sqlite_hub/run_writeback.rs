use crate::runtime_domain::{
    PromptRecord, RunEntity, RunInspection, RunOutcome, RunRecord, RunRecoveryState, RunStoreError,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use super::{HubEntity, HubStoreError, rows, run_read, write};

pub(super) fn reconcile_completed_assistant(
    connection: &mut Connection,
    run_id: &str,
) -> Result<PromptRecord, RunStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let inspection = run_read::inspect(&transaction, run_id)?;
    let answer = completed_answer(&inspection)?;
    let prompt = writeback_locked(&transaction, &inspection.run, answer)?;
    transaction.commit().map_err(unavailable)?;
    Ok(prompt)
}

fn completed_answer(inspection: &RunInspection) -> Result<&str, RunStoreError> {
    match &inspection.recovery.state {
        RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { answer },
        } => Ok(answer),
        _ => Err(conflict(
            RunEntity::Run,
            "assistant writeback requires a completed terminal Run",
        )),
    }
}

fn writeback_locked(
    transaction: &Transaction<'_>,
    record: &RunRecord,
    answer: &str,
) -> Result<PromptRecord, RunStoreError> {
    if let Some(existing) = existing_writeback(transaction, &record.run_id)? {
        ensure_same_writeback(&existing, record, answer)?;
        return Ok(existing);
    }
    let prompt = new_prompt(transaction, record, answer)?;
    write::insert_prompt(transaction, &prompt).map_err(hub_error)?;
    transaction
        .execute(
            "INSERT INTO run_assistant_prompts(run_id,prompt_id) VALUES(?1,?2)",
            params![record.run_id, prompt.id],
        )
        .map_err(unavailable)?;
    Ok(prompt)
}

fn new_prompt(
    transaction: &Transaction<'_>,
    record: &RunRecord,
    answer: &str,
) -> Result<PromptRecord, RunStoreError> {
    Ok(PromptRecord {
        id: rows::new_id(transaction, "prompt").map_err(hub_error)?,
        conversation_id: record.conversation_id.clone(),
        role: "assistant".into(),
        content: answer.into(),
        idempotency_key: rows::new_id(transaction, "run-assistant-key").map_err(hub_error)?,
        created_at_ms: rows::now_ms().map_err(hub_error)?,
    })
}

fn existing_writeback(
    transaction: &Transaction<'_>,
    run_id: &str,
) -> Result<Option<PromptRecord>, RunStoreError> {
    transaction
        .query_row(
            "SELECT p.id,p.conversation_id,p.role,p.content,p.idempotency_key,p.created_at_ms
             FROM run_assistant_prompts w
             JOIN prompts p ON p.id = w.prompt_id
             WHERE w.run_id = ?1",
            [run_id],
            rows::prompt,
        )
        .optional()
        .map_err(unavailable)
}

fn ensure_same_writeback(
    prompt: &PromptRecord,
    record: &RunRecord,
    answer: &str,
) -> Result<(), RunStoreError> {
    if prompt.conversation_id == record.conversation_id
        && prompt.role == "assistant"
        && prompt.content == answer
    {
        return Ok(());
    }
    Err(corrupt(
        "Run assistant writeback disagrees with terminal evidence",
    ))
}

fn hub_error(error: HubStoreError) -> RunStoreError {
    match error {
        HubStoreError::NotFound { entity, id } => match run_entity(entity) {
            Some(entity) => RunStoreError::NotFound { entity, id },
            None => unexpected_entity(entity),
        },
        HubStoreError::Conflict { entity, message } => match run_entity(entity) {
            Some(entity) => RunStoreError::Conflict { entity, message },
            None => unexpected_entity(entity),
        },
        HubStoreError::Unavailable { message } => RunStoreError::Unavailable { message },
        HubStoreError::Corrupt { message } => RunStoreError::Corrupt { message },
    }
}

fn run_entity(entity: HubEntity) -> Option<RunEntity> {
    match entity {
        HubEntity::Project => Some(RunEntity::Project),
        HubEntity::Conversation => Some(RunEntity::Conversation),
        HubEntity::Prompt => Some(RunEntity::Prompt),
        HubEntity::Group
        | HubEntity::GroupProjectMember
        | HubEntity::GroupRun
        | HubEntity::GroupExecution
        | HubEntity::GroupModelAnalysis
        | HubEntity::GroupAnalysisPanel
        | HubEntity::GroupPanelSynthesis
        | HubEntity::GroupAgentGraph
        | HubEntity::GroupAgentGraphRun
        | HubEntity::GroupAgentGraphExecutionSchedule
        | HubEntity::GroupAgentScheduledNodeContract
        | HubEntity::GroupAgentScheduledNodeProviderRequest
        | HubEntity::GroupAgentNodeExecutionContract
        | HubEntity::GroupAgentNodeDispatchRequest
        | HubEntity::GroupAgentNodeLifecycle
        | HubEntity::GroupAgentScheduledNodeLifecycle
        | HubEntity::ScheduledGraphController
        | HubEntity::GovernanceRecord => None,
    }
}

fn unexpected_entity(entity: HubEntity) -> RunStoreError {
    corrupt(&format!(
        "Run assistant writeback received unexpected Hub entity {entity:?}"
    ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrelated_hub_entity_is_not_misreported_as_a_run() {
        let error = HubStoreError::Conflict {
            entity: HubEntity::GovernanceRecord,
            message: "unreachable foreign error".into(),
        };
        let mapped = hub_error(error);
        let RunStoreError::Corrupt { message } = mapped else {
            panic!("unexpected Hub entity must be corruption");
        };
        assert!(message.contains("GovernanceRecord"), "{message}");
    }
}
