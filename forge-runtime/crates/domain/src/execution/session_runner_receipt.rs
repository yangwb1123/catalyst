//! Pure binding of a Runner terminal receipt observation to one session.
//!
//! This value bridge repeats owner and Conversation/Prompt/Run identities so a
//! client can display terminal evidence without treating it as execution
//! authority. It has no storage, clock, transport, lease, or process effect.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::runner_command::{
    RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE, RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION,
    RunnerTerminalReceiptAuthority, RunnerTerminalReceiptObservation,
};
use super::runner_execution_intent::{
    RUNNER_EXECUTION_INTENT_EVALUATION_MODE, RUNNER_EXECUTION_INTENT_SCHEMA_VERSION,
    RunnerExecutionIntentAuthority, RunnerExecutionIntentObservation, RunnerExecutionOwner,
};

pub const SESSION_RUNNER_RECEIPT_OBSERVATION_SCHEMA_VERSION: &str =
    "forge.session-runner-receipt-observation/v1";
pub const SESSION_RUNNER_RECEIPT_OBSERVATION_EVALUATION_MODE: &str =
    "pure_session_runner_receipt_binding_only";

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The session receipt wire contract represents each authority claim as a separate boolean."
)]
pub struct SessionRunnerReceiptAuthority {
    pub identity_verified: bool,
    pub receipt_persisted: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

/// Existing observations supplied to the pure session binding operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRunnerReceiptObservationRequest {
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub runner_execution_intent: RunnerExecutionIntentObservation,
    pub receipt_observation: RunnerTerminalReceiptObservation,
}

/// Owned wire form of the nested terminal observation. The base Runtime
/// projection uses static schema strings; this owned form lets strict clients
/// decode the session envelope without borrowing a process-static buffer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The owned wire form preserves the terminal observation's distinct evidence and follow-up predicates."
)]
pub struct SessionRunnerTerminalReceiptObservation {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub command_id: String,
    pub command_sha256: String,
    pub attempt_id: String,
    pub target_id: String,
    pub disposition_kind: String,
    pub observed_at_ms: u64,
    pub receipt_valid: bool,
    pub preview_only: bool,
    pub uncertain: bool,
    pub reconciliation_required: bool,
    pub manual_review_required: bool,
    pub automatic_retry: bool,
    pub follow_up: String,
    pub authority: RunnerTerminalReceiptAuthority,
}

impl From<RunnerTerminalReceiptObservation> for SessionRunnerTerminalReceiptObservation {
    fn from(value: RunnerTerminalReceiptObservation) -> Self {
        Self {
            schema_version: value.schema_version.into(),
            evaluation_mode: value.evaluation_mode.into(),
            command_id: value.command_id,
            command_sha256: value.command_sha256,
            attempt_id: value.attempt_id,
            target_id: value.target_id,
            disposition_kind: value.disposition_kind,
            observed_at_ms: value.observed_at_ms,
            receipt_valid: value.receipt_valid,
            preview_only: value.preview_only,
            uncertain: value.uncertain,
            reconciliation_required: value.reconciliation_required,
            manual_review_required: value.manual_review_required,
            automatic_retry: value.automatic_retry,
            follow_up: value.follow_up,
            authority: value.authority,
        }
    }
}

impl SessionRunnerTerminalReceiptObservation {
    fn validate(&self) -> Result<(), SessionRunnerReceiptError> {
        let uncertain = self.disposition_kind == "uncertain";
        if self.schema_version != RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION
            || self.evaluation_mode != RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE
            || !valid_identifier(&self.command_id)
            || !valid_identifier(&self.attempt_id)
            || !valid_identifier(&self.target_id)
            || !valid_digest(&self.command_sha256)
            || self.observed_at_ms > MAX_SAFE_INTEGER
            || !self.receipt_valid
            || !self.preview_only
            || self.uncertain != uncertain
            || self.reconciliation_required != uncertain
            || self.manual_review_required != uncertain
            || self.automatic_retry
            || (!uncertain
                && self.disposition_kind != "completed"
                && self.disposition_kind != "failed")
            || ((uncertain && self.follow_up != "reconciliation_manual")
                || (!uncertain && self.follow_up != "none"))
            || self.authority != RunnerTerminalReceiptAuthority::default()
        {
            return Err(SessionRunnerReceiptError::InvalidBinding);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRunnerReceiptObservation {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub receipt_observation: SessionRunnerTerminalReceiptObservation,
    pub prompt_run_binding_valid: bool,
    pub receipt_binding_valid: bool,
    pub preview_only: bool,
    pub selected_target_id: Option<String>,
    pub authority: SessionRunnerReceiptAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRunnerReceiptError {
    InvalidBinding,
}

impl fmt::Display for SessionRunnerReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "session Runner receipt observation is invalid: {self:?}"
        )
    }
}

impl std::error::Error for SessionRunnerReceiptError {}

/// Binds terminal evidence to an existing owner/session observation.
///
/// The returned value is always preview-only. It does not infer that the
/// caller owns the session, that the receipt was persisted, or that execution
/// was authorized.
///
/// # Errors
///
/// Returns [`SessionRunnerReceiptError::InvalidBinding`] when intent/session
/// identities or receipt/command bindings disagree, or supplied intent/receipt
/// metadata and flags violate the preview-only contract.
pub fn observe_session_runner_receipt(
    input: SessionRunnerReceiptObservationRequest,
) -> Result<SessionRunnerReceiptObservation, SessionRunnerReceiptError> {
    if !valid_intent(
        &input.runner_execution_intent,
        &input.owner,
        &input.conversation_id,
        &input.prompt_id,
        &input.run_id,
    ) || !valid_receipt(&input.receipt_observation)
        || input.receipt_observation.attempt_id != input.runner_execution_intent.attempt_id
        || input.receipt_observation.command_id != input.runner_execution_intent.command_id
        || input.receipt_observation.target_id != input.runner_execution_intent.target_id
        || input.receipt_observation.command_sha256 != input.runner_execution_intent.command_sha256
    {
        return Err(SessionRunnerReceiptError::InvalidBinding);
    }

    Ok(SessionRunnerReceiptObservation {
        schema_version: SESSION_RUNNER_RECEIPT_OBSERVATION_SCHEMA_VERSION.into(),
        evaluation_mode: SESSION_RUNNER_RECEIPT_OBSERVATION_EVALUATION_MODE.into(),
        owner: input.owner,
        conversation_id: input.conversation_id,
        prompt_id: input.prompt_id,
        run_id: input.run_id,
        receipt_observation: input.receipt_observation.into(),
        prompt_run_binding_valid: true,
        receipt_binding_valid: true,
        preview_only: true,
        selected_target_id: None,
        authority: SessionRunnerReceiptAuthority::default(),
    })
}

impl SessionRunnerReceiptObservation {
    /// Validates the canonical wire projection without adding authority.
    ///
    /// # Errors
    ///
    /// Returns [`SessionRunnerReceiptError::InvalidBinding`] for invalid schema,
    /// owner or session identities, a selected target, non-preview/authority claims,
    /// false binding predicates, or an invalid nested terminal observation.
    pub fn validate(&self) -> Result<(), SessionRunnerReceiptError> {
        if self.schema_version != SESSION_RUNNER_RECEIPT_OBSERVATION_SCHEMA_VERSION
            || self.evaluation_mode != SESSION_RUNNER_RECEIPT_OBSERVATION_EVALUATION_MODE
            || !valid_owner(&self.owner)
            || !valid_identifier(&self.conversation_id)
            || !valid_identifier(&self.prompt_id)
            || !valid_identifier(&self.run_id)
            || !self.prompt_run_binding_valid
            || !self.receipt_binding_valid
            || !self.preview_only
            || self.selected_target_id.is_some()
            || self.authority != SessionRunnerReceiptAuthority::default()
            || self.receipt_observation.validate().is_err()
        {
            return Err(SessionRunnerReceiptError::InvalidBinding);
        }
        Ok(())
    }
}

fn valid_intent(
    intent: &RunnerExecutionIntentObservation,
    owner: &RunnerExecutionOwner,
    conversation_id: &str,
    prompt_id: &str,
    run_id: &str,
) -> bool {
    intent.schema_version == RUNNER_EXECUTION_INTENT_SCHEMA_VERSION
        && intent.evaluation_mode == RUNNER_EXECUTION_INTENT_EVALUATION_MODE
        && &intent.owner == owner
        && intent.conversation_id == conversation_id
        && intent.prompt_id == prompt_id
        && intent.run_id == run_id
        && valid_identifier(conversation_id)
        && valid_identifier(prompt_id)
        && valid_identifier(run_id)
        && valid_identifier(&intent.attempt_id)
        && valid_identifier(&intent.command_id)
        && valid_identifier(&intent.target_id)
        && valid_digest(&intent.command_sha256)
        && intent.idempotency_key
            == format!(
                "{}:{}:{}",
                intent.run_id, intent.attempt_id, intent.command_id
            )
        && intent.prompt_run_binding_valid
        && intent.runner_command_binding_valid
        && intent.preview_only
        && intent.selected_target_id.is_none()
        && intent.authority == RunnerExecutionIntentAuthority::default()
}

fn valid_receipt(receipt: &RunnerTerminalReceiptObservation) -> bool {
    if !matches!(
        receipt.disposition_kind.as_str(),
        "completed" | "failed" | "uncertain"
    ) {
        return false;
    }
    let uncertain = receipt.disposition_kind == "uncertain";
    receipt.schema_version == RUNNER_TERMINAL_RECEIPT_SCHEMA_VERSION
        && receipt.evaluation_mode == RUNNER_TERMINAL_RECEIPT_EVALUATION_MODE
        && valid_identifier(&receipt.command_id)
        && valid_identifier(&receipt.attempt_id)
        && valid_identifier(&receipt.target_id)
        && valid_digest(&receipt.command_sha256)
        && receipt.observed_at_ms <= MAX_SAFE_INTEGER
        && receipt.receipt_valid
        && receipt.preview_only
        && receipt.uncertain == uncertain
        && receipt.reconciliation_required == uncertain
        && receipt.manual_review_required == uncertain
        && !receipt.automatic_retry
        && ((uncertain && receipt.follow_up == "reconciliation_manual")
            || (!uncertain && receipt.follow_up == "none"))
        && receipt.authority == RunnerTerminalReceiptAuthority::default()
}

fn valid_owner(owner: &RunnerExecutionOwner) -> bool {
    valid_text(&owner.issuer, MAX_OWNER_PART_BYTES)
        && valid_text(&owner.subject, MAX_OWNER_PART_BYTES)
        && valid_text(&owner.tenant_id, MAX_OWNER_PART_BYTES)
}

fn valid_identifier(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
        return false;
    }
    value.bytes().enumerate().all(|(index, byte)| {
        byte.is_ascii_alphanumeric()
            || (index > 0 && matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-'))
    })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && value
            .chars()
            .all(|character| !character.is_control() || character == '\t')
}

#[cfg(test)]
#[path = "session_runner_receipt_tests.rs"]
mod tests;
