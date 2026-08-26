use forge_runtime_domain::{
    BeginRun, BeginRunDisposition, PROTOCOL_VERSION, RUN_STORE_VERSION, RunInspection,
    RunRecoveryState, RuntimeEvent, RuntimeEventKind,
};
use sha2::{Digest, Sha256};

use super::{RESTART_RUN_ID_PREFIX, RunService, required, required_id};
use crate::{MAX_IDEMPOTENCY_KEY_BYTES, RunError, RunField};

const RESTART_RUN_ID_DOMAIN: &[u8] = b"forgeos.project-run.restart-id.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareRunRestart {
    pub source_run_id: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrepareRunRestartResult {
    pub disposition: BeginRunDisposition,
    pub inspection: RunInspection,
}

impl RunService {
    /// Materializes a terminal Run's durable input as a new, resume-ready Run.
    ///
    /// The operation copies no journal suffix or result and performs no model,
    /// tool, workspace, or network effect. An exact retry repairs or replays the
    /// deterministic initial event.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, a nonterminal or corrupt source, an
    /// idempotency-key conflict, or unavailable storage.
    pub fn prepare_restart(
        &self,
        input: &PrepareRunRestart,
    ) -> Result<PrepareRunRestartResult, RunError> {
        validate_input(input)?;
        let source = self.store.inspect_run(&input.source_run_id)?;
        if !matches!(source.recovery.state, RunRecoveryState::Terminal { .. }) {
            return Err(RunError::RestartSourceNotTerminal);
        }
        let run_id = restart_run_id(&input.source_run_id, &input.idempotency_key);
        let begin_request = BeginRun {
            v: RUN_STORE_VERSION,
            run_id: run_id.clone(),
            conversation_id: source.run.conversation_id,
            prompt_id: source.run.prompt_id,
            project_id: source.run.project_id,
            execution: source.run.execution,
            idempotency_key: input.idempotency_key.clone(),
            created_at_ms: input.created_at_ms,
        };
        let begin_result = match self.begin_restart_run(&begin_request) {
            Ok(result) => result,
            Err(error) => {
                if restart_key_has_another_owner(self, &input.idempotency_key, &run_id)? {
                    return Err(RunError::RestartIdempotencyConflict);
                }
                return Err(error);
            }
        };
        if begin_result.run.run_id != run_id {
            return Err(RunError::RestartIdempotencyConflict);
        }
        self.store.append_event(&restart_event(&begin_result))?;
        Ok(PrepareRunRestartResult {
            disposition: begin_result.disposition,
            inspection: self.store.inspect_run(&run_id)?,
        })
    }
}

fn restart_key_has_another_owner(
    service: &RunService,
    idempotency_key: &str,
    expected_run_id: &str,
) -> Result<bool, RunError> {
    Ok(service
        .find_run_by_idempotency_key(idempotency_key)?
        .is_some_and(|run| run.run_id != expected_run_id))
}

fn validate_input(input: &PrepareRunRestart) -> Result<(), RunError> {
    required_id(&input.source_run_id, RunField::RunId)?;
    required(
        &input.idempotency_key,
        RunField::IdempotencyKey,
        MAX_IDEMPOTENCY_KEY_BYTES,
    )
}

fn restart_run_id(source_run_id: &str, idempotency_key: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(RESTART_RUN_ID_DOMAIN);
    digest.update(Sha256::digest(source_run_id.as_bytes()));
    digest.update(idempotency_key.as_bytes());
    format!("{RESTART_RUN_ID_PREFIX}{:x}", digest.finalize())
}

fn restart_event(result: &forge_runtime_domain::BeginRunResult) -> RuntimeEvent {
    RuntimeEvent {
        v: PROTOCOL_VERSION,
        session_id: result.run.conversation_id.clone(),
        run_id: result.run.run_id.clone(),
        seq: 1,
        emitted_at_ms: result.run.created_at_ms,
        kind: RuntimeEventKind::RunStarted {
            prompt: result.prompt.content.clone(),
        },
    }
}
