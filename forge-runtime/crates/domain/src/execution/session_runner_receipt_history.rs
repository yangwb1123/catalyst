//! Pure reduction of terminal receipt observations for one owner/session Run.
//!
//! The history keeps only bounded, content-free observations. Definite failed
//! attempts may be followed by another caller-supplied attempt; completed and
//! uncertain outcomes close the value. No retry, persistence, lease, clock, or
//! Runner effect is performed here.

use std::{collections::HashSet, fmt};

use serde::{Deserialize, Serialize};

use super::runner_execution_intent::RunnerExecutionOwner;
use super::session_runner_receipt::SessionRunnerReceiptObservation;

pub const SESSION_RUNNER_RECEIPT_HISTORY_SCHEMA_VERSION: &str =
    "forge.session-runner-receipt-history/v1";
pub const SESSION_RUNNER_RECEIPT_HISTORY_EVALUATION_MODE: &str =
    "pure_session_runner_receipt_history_only";
pub const MAX_SESSION_RUNNER_RECEIPTS: usize = 16;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The frozen wire carries independent authority disclaimers."
)]
pub struct SessionRunnerReceiptHistoryAuthority {
    pub identity_verified: bool,
    pub receipt_persisted: bool,
    pub execution_authorized: bool,
    pub dispatch_performed: bool,
    pub audit_published: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRunnerReceiptHistoryRequest {
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub receipts: Vec<SessionRunnerReceiptObservation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "The frozen wire carries independent observation and authority flags."
)]
pub struct SessionRunnerReceiptHistoryObservation {
    pub schema_version: String,
    pub evaluation_mode: String,
    pub owner: RunnerExecutionOwner,
    pub conversation_id: String,
    pub prompt_id: String,
    pub run_id: String,
    pub receipts: Vec<SessionRunnerReceiptObservation>,
    pub attempt_count: u32,
    pub latest_attempt_id: String,
    pub latest_command_id: String,
    pub latest_target_id: String,
    pub latest_disposition_kind: String,
    pub latest_observed_at_ms: u64,
    pub reconciliation_required: bool,
    pub manual_review_required: bool,
    pub automatic_retry: bool,
    pub follow_up: String,
    pub selected_target_id: Option<String>,
    pub preview_only: bool,
    pub authority: SessionRunnerReceiptHistoryAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRunnerReceiptHistoryError {
    InvalidHistory,
}

impl fmt::Display for SessionRunnerReceiptHistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("session Runner receipt history is invalid")
    }
}

impl std::error::Error for SessionRunnerReceiptHistoryError {}

/// Reduces a bounded receipt history without adding execution or persistence authority.
///
/// # Errors
/// Returns `SessionRunnerReceiptHistoryError::InvalidHistory` for an empty or
/// oversized history, invalid receipt bindings, duplicate attempts, unordered
/// receipts, or an attempt following a completed or uncertain receipt.
pub fn observe_session_runner_receipt_history(
    input: SessionRunnerReceiptHistoryRequest,
) -> Result<SessionRunnerReceiptHistoryObservation, SessionRunnerReceiptHistoryError> {
    if !valid_history_input(&input) {
        return Err(SessionRunnerReceiptHistoryError::InvalidHistory);
    }
    let receipts = input.receipts;
    // Validated histories contain at most sixteen receipts, so this is exact.
    let attempt_count = u32::try_from(receipts.len())
        .map_err(|_| SessionRunnerReceiptHistoryError::InvalidHistory)?;
    let latest = receipts
        .last()
        .ok_or(SessionRunnerReceiptHistoryError::InvalidHistory)?
        .receipt_observation
        .clone();
    let uncertain = latest.uncertain;
    Ok(SessionRunnerReceiptHistoryObservation {
        schema_version: SESSION_RUNNER_RECEIPT_HISTORY_SCHEMA_VERSION.into(),
        evaluation_mode: SESSION_RUNNER_RECEIPT_HISTORY_EVALUATION_MODE.into(),
        owner: input.owner,
        conversation_id: input.conversation_id,
        prompt_id: input.prompt_id,
        run_id: input.run_id,
        attempt_count,
        receipts,
        latest_attempt_id: latest.attempt_id,
        latest_command_id: latest.command_id,
        latest_target_id: latest.target_id,
        latest_disposition_kind: latest.disposition_kind,
        latest_observed_at_ms: latest.observed_at_ms,
        reconciliation_required: uncertain,
        manual_review_required: uncertain,
        automatic_retry: false,
        follow_up: if uncertain {
            "reconciliation_manual".into()
        } else {
            "none".into()
        },
        selected_target_id: None,
        preview_only: true,
        authority: SessionRunnerReceiptHistoryAuthority::default(),
    })
}

impl SessionRunnerReceiptHistoryObservation {
    /// Checks the history and recomputes its derived summary and boundary flags.
    ///
    /// # Errors
    /// Returns `SessionRunnerReceiptHistoryError::InvalidHistory` when the wire
    /// identifiers, receipt history, or any derived field violate the contract.
    pub fn validate(&self) -> Result<(), SessionRunnerReceiptHistoryError> {
        if self.schema_version != SESSION_RUNNER_RECEIPT_HISTORY_SCHEMA_VERSION
            || self.evaluation_mode != SESSION_RUNNER_RECEIPT_HISTORY_EVALUATION_MODE
        {
            return Err(SessionRunnerReceiptHistoryError::InvalidHistory);
        }
        let derived = observe_session_runner_receipt_history(SessionRunnerReceiptHistoryRequest {
            owner: self.owner.clone(),
            conversation_id: self.conversation_id.clone(),
            prompt_id: self.prompt_id.clone(),
            run_id: self.run_id.clone(),
            receipts: self.receipts.clone(),
        })?;
        if self != &derived {
            return Err(SessionRunnerReceiptHistoryError::InvalidHistory);
        }
        Ok(())
    }
}

fn valid_history_input(input: &SessionRunnerReceiptHistoryRequest) -> bool {
    if input.receipts.is_empty() || input.receipts.len() > MAX_SESSION_RUNNER_RECEIPTS {
        return false;
    }
    let mut attempts = HashSet::with_capacity(input.receipts.len());
    for (index, receipt) in input.receipts.iter().enumerate() {
        if receipt.validate().is_err()
            || receipt.owner != input.owner
            || receipt.conversation_id != input.conversation_id
            || receipt.prompt_id != input.prompt_id
            || receipt.run_id != input.run_id
            || !attempts.insert(receipt.receipt_observation.attempt_id.clone())
        {
            return false;
        }
        if index > 0 {
            let previous = &input.receipts[index - 1];
            if receipt_after(previous, receipt)
                || matches!(
                    previous.receipt_observation.disposition_kind.as_str(),
                    "completed" | "uncertain"
                )
            {
                return false;
            }
        }
    }
    true
}

fn receipt_after(
    left: &SessionRunnerReceiptObservation,
    right: &SessionRunnerReceiptObservation,
) -> bool {
    let left_time = left.receipt_observation.observed_at_ms;
    let right_time = right.receipt_observation.observed_at_ms;
    left_time > right_time
        || (left_time == right_time
            && left.receipt_observation.attempt_id >= right.receipt_observation.attempt_id)
}

#[cfg(test)]
#[path = "session_runner_receipt_history_tests.rs"]
mod tests;

#[cfg(test)]
mod count_boundaries {
    use super::*;

    #[test]
    fn valid_history_counts_are_exact_at_both_boundaries() {
        for count in [1, 16] {
            let observation = observe_session_runner_receipt_history(request(count)).unwrap();
            assert_eq!(observation.attempt_count, count);
            assert!(observation.validate().is_ok());
        }
    }

    #[test]
    fn empty_and_oversized_histories_return_the_existing_error() {
        for count in [0, 17] {
            assert_eq!(
                observe_session_runner_receipt_history(request(count)),
                Err(SessionRunnerReceiptHistoryError::InvalidHistory)
            );
        }
    }

    fn request(count: u32) -> SessionRunnerReceiptHistoryRequest {
        let history: SessionRunnerReceiptHistoryObservation = serde_json::from_str(include_str!(
            "../../../../../docs/contracts/fixtures/forge-session-runner-receipt-history-v1.json"
        ))
        .unwrap();
        let receipts = (1..=count)
            .map(|attempt| {
                let mut receipt = history.receipts[0].clone();
                receipt.receipt_observation.attempt_id = format!("attempt-{attempt:03}");
                receipt.receipt_observation.observed_at_ms = u64::from(attempt);
                receipt
            })
            .collect();
        SessionRunnerReceiptHistoryRequest {
            owner: history.owner,
            conversation_id: history.conversation_id,
            prompt_id: history.prompt_id,
            run_id: history.run_id,
            receipts,
        }
    }
}
