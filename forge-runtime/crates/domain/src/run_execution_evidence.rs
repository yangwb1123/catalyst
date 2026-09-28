//! Content-free evidence binding for one observed Run and one Runner receipt.
//!
//! This module joins two already projected metadata values. It does not read
//! a clock, persist a receipt, issue a lease, select a target, or authorize
//! execution.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ConversationOwner;
use crate::execution::session_runner_receipt::SessionRunnerReceiptObservation;
use crate::run_observed::{FORGE_RUN_OBSERVED_V1, RunObserved, owner_reference};

pub const RUN_EXECUTION_EVIDENCE_SCHEMA_VERSION: &str = "forge.run.execution-evidence.v1";
pub const RUN_EXECUTION_EVIDENCE_EVALUATION_MODE: &str = "pure_run_execution_evidence_binding";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_IDENTIFIER_BYTES: usize = 85;

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunExecutionEvidenceAuthority {
    pub identity_verified: bool,
    pub owner_authorized: bool,
    pub run_authoritative: bool,
    pub receipt_persisted: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunExecutionEvidenceInput {
    pub run: RunObserved,
    pub receipt: SessionRunnerReceiptObservation,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunExecutionEvidence {
    pub api_version: String,
    pub evaluation_mode: String,
    pub owner_ref: String,
    pub conversation_id: String,
    pub run_id: String,
    pub prompt_id: String,
    pub run_status: String,
    pub attempt_id: String,
    pub target_id: String,
    pub command_id: String,
    pub command_sha256: String,
    pub disposition_kind: String,
    pub receipt_observed_at_ms: u64,
    pub uncertain: bool,
    pub reconciliation_required: bool,
    pub metadata_observed: bool,
    pub content_included: bool,
    pub authority: RunExecutionEvidenceAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunExecutionEvidenceError {
    InvalidRun,
    InvalidReceipt,
    InvalidEvidence,
    OwnerMismatch,
    BindingMismatch,
}

impl RunExecutionEvidenceError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidRun | Self::InvalidReceipt | Self::InvalidEvidence => {
                "invalid_run_execution_evidence"
            }
            Self::OwnerMismatch => "run_execution_owner_mismatch",
            Self::BindingMismatch => "run_execution_binding_mismatch",
        }
    }
}

impl fmt::Display for RunExecutionEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RunExecutionEvidenceError {}

impl RunExecutionEvidence {
    /// Validates a caller-supplied wire projection without adding authority.
    ///
    /// This is deliberately separate from `observe_run_execution_evidence` so
    /// local CLI/TUI consumers can safely inspect a fixture without
    /// reconstructing the source Run and receipt observations.
    ///
    /// # Errors
    ///
    /// Returns `InvalidEvidence` for invalid identifiers, metadata or bounds,
    /// inconsistent disposition flags, or included content or authority.
    pub fn validate(&self) -> Result<(), RunExecutionEvidenceError> {
        if self.api_version != RUN_EXECUTION_EVIDENCE_SCHEMA_VERSION
            || self.evaluation_mode != RUN_EXECUTION_EVIDENCE_EVALUATION_MODE
            || !valid_digest(&self.owner_ref)
            || !valid_identifier(&self.conversation_id)
            || !valid_identifier(&self.run_id)
            || !valid_identifier(&self.prompt_id)
            || !matches!(
                self.run_status.as_str(),
                "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
            )
            || !valid_identifier(&self.attempt_id)
            || !valid_identifier(&self.target_id)
            || !valid_identifier(&self.command_id)
            || !valid_digest(&self.command_sha256)
            || !matches!(
                self.disposition_kind.as_str(),
                "completed" | "failed" | "uncertain"
            )
            || self.receipt_observed_at_ms > MAX_SAFE_INTEGER
            || self.uncertain != (self.disposition_kind == "uncertain")
            || self.reconciliation_required != self.uncertain
            || !self.metadata_observed
            || self.content_included
            || self.authority != RunExecutionEvidenceAuthority::default()
        {
            return Err(RunExecutionEvidenceError::InvalidEvidence);
        }
        Ok(())
    }
}

/// Binds an existing Run observation to an existing session receipt
/// observation. The result remains preview-only evidence with no authority.
///
/// # Errors
///
/// Returns an error when the Run metadata, nested receipt, owner reference, or
/// Conversation/Prompt/Run binding is invalid.
pub fn observe_run_execution_evidence(
    input: RunExecutionEvidenceInput,
) -> Result<RunExecutionEvidence, RunExecutionEvidenceError> {
    if !valid_run(&input.run) {
        return Err(RunExecutionEvidenceError::InvalidRun);
    }
    input
        .receipt
        .validate()
        .map_err(|_| RunExecutionEvidenceError::InvalidReceipt)?;

    let owner = ConversationOwner {
        issuer: input.receipt.owner.issuer.clone(),
        subject: input.receipt.owner.subject.clone(),
        tenant_id: input.receipt.owner.tenant_id.clone(),
    };
    if input.run.owner_ref != owner_reference(&owner) {
        return Err(RunExecutionEvidenceError::OwnerMismatch);
    }
    if input.run.conversation_id != input.receipt.conversation_id
        || input.run.run_id != input.receipt.run_id
        || input.run.prompt_id != input.receipt.prompt_id
    {
        return Err(RunExecutionEvidenceError::BindingMismatch);
    }

    let receipt = input.receipt.receipt_observation;
    Ok(RunExecutionEvidence {
        api_version: RUN_EXECUTION_EVIDENCE_SCHEMA_VERSION.into(),
        evaluation_mode: RUN_EXECUTION_EVIDENCE_EVALUATION_MODE.into(),
        owner_ref: input.run.owner_ref,
        conversation_id: input.run.conversation_id,
        run_id: input.run.run_id,
        prompt_id: input.run.prompt_id,
        run_status: input.run.status.into(),
        attempt_id: receipt.attempt_id,
        target_id: receipt.target_id,
        command_id: receipt.command_id,
        command_sha256: receipt.command_sha256,
        disposition_kind: receipt.disposition_kind,
        receipt_observed_at_ms: receipt.observed_at_ms,
        uncertain: receipt.uncertain,
        reconciliation_required: receipt.reconciliation_required,
        metadata_observed: true,
        content_included: false,
        authority: RunExecutionEvidenceAuthority::default(),
    })
}

fn valid_run(value: &RunObserved) -> bool {
    value.api_version == FORGE_RUN_OBSERVED_V1
        && valid_digest(&value.owner_ref)
        && valid_identifier(&value.conversation_id)
        && valid_identifier(&value.run_id)
        && valid_identifier(&value.prompt_id)
        && value.created_at_ms <= MAX_SAFE_INTEGER
        && value.latest_sequence > 0
        && value.latest_sequence <= MAX_SAFE_INTEGER
        && matches!(
            value.status,
            "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
        )
        && value.metadata_observed
        && !value.content_included
        && value.authority == crate::run_observed::RunObservedAuthority::default()
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.chars().all(|character| {
            !character.is_control()
                && !character.is_whitespace()
                && !matches!(character, ':' | '/' | '\\')
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests;
