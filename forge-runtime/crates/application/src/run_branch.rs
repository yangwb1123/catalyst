use crate::runtime_domain::{
    BeginRunBranch, BeginRunDisposition, RUN_LINEAGE_VERSION, RunInspection, RunLineageRecord,
    RunRecoveryState,
};
use sha2::{Digest, Sha256};

use super::{BRANCH_RUN_ID_PREFIX, RunService, required, required_id};
use crate::{MAX_IDEMPOTENCY_KEY_BYTES, RunError, RunField};

const BRANCH_RUN_ID_DOMAIN: &[u8] = b"forgeos.project-run.root-input-branch-id.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareRunBranch {
    pub parent_run_id: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrepareRunBranchResult {
    pub disposition: BeginRunDisposition,
    pub inspection: RunInspection,
    pub lineage: RunLineageRecord,
}

impl RunService {
    /// Creates an atomic, resume-ready child at a terminal parent's root input.
    ///
    /// The child inherits only Project, Conversation, Prompt and execution
    /// configuration. Parent output and journal suffix are never copied.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, a nonterminal parent, conflicting
    /// operation ownership, corrupt lineage, or unavailable storage.
    pub fn prepare_branch(
        &self,
        input: &PrepareRunBranch,
    ) -> Result<PrepareRunBranchResult, RunError> {
        validate_input(input)?;
        let parent = self.store.inspect_run(&input.parent_run_id)?;
        if !matches!(parent.recovery.state, RunRecoveryState::Terminal { .. }) {
            return Err(RunError::BranchParentNotTerminal);
        }
        let child_run_id = branch_run_id(&input.parent_run_id, &input.idempotency_key);
        let request = BeginRunBranch {
            v: RUN_LINEAGE_VERSION,
            child_run_id: child_run_id.clone(),
            parent_run_id: input.parent_run_id.clone(),
            idempotency_key: input.idempotency_key.clone(),
            created_at_ms: input.created_at_ms,
        };
        let prepared = match self.store.begin_run_branch(&request) {
            Ok(result) => result,
            Err(error) => {
                if key_has_another_owner(self, input, &child_run_id)? {
                    return Err(RunError::BranchIdempotencyConflict);
                }
                return Err(error.into());
            }
        };
        if prepared.run.run_id != child_run_id
            || prepared.lineage.child_run_id != child_run_id
            || prepared.lineage.parent_run_id != input.parent_run_id
        {
            return Err(RunError::BranchIdempotencyConflict);
        }
        Ok(PrepareRunBranchResult {
            disposition: prepared.disposition,
            inspection: self.store.inspect_run(&child_run_id)?,
            lineage: prepared.lineage,
        })
    }

    /// Returns the immutable direct-parent record for a Run, if one exists.
    ///
    /// # Errors
    ///
    /// Returns validation, corruption, not-found, or storage errors.
    pub fn run_lineage(&self, run_id: &str) -> Result<Option<RunLineageRecord>, RunError> {
        required_id(run_id, RunField::RunId)?;
        self.store.inspect_run(run_id)?;
        Ok(self.store.find_run_lineage(run_id)?)
    }
}

fn key_has_another_owner(
    service: &RunService,
    input: &PrepareRunBranch,
    expected_run_id: &str,
) -> Result<bool, RunError> {
    Ok(service
        .find_run_by_idempotency_key(&input.idempotency_key)?
        .is_some_and(|run| run.run_id != expected_run_id))
}

fn validate_input(input: &PrepareRunBranch) -> Result<(), RunError> {
    required_id(&input.parent_run_id, RunField::RunId)?;
    required(
        &input.idempotency_key,
        RunField::IdempotencyKey,
        MAX_IDEMPOTENCY_KEY_BYTES,
    )
}

fn branch_run_id(parent_run_id: &str, idempotency_key: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(BRANCH_RUN_ID_DOMAIN);
    digest.update(Sha256::digest(parent_run_id.as_bytes()));
    digest.update(idempotency_key.as_bytes());
    format!("{BRANCH_RUN_ID_PREFIX}{:x}", digest.finalize())
}
