use std::sync::Arc;

use thiserror::Error;

use crate::runtime_domain::{
    HubStoreError, MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES, ScheduledGraphProgressSnapshot,
    ScheduledGraphProgressStore, ScheduledGraphReconcileDecision, ScheduledGraphReconcilePort,
    ScheduledGraphReconcilePortError,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ScheduledGraphReconcileServiceError {
    #[error("scheduled Graph reconcile input is invalid")]
    InvalidInput,
    #[error("scheduled Graph progress was not found")]
    NotFound,
    #[error("scheduled Graph progress storage is unavailable")]
    StorageUnavailable,
    #[error("scheduled Graph durable progress is corrupt")]
    CorruptProgress,
    #[error("scheduled Graph Core reconcile is unavailable")]
    CoreUnavailable,
    #[error("scheduled Graph Core reconcile decision is invalid")]
    InvalidCoreDecision,
}

pub struct ScheduledGraphReconcileService {
    progress: Arc<dyn ScheduledGraphProgressStore>,
    core: Arc<dyn ScheduledGraphReconcilePort>,
}

impl ScheduledGraphReconcileService {
    #[must_use]
    pub fn new(
        progress: Arc<dyn ScheduledGraphProgressStore>,
        core: Arc<dyn ScheduledGraphReconcilePort>,
    ) -> Self {
        Self { progress, core }
    }

    /// Loads one atomic progress snapshot and asks the pinned Core for a pure decision.
    ///
    /// This service does not persist, claim, dispatch, retry, or release authority.
    ///
    /// # Errors
    ///
    /// Returns a redacted input, storage, corruption, Core, or decision error.
    pub fn reconcile(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcileServiceError> {
        validate_identifier(graph_run_id)?;
        let snapshot = self
            .progress
            .snapshot_scheduled_graph_progress(graph_run_id)
            .map_err(|error| map_store_error(&error))?;
        validate_snapshot_source(&snapshot, graph_run_id)?;
        let decision = self.core.decide(&snapshot).map_err(map_core_error)?;
        decision
            .validate_against_snapshot(&snapshot)
            .map_err(|_| ScheduledGraphReconcileServiceError::InvalidCoreDecision)?;
        Ok(decision)
    }
}

fn map_core_error(error: ScheduledGraphReconcilePortError) -> ScheduledGraphReconcileServiceError {
    match error {
        ScheduledGraphReconcilePortError::Unavailable => {
            ScheduledGraphReconcileServiceError::CoreUnavailable
        }
        ScheduledGraphReconcilePortError::InvalidDecision => {
            ScheduledGraphReconcileServiceError::InvalidCoreDecision
        }
    }
}

fn validate_snapshot_source(
    snapshot: &ScheduledGraphProgressSnapshot,
    graph_run_id: &str,
) -> Result<(), ScheduledGraphReconcileServiceError> {
    snapshot
        .validate()
        .map_err(|_| ScheduledGraphReconcileServiceError::CorruptProgress)?;
    (snapshot.graph_run_id == graph_run_id)
        .then_some(())
        .ok_or(ScheduledGraphReconcileServiceError::CorruptProgress)
}

fn validate_identifier(value: &str) -> Result<(), ScheduledGraphReconcileServiceError> {
    let valid = !value.trim().is_empty()
        && value.len() <= MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES
        && !value.chars().any(unsupported_character);
    valid
        .then_some(())
        .ok_or(ScheduledGraphReconcileServiceError::InvalidInput)
}

fn unsupported_character(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

fn map_store_error(error: &HubStoreError) -> ScheduledGraphReconcileServiceError {
    match error {
        HubStoreError::NotFound { .. } => ScheduledGraphReconcileServiceError::NotFound,
        HubStoreError::Unavailable { .. } => {
            ScheduledGraphReconcileServiceError::StorageUnavailable
        }
        HubStoreError::Conflict { .. } | HubStoreError::Corrupt { .. } => {
            ScheduledGraphReconcileServiceError::CorruptProgress
        }
    }
}
