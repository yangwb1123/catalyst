//! Pure restart-boundary reconciliation for Run, Attempt, lease, and terminal
//! receipt declarations.
//!
//! This module classifies observations only. It has no clock, store, Runner,
//! scheduler, retry, or transport effect; a live lease never becomes authority.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::lease::{LeaseGrant, LeaseState, TerminalDisposition, TerminalReceipt};

pub const EXECUTION_RECONCILIATION_SCHEMA_VERSION: &str =
    "forge.execution-reconciliation-observation/v1";
pub const EXECUTION_RECONCILIATION_EVALUATION_MODE: &str =
    "pure_execution_reconciliation_observation";
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_IDENTIFIER_BYTES: usize = 85;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationOwner {
    pub issuer: String,
    pub subject: String,
    pub tenant_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationInput {
    pub owner: ReconciliationOwner,
    pub conversation_id: String,
    pub run_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub target_id: String,
    pub run_status: String,
    pub attempt_state: String,
    pub lease: LeaseGrant,
    pub observed_at_ms: u64,
    pub terminal: Option<TerminalReceipt>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationAuthority {
    pub identity_verified: bool,
    pub run_authoritative: bool,
    pub attempt_persisted: bool,
    pub lease_issued: bool,
    pub terminal_persisted: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationObservation {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub owner: ReconciliationOwner,
    pub conversation_id: String,
    pub run_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub target_id: String,
    pub run_status: String,
    pub attempt_state: String,
    pub lease_epoch: u64,
    pub lease_active: bool,
    pub observed_at_ms: u64,
    pub terminal_observed: bool,
    pub terminal_disposition: String,
    pub terminal_state_aligned: bool,
    pub next_observation: String,
    pub reconciliation_required: bool,
    pub manual_review_required: bool,
    pub automatic_retry: bool,
    pub preview_only: bool,
    pub authority: ReconciliationAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationError;

impl fmt::Display for ReconciliationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid_execution_reconciliation_observation")
    }
}

impl std::error::Error for ReconciliationError {}

/// Classifies a caller-supplied restart snapshot without reading or writing
/// any durable state.
pub fn observe(
    input: ReconciliationInput,
) -> Result<ReconciliationObservation, ReconciliationError> {
    if !valid_owner(&input.owner)
        || !valid_identifier(&input.conversation_id)
        || !valid_identifier(&input.run_id)
        || !valid_identifier(&input.attempt_id)
        || !valid_identifier(&input.command_id)
        || !valid_identifier(&input.target_id)
        || !valid_run_status(&input.run_status)
        || !valid_attempt_state(&input.attempt_state)
        || input.observed_at_ms == 0
        || input.observed_at_ms > MAX_SAFE_INTEGER
        || input.lease.epoch == 0
        || input.lease.epoch > MAX_SAFE_INTEGER
        || input.lease.attempt_id != input.attempt_id
        || input.lease.target_id != input.target_id
        || input.lease.issued_at_ms > MAX_SAFE_INTEGER
        || input.lease.expires_at_ms > MAX_SAFE_INTEGER
        || input.observed_at_ms < input.lease.issued_at_ms
        || input.lease.validate().is_err()
    {
        return Err(ReconciliationError);
    }

    let mut terminal_disposition = "none".to_owned();
    let terminal_observed = input.terminal.is_some();
    let mut terminal_state_aligned = true;
    if let Some(terminal) = &input.terminal {
        let mut state = LeaseState::new(input.lease.clone()).map_err(|_| ReconciliationError)?;
        state
            .submit_terminal(
                terminal.proof.clone(),
                terminal.disposition.clone(),
                terminal.observed_at_ms,
            )
            .map_err(|_| ReconciliationError)?;
        if terminal.observed_at_ms > input.observed_at_ms {
            return Err(ReconciliationError);
        }
        terminal_disposition = disposition_name(&terminal.disposition).to_owned();
        terminal_state_aligned =
            terminal_state_matches_attempt(&terminal_disposition, &input.attempt_state);
    }

    let lease_active = input.lease.is_active(input.observed_at_ms);
    let next_observation = classify(
        &input.run_status,
        &input.attempt_state,
        lease_active,
        terminal_observed,
        &terminal_disposition,
        terminal_state_aligned,
    )
    .to_owned();
    let reconciliation_required = requires_reconciliation(&next_observation);
    Ok(ReconciliationObservation {
        schema_version: EXECUTION_RECONCILIATION_SCHEMA_VERSION.to_owned(),
        evaluation_mode: EXECUTION_RECONCILIATION_EVALUATION_MODE.to_owned(),
        owner: input.owner,
        conversation_id: input.conversation_id,
        run_id: input.run_id,
        attempt_id: input.attempt_id,
        command_id: input.command_id,
        target_id: input.target_id,
        run_status: input.run_status,
        attempt_state: input.attempt_state,
        lease_epoch: input.lease.epoch,
        lease_active,
        observed_at_ms: input.observed_at_ms,
        terminal_observed,
        terminal_disposition,
        terminal_state_aligned,
        next_observation,
        reconciliation_required,
        manual_review_required: reconciliation_required,
        automatic_retry: false,
        preview_only: true,
        authority: ReconciliationAuthority::default(),
    })
}

impl ReconciliationObservation {
    /// Validates the output shape and its classification invariants. It does
    /// not establish that the observation is current or durable.
    pub fn validate(&self) -> Result<(), ReconciliationError> {
        if self.schema_version != EXECUTION_RECONCILIATION_SCHEMA_VERSION
            || self.evaluation_mode != EXECUTION_RECONCILIATION_EVALUATION_MODE
            || !valid_owner(&self.owner)
            || !valid_identifier(&self.conversation_id)
            || !valid_identifier(&self.run_id)
            || !valid_identifier(&self.attempt_id)
            || !valid_identifier(&self.command_id)
            || !valid_identifier(&self.target_id)
            || !valid_run_status(&self.run_status)
            || !valid_attempt_state(&self.attempt_state)
            || self.lease_epoch == 0
            || self.lease_epoch > MAX_SAFE_INTEGER
            || self.observed_at_ms == 0
            || self.observed_at_ms > MAX_SAFE_INTEGER
            || !valid_terminal_disposition(&self.terminal_disposition)
            || !valid_next_observation(&self.next_observation)
            || !self.preview_only
            || self.automatic_retry
            || self.authority != ReconciliationAuthority::default()
            || self.terminal_observed != (self.terminal_disposition != "none")
            || self.reconciliation_required != requires_reconciliation(&self.next_observation)
            || self.manual_review_required != self.reconciliation_required
        {
            return Err(ReconciliationError);
        }
        if self.next_observation
            != classify(
                &self.run_status,
                &self.attempt_state,
                self.lease_active,
                self.terminal_observed,
                &self.terminal_disposition,
                self.terminal_state_aligned,
            )
            || (!self.terminal_observed && !self.terminal_state_aligned)
        {
            return Err(ReconciliationError);
        }
        if self.next_observation == "terminal_completed"
            && (self.terminal_disposition != "completed" || !self.terminal_state_aligned)
        {
            return Err(ReconciliationError);
        }
        if self.next_observation == "terminal_failed"
            && (self.terminal_disposition != "failed" || !self.terminal_state_aligned)
        {
            return Err(ReconciliationError);
        }
        if self.next_observation == "terminal_uncertain"
            && (self.terminal_disposition != "uncertain" || !self.terminal_state_aligned)
        {
            return Err(ReconciliationError);
        }
        if self.next_observation == "terminal_state_conflict"
            && (!self.terminal_observed || self.terminal_state_aligned)
        {
            return Err(ReconciliationError);
        }
        if self.terminal_observed
            && !self.terminal_state_aligned
            && self.next_observation != "terminal_state_conflict"
        {
            return Err(ReconciliationError);
        }
        Ok(())
    }
}

fn classify(
    run_status: &str,
    attempt_state: &str,
    lease_active: bool,
    terminal_observed: bool,
    terminal_disposition: &str,
    terminal_state_aligned: bool,
) -> &'static str {
    if terminal_observed {
        if !terminal_state_aligned {
            return "terminal_state_conflict";
        }
        return match terminal_disposition {
            "completed" => "terminal_completed",
            "failed" => "terminal_failed",
            _ => "terminal_uncertain",
        };
    }
    if run_status != "nonterminal" {
        return "run_terminal_without_receipt";
    }
    if matches!(attempt_state, "completed" | "failed" | "uncertain") {
        return "attempt_terminal_without_receipt";
    }
    if !matches!(attempt_state, "accepted" | "starting" | "running") {
        return "attempt_not_dispatchable";
    }
    if !lease_active {
        return "lease_expired_without_terminal";
    }
    "await_terminal"
}

fn requires_reconciliation(value: &str) -> bool {
    matches!(
        value,
        "lease_expired_without_terminal"
            | "attempt_not_dispatchable"
            | "attempt_terminal_without_receipt"
            | "run_terminal_without_receipt"
            | "terminal_uncertain"
            | "terminal_state_conflict"
    )
}

fn disposition_name(value: &TerminalDisposition) -> &'static str {
    match value {
        TerminalDisposition::Completed { .. } => "completed",
        TerminalDisposition::Failed { .. } => "failed",
        TerminalDisposition::Uncertain { .. } => "uncertain",
    }
}

fn terminal_state_matches_attempt(disposition: &str, state: &str) -> bool {
    matches!(
        (disposition, state),
        ("completed", "completed") | ("failed", "failed") | ("uncertain", "uncertain")
    )
}

fn valid_terminal_disposition(value: &str) -> bool {
    matches!(value, "none" | "completed" | "failed" | "uncertain")
}

fn valid_next_observation(value: &str) -> bool {
    matches!(
        value,
        "await_terminal"
            | "lease_expired_without_terminal"
            | "attempt_not_dispatchable"
            | "attempt_terminal_without_receipt"
            | "run_terminal_without_receipt"
            | "terminal_completed"
            | "terminal_failed"
            | "terminal_uncertain"
            | "terminal_state_conflict"
    )
}

fn valid_owner(value: &ReconciliationOwner) -> bool {
    valid_owner_part(&value.issuer, 2_048)
        && valid_owner_part(&value.subject, 255)
        && valid_owner_part(&value.tenant_id, 255)
}

fn valid_owner_part(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
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

fn valid_run_status(value: &str) -> bool {
    matches!(
        value,
        "nonterminal" | "completed" | "cancelled" | "limit_exceeded" | "failed"
    )
}

fn valid_attempt_state(value: &str) -> bool {
    matches!(
        value,
        "requested"
            | "accepted"
            | "starting"
            | "running"
            | "interrupted"
            | "completed"
            | "failed"
            | "uncertain"
    )
}
