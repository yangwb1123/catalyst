//! Pure binding of a Prompt/Run reference to a future Runner command.
//!
//! This module validates repeated identities before a command reaches an
//! execution adapter. It does not select a target, verify a device, issue a
//! lease, persist a command, reserve capacity, dispatch, or execute.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::runner_command::{RunnerCommand, RunnerCommandError};

pub const RUNNER_EXECUTION_INTENT_SCHEMA_VERSION: &str = "forge.runner-execution-intent/v1";
pub const RUNNER_EXECUTION_INTENT_EVALUATION_MODE: &str = "pure_runner_binding_only";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerExecutionOwner {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerExecutionPromptReceipt {
    pub prompt_id: String,
    pub conversation_id: String,
    pub role: String,
    pub accepted_at_ms: u64,
    pub intent_id: String,
    pub initial_event_id: String,
    pub initial_event_sequence: u64,
    pub initial_event_type: String,
    pub replayed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerExecutionRunReference {
    pub run_id: String,
    pub conversation_id: String,
    pub prompt_id: String,
    pub created_at_ms: u64,
    pub latest_sequence: u64,
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerExecutionIntentBinding {
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub target_id: String,
    pub command_sha256: String,
    pub idempotency_key: String,
    pub selected_target_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerExecutionIntentRequest {
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_receipt: RunnerExecutionPromptReceipt,
    pub run_reference: RunnerExecutionRunReference,
    pub execution_intent: RunnerExecutionIntentBinding,
    pub command: RunnerCommand,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The execution-intent wire contract records each authority claim as a separate boolean."
)]
pub struct RunnerExecutionIntentAuthority {
    pub device_identity_verified: bool,
    pub command_persisted: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RunnerExecutionIntentObservation {
    pub schema_version: &'static str,
    pub evaluation_mode: &'static str,
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub target_id: String,
    pub command_sha256: String,
    pub idempotency_key: String,
    pub prompt_run_binding_valid: bool,
    pub runner_command_binding_valid: bool,
    pub preview_only: bool,
    pub selected_target_id: Option<String>,
    pub authority: RunnerExecutionIntentAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunnerExecutionIntentError {
    InvalidOwner,
    InvalidPrompt,
    InvalidRun,
    InvalidBinding,
    Command(RunnerCommandError),
}

impl fmt::Display for RunnerExecutionIntentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Runner execution intent is invalid: {self:?}")
    }
}

impl std::error::Error for RunnerExecutionIntentError {}

/// Validates one repeated Conversation/Prompt/Run/command identity tuple.
///
/// The returned value is metadata-only and always preview-only. It has no
/// clock, storage, transport, selection, reservation, dispatch, or process
/// effect.
///
/// # Errors
///
/// Returns an owner, Prompt, Run, or binding error for invalid supplied metadata
/// or inconsistent identities, digests, keys, and selection fields. Command
/// validation and encoding failures are wrapped in [`RunnerExecutionIntentError::Command`].
pub fn observe_runner_execution_intent(
    input: RunnerExecutionIntentRequest,
) -> Result<RunnerExecutionIntentObservation, RunnerExecutionIntentError> {
    validate_owner(&input.owner)?;
    validate_prompt(&input.prompt_receipt, &input.conversation_id)?;
    validate_run(
        &input.run_reference,
        &input.prompt_receipt,
        &input.conversation_id,
    )?;
    validate_binding(&input)?;
    Ok(RunnerExecutionIntentObservation {
        schema_version: RUNNER_EXECUTION_INTENT_SCHEMA_VERSION,
        evaluation_mode: RUNNER_EXECUTION_INTENT_EVALUATION_MODE,
        owner: input.owner,
        conversation_id: input.conversation_id,
        prompt_id: input.prompt_receipt.prompt_id,
        run_id: input.run_reference.run_id,
        attempt_id: input.execution_intent.attempt_id,
        command_id: input.execution_intent.command_id,
        target_id: input.execution_intent.target_id,
        command_sha256: input.execution_intent.command_sha256,
        idempotency_key: input.execution_intent.idempotency_key,
        prompt_run_binding_valid: true,
        runner_command_binding_valid: true,
        preview_only: true,
        selected_target_id: None,
        authority: RunnerExecutionIntentAuthority::default(),
    })
}

fn validate_owner(owner: &RunnerExecutionOwner) -> Result<(), RunnerExecutionIntentError> {
    if !valid_owner_part(&owner.issuer)
        || !valid_owner_part(&owner.subject)
        || !valid_owner_part(&owner.tenant_id)
    {
        return Err(RunnerExecutionIntentError::InvalidOwner);
    }
    Ok(())
}

fn validate_prompt(
    prompt: &RunnerExecutionPromptReceipt,
    conversation_id: &str,
) -> Result<(), RunnerExecutionIntentError> {
    if !valid_identifier(&prompt.prompt_id)
        || prompt.conversation_id != conversation_id
        || prompt.role != "user"
        || prompt.accepted_at_ms > MAX_SAFE_INTEGER
        || !valid_identifier(&prompt.intent_id)
        || !valid_identifier(&prompt.initial_event_id)
        || prompt.initial_event_sequence != 1
        || prompt.initial_event_type != "submitted"
    {
        return Err(RunnerExecutionIntentError::InvalidPrompt);
    }
    Ok(())
}

fn validate_run(
    run: &RunnerExecutionRunReference,
    prompt: &RunnerExecutionPromptReceipt,
    conversation_id: &str,
) -> Result<(), RunnerExecutionIntentError> {
    if !valid_identifier(&run.run_id)
        || run.conversation_id != conversation_id
        || run.prompt_id != prompt.prompt_id
        || run.created_at_ms < prompt.accepted_at_ms
        || run.created_at_ms > MAX_SAFE_INTEGER
        || run.latest_sequence == 0
        || run.latest_sequence > MAX_SAFE_INTEGER
        || !matches!(
            run.status.as_str(),
            "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
        )
    {
        return Err(RunnerExecutionIntentError::InvalidRun);
    }
    Ok(())
}

fn validate_binding(
    input: &RunnerExecutionIntentRequest,
) -> Result<(), RunnerExecutionIntentError> {
    let binding = &input.execution_intent;
    if binding.conversation_id != input.conversation_id
        || binding.prompt_id != input.prompt_receipt.prompt_id
        || binding.run_id != input.run_reference.run_id
        || binding.selected_target_id.is_some()
        || !valid_identifier(&binding.attempt_id)
        || !valid_identifier(&binding.command_id)
        || !valid_identifier(&binding.target_id)
        || !valid_digest(&binding.command_sha256)
        || binding.idempotency_key
            != format!(
                "{}:{}:{}",
                input.run_reference.run_id, binding.attempt_id, binding.command_id
            )
    {
        return Err(RunnerExecutionIntentError::InvalidBinding);
    }
    input
        .command
        .validate()
        .map_err(RunnerExecutionIntentError::Command)?;
    if input.command.command_id != binding.command_id
        || input.command.lease_proof.attempt_id != binding.attempt_id
        || input.command.lease_proof.target_id != binding.target_id
        || input.command.idempotency_key != binding.idempotency_key
        || input
            .command
            .command_sha256()
            .map_err(RunnerExecutionIntentError::Command)?
            != binding.command_sha256
    {
        return Err(RunnerExecutionIntentError::InvalidBinding);
    }
    Ok(())
}

fn valid_owner_part(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_OWNER_PART_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn valid_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_IDENTIFIER_BYTES
        && bytes[0].is_ascii_alphanumeric()
        && bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'/' | b'-')
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
#[path = "runner_execution_intent_tests.rs"]
mod tests;
