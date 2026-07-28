use std::sync::Arc;

use crate::{
    MAX_ENTITY_ID_BYTES, MAX_IDEMPOTENCY_KEY_BYTES, MAX_PROMPT_BYTES, RunError, RunField,
    runtime_domain::{
        BeginRun, BeginRunResult, MAX_RUN_LIST_LIMIT, PromptRecord, RunInspection, RunOutcome,
        RunRecord, RunRecoveryState, RunStore,
    },
};

pub struct RunService {
    store: Arc<dyn RunStore>,
}

impl RunService {
    #[must_use]
    pub fn new(store: Arc<dyn RunStore>) -> Self {
        Self { store }
    }

    /// Creates or replays one durable Run intent.
    ///
    /// # Errors
    ///
    /// Returns validation errors before storage is called, or a structured
    /// storage error when the atomic begin operation fails.
    pub fn begin_run(&self, request: &BeginRun) -> Result<BeginRunResult, RunError> {
        required_id(&request.run_id, RunField::RunId)?;
        required_id(&request.conversation_id, RunField::ConversationId)?;
        required_id(&request.prompt_id, RunField::PromptId)?;
        required_id(&request.project_id, RunField::ProjectId)?;
        required(
            &request.idempotency_key,
            RunField::IdempotencyKey,
            MAX_IDEMPOTENCY_KEY_BYTES,
        )?;
        Ok(self.store.begin_run(request)?)
    }

    /// Looks up a retry key without creating a Run.
    ///
    /// # Errors
    ///
    /// Returns validation or structured storage errors.
    pub fn find_run_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<RunRecord>, RunError> {
        required(
            idempotency_key,
            RunField::IdempotencyKey,
            MAX_IDEMPOTENCY_KEY_BYTES,
        )?;
        Ok(self.store.find_run_by_idempotency_key(idempotency_key)?)
    }

    /// Lists Runs newest first, optionally restricted to one Conversation.
    ///
    /// # Errors
    ///
    /// Returns validation errors before storage is called, or a structured
    /// storage error when the query fails.
    pub fn list_runs(
        &self,
        conversation_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<RunRecord>, RunError> {
        if let Some(id) = conversation_id {
            required_id(id, RunField::ConversationId)?;
        }
        run_limit(limit)?;
        Ok(self.store.list_runs(conversation_id, limit)?)
    }

    /// Loads one durable Run prefix and its derived recovery state.
    ///
    /// # Errors
    ///
    /// Returns validation errors before storage is called, or a structured
    /// storage error when inspection fails.
    pub fn inspect_run(&self, run_id: &str) -> Result<RunInspection, RunError> {
        required_id(run_id, RunField::RunId)?;
        Ok(self.store.inspect_run(run_id)?)
    }

    /// Atomically reconciles the assistant Prompt authorized by a completed Run.
    ///
    /// # Errors
    ///
    /// Returns an error for non-completed Runs, invalid answer bounds, or
    /// storage conflicts/corruption.
    pub fn reconcile_completed_assistant(&self, run_id: &str) -> Result<PromptRecord, RunError> {
        required_id(run_id, RunField::RunId)?;
        let inspection = self.store.inspect_run(run_id)?;
        let RunRecoveryState::Terminal {
            outcome: RunOutcome::Completed { answer },
        } = &inspection.recovery.state
        else {
            return Err(RunError::NotCompleted);
        };
        required(answer, RunField::Answer, MAX_PROMPT_BYTES)?;
        Ok(self.store.reconcile_completed_assistant(run_id)?)
    }
}

fn required_id(value: &str, field: RunField) -> Result<(), RunError> {
    required(value, field, MAX_ENTITY_ID_BYTES)
}

fn required(value: &str, field: RunField, max_bytes: usize) -> Result<(), RunError> {
    if value.trim().is_empty() {
        return Err(RunError::Empty { field });
    }
    if value.len() > max_bytes {
        return Err(RunError::TooLong { field, max_bytes });
    }
    Ok(())
}

fn run_limit(limit: usize) -> Result<(), RunError> {
    if (1..=MAX_RUN_LIST_LIMIT).contains(&limit) {
        return Ok(());
    }
    Err(RunError::OutOfRange {
        field: RunField::RunLimit,
        min: 1,
        max: MAX_RUN_LIST_LIMIT,
    })
}
