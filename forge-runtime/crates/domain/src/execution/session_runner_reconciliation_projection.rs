//! Pure manual-reconciliation projection for an uncertain session receipt history.
//!
//! The value is derived entirely from a validated history observation. It does
//! not persist a receipt, retry work, choose a target, dispatch, execute, or
//! publish Audit evidence.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::{
    runner_execution_intent::RunnerExecutionOwner,
    session_runner_receipt_history::{
        MAX_SESSION_RUNNER_RECEIPTS, SESSION_RUNNER_RECEIPT_HISTORY_SCHEMA_VERSION,
        SessionRunnerReceiptHistoryObservation,
    },
};

pub const SESSION_RUNNER_RECONCILIATION_PROJECTION_SCHEMA_VERSION: &str =
    "forge.session-runner-reconciliation-projection/v1";
pub const SESSION_RUNNER_RECONCILIATION_PROJECTION_EVALUATION_MODE: &str =
    "pure_session_runner_reconciliation_projection_only";

const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_OWNER_PART_BYTES: usize = 512;
const MAX_IDENTIFIER_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRunnerReconciliationAuthority {
    pub identity_verified: bool,
    pub receipt_persisted: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRunnerReceiptHistorySummary {
    pub schema_version: String,
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub attempt_count: u32,
    pub latest_attempt_id: String,
    pub latest_command_id: String,
    pub latest_target_id: String,
    pub latest_disposition_kind: String,
    pub latest_observed_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionRunnerReconciliationProjection {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub source: SessionRunnerReceiptHistorySummary,
    pub latest_attempt_id: String,
    pub latest_command_id: String,
    pub latest_target_id: String,
    pub latest_disposition_kind: String,
    pub latest_observed_at_ms: u64,
    pub reconciliation_kind: String,
    pub reconciliation_reason: String,
    pub reconciliation_required: bool,
    pub manual_review_required: bool,
    pub automatic_retry: bool,
    pub follow_up: String,
    pub selected_target_id: Option<String>,
    pub preview_only: bool,
    pub authority: SessionRunnerReconciliationAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRunnerReconciliationProjectionError {
    InvalidProjection,
    HistoryIsNotUncertain,
}

impl fmt::Display for SessionRunnerReconciliationProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("session Runner reconciliation projection is invalid")
    }
}

impl std::error::Error for SessionRunnerReconciliationProjectionError {}

/// Projects the manual-reconciliation view of one validated uncertain history.
pub fn project_session_runner_reconciliation(
    history: &SessionRunnerReceiptHistoryObservation,
) -> Result<SessionRunnerReconciliationProjection, SessionRunnerReconciliationProjectionError> {
    history
        .validate()
        .map_err(|_| SessionRunnerReconciliationProjectionError::InvalidProjection)?;
    if history.latest_disposition_kind != "uncertain"
        || !history.reconciliation_required
        || !history.manual_review_required
        || history.automatic_retry
        || history.follow_up != "reconciliation_manual"
    {
        return Err(SessionRunnerReconciliationProjectionError::HistoryIsNotUncertain);
    }
    Ok(expected_projection(history_summary(history)))
}

impl SessionRunnerReconciliationProjection {
    /// Recomputes every derived field from the embedded history summary.
    pub fn validate(&self) -> Result<(), SessionRunnerReconciliationProjectionError> {
        validate_summary(&self.source)?;
        if self != &expected_projection(self.source.clone()) {
            return Err(SessionRunnerReconciliationProjectionError::InvalidProjection);
        }
        Ok(())
    }
}

fn history_summary(
    history: &SessionRunnerReceiptHistoryObservation,
) -> SessionRunnerReceiptHistorySummary {
    SessionRunnerReceiptHistorySummary {
        schema_version: SESSION_RUNNER_RECEIPT_HISTORY_SCHEMA_VERSION.into(),
        owner: history.owner.clone(),
        conversation_id: history.conversation_id.clone(),
        prompt_id: history.prompt_id.clone(),
        run_id: history.run_id.clone(),
        attempt_count: history.attempt_count,
        latest_attempt_id: history.latest_attempt_id.clone(),
        latest_command_id: history.latest_command_id.clone(),
        latest_target_id: history.latest_target_id.clone(),
        latest_disposition_kind: history.latest_disposition_kind.clone(),
        latest_observed_at_ms: history.latest_observed_at_ms,
    }
}

fn expected_projection(
    source: SessionRunnerReceiptHistorySummary,
) -> SessionRunnerReconciliationProjection {
    SessionRunnerReconciliationProjection {
        schema_version: SESSION_RUNNER_RECONCILIATION_PROJECTION_SCHEMA_VERSION.into(),
        evaluation_mode: SESSION_RUNNER_RECONCILIATION_PROJECTION_EVALUATION_MODE.into(),
        owner: source.owner.clone(),
        conversation_id: source.conversation_id.clone(),
        prompt_id: source.prompt_id.clone(),
        run_id: source.run_id.clone(),
        latest_attempt_id: source.latest_attempt_id.clone(),
        latest_command_id: source.latest_command_id.clone(),
        latest_target_id: source.latest_target_id.clone(),
        latest_disposition_kind: source.latest_disposition_kind.clone(),
        latest_observed_at_ms: source.latest_observed_at_ms,
        source,
        reconciliation_kind: "manual".into(),
        reconciliation_reason: "uncertain_terminal_receipt".into(),
        reconciliation_required: true,
        manual_review_required: true,
        automatic_retry: false,
        follow_up: "reconciliation_manual".into(),
        selected_target_id: None,
        preview_only: true,
        authority: SessionRunnerReconciliationAuthority::default(),
    }
}

fn validate_summary(
    summary: &SessionRunnerReceiptHistorySummary,
) -> Result<(), SessionRunnerReconciliationProjectionError> {
    let valid = summary.schema_version == SESSION_RUNNER_RECEIPT_HISTORY_SCHEMA_VERSION
        && valid_owner(&summary.owner)
        && valid_identifier(&summary.conversation_id)
        && valid_identifier(&summary.prompt_id)
        && valid_identifier(&summary.run_id)
        && (1..=MAX_SESSION_RUNNER_RECEIPTS as u32).contains(&summary.attempt_count)
        && valid_identifier(&summary.latest_attempt_id)
        && valid_identifier(&summary.latest_command_id)
        && valid_identifier(&summary.latest_target_id)
        && summary.latest_disposition_kind == "uncertain"
        && summary.latest_observed_at_ms <= MAX_SAFE_INTEGER;
    if !valid {
        return Err(SessionRunnerReconciliationProjectionError::InvalidProjection);
    }
    Ok(())
}

fn valid_owner(owner: &RunnerExecutionOwner) -> bool {
    [&owner.issuer, &owner.subject, &owner.tenant_id]
        .into_iter()
        .all(|value| valid_text(value, MAX_OWNER_PART_BYTES))
}

fn valid_identifier(value: &str) -> bool {
    valid_text(value, MAX_IDENTIFIER_BYTES) && !value.contains('/')
}

fn valid_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
#[path = "session_runner_reconciliation_projection_tests.rs"]
mod tests;
