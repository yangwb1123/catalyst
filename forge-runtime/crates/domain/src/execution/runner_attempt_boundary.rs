//! Strict, redacted Attempt lifecycle metadata before a future Runner effect.
//!
//! This receiver rechecks the Platform Core Attempt state graph and keeps the
//! two dispatch-start edges separate from terminal/preparatory observations.
//! It does not read or mutate an Attempt, lease, Runner transport, or Audit.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::platform_core_contract::{AttemptState, validate_attempt_transition};

use super::runner_execution_intent::RunnerExecutionOwner;

pub const RUNNER_ATTEMPT_BOUNDARY_SCHEMA_VERSION: &str = "forge.runner-attempt-boundary/v1";
pub const RUNNER_ATTEMPT_BOUNDARY_EVALUATION_MODE: &str =
    "attempt_lifecycle_dispatch_boundary_preview";

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The Attempt boundary wire contract records each authority claim as a separate boolean."
)]
pub struct RunnerAttemptBoundaryAuthority {
    pub attempt_persisted: bool,
    pub reservation_created: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "Wire compatibility requires distinct lifecycle, readiness, and preview predicates."
)]
pub struct RunnerAttemptBoundaryObservation {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub run_id: String,
    pub attempt_id: String,
    pub command_id: String,
    pub target_id: String,
    pub lease_epoch: u64,
    pub current_attempt_state: String,
    pub next_attempt_state: String,
    pub transition: String,
    pub execution_boundary_ready: bool,
    pub attempt_transition_valid: bool,
    pub attempt_transition_dispatchable: bool,
    pub attempt_boundary_ready: bool,
    pub rejection_reasons: Vec<String>,
    pub preview_only: bool,
    pub authority: RunnerAttemptBoundaryAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunnerAttemptBoundaryError {
    InvalidObservation,
}

impl fmt::Display for RunnerAttemptBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("invalid Runner Attempt boundary observation")
    }
}

impl std::error::Error for RunnerAttemptBoundaryError {}

/// Decodes one strict canonical observation. `serde_json` rejects unknown,
/// duplicate, malformed, and trailing JSON fields for this typed envelope.
/// Decoding checks shape only; use [`RunnerAttemptBoundaryObservation::validate`]
/// to check the derived lifecycle predicates.
///
/// # Errors
///
/// Returns a prefixed decode error for malformed JSON, unknown or duplicate
/// fields, missing required fields, type mismatches, or trailing input.
pub fn decode(bytes: &[u8]) -> Result<RunnerAttemptBoundaryObservation, String> {
    serde_json::from_slice(bytes)
        .map_err(|error| format!("invalid_runner_attempt_boundary: {error}"))
}

impl RunnerAttemptBoundaryObservation {
    /// Validates the derived lifecycle predicates without granting authority.
    ///
    /// # Errors
    ///
    /// Returns [`RunnerAttemptBoundaryError::InvalidObservation`] for invalid
    /// metadata, identities, epoch, state/transition names, inconsistent derived
    /// predicates or rejection reasons, or non-preview/authority claims.
    pub fn validate(&self) -> Result<(), RunnerAttemptBoundaryError> {
        if self.schema_version != RUNNER_ATTEMPT_BOUNDARY_SCHEMA_VERSION
            || self.evaluation_mode != RUNNER_ATTEMPT_BOUNDARY_EVALUATION_MODE
            || !valid_owner(&self.owner)
            || !valid_identifier(&self.conversation_id)
            || !valid_identifier(&self.run_id)
            || !valid_identifier(&self.attempt_id)
            || !valid_identifier(&self.command_id)
            || !valid_identifier(&self.target_id)
            || self.lease_epoch == 0
            || self.lease_epoch > MAX_SAFE_INTEGER
            || !valid_attempt_state(&self.current_attempt_state)
            || !valid_attempt_state(&self.next_attempt_state)
            || !valid_transition(&self.transition)
            || self.next_attempt_state != transition_target(&self.transition)
            || self.attempt_transition_valid
                != transition_valid(&self.current_attempt_state, &self.next_attempt_state)
            || self.attempt_transition_dispatchable
                != transition_dispatchable(&self.current_attempt_state, &self.next_attempt_state)
            || (self.execution_boundary_ready
                && !dispatchable_attempt_state(&self.current_attempt_state))
            || self.attempt_boundary_ready
                != (self.execution_boundary_ready && self.attempt_transition_dispatchable)
            || self.rejection_reasons
                != rejection_reasons(
                    self.execution_boundary_ready,
                    self.attempt_transition_valid,
                    self.attempt_transition_dispatchable,
                )
            || (self.attempt_boundary_ready && !self.rejection_reasons.is_empty())
            || !self.preview_only
            || self.authority != RunnerAttemptBoundaryAuthority::default()
        {
            return Err(RunnerAttemptBoundaryError::InvalidObservation);
        }
        Ok(())
    }
}

fn valid_owner(owner: &RunnerExecutionOwner) -> bool {
    [&owner.issuer, &owner.subject, &owner.tenant_id]
        .into_iter()
        .all(|value| valid_text(value, MAX_OWNER_PART_BYTES))
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

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.trim() == value
        && !value.chars().any(char::is_control)
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

fn valid_transition(value: &str) -> bool {
    matches!(
        value,
        "accept"
            | "begin_starting"
            | "observe_running"
            | "observe_interrupted"
            | "observe_completed"
            | "observe_failed"
            | "observe_effect_outcome_uncertain"
    )
}

fn transition_target(value: &str) -> &str {
    match value {
        "accept" => "accepted",
        "begin_starting" => "starting",
        "observe_running" => "running",
        "observe_interrupted" => "interrupted",
        "observe_completed" => "completed",
        "observe_failed" => "failed",
        "observe_effect_outcome_uncertain" => "uncertain",
        _ => "",
    }
}

fn transition_valid(current: &str, next: &str) -> bool {
    validate_attempt_transition(&attempt_state(current), &attempt_state(next)).is_ok()
}

fn transition_dispatchable(current: &str, next: &str) -> bool {
    (current == "accepted" && next == "starting") || (current == "starting" && next == "running")
}

fn dispatchable_attempt_state(value: &str) -> bool {
    matches!(value, "accepted" | "starting" | "running")
}

fn attempt_state(value: &str) -> AttemptState {
    match value {
        "requested" => AttemptState::Requested,
        "accepted" => AttemptState::Accepted,
        "starting" => AttemptState::Starting,
        "running" => AttemptState::Running,
        "interrupted" => AttemptState::Interrupted,
        "completed" => AttemptState::Completed,
        "failed" => AttemptState::Failed,
        "uncertain" => AttemptState::Uncertain,
        other => AttemptState::Unknown(other.to_owned()),
    }
}

fn rejection_reasons(
    boundary_ready: bool,
    transition_is_valid: bool,
    transition_is_dispatchable: bool,
) -> Vec<String> {
    let mut reasons = Vec::with_capacity(3);
    if !boundary_ready {
        reasons.push("execution_boundary_not_ready".to_owned());
    }
    if !transition_is_valid {
        reasons.push("attempt_transition_invalid".to_owned());
    } else if !transition_is_dispatchable {
        reasons.push("attempt_transition_not_dispatchable".to_owned());
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

#[cfg(test)]
#[path = "runner_attempt_boundary_tests.rs"]
mod tests;
