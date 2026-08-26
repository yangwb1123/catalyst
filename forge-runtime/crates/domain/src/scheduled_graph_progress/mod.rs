use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentGraphExecutionAttemptPolicy, GroupAgentGraphExecutionFailurePolicy,
    GroupAgentGraphExecutionMode, GroupAgentGraphExecutionProgressionPolicy,
    GroupAgentNodeTerminalOutcome, GroupAgentScheduledNodeLifecycleStatus, HubStoreError,
};

mod codec;
mod validation;

#[cfg(test)]
mod tests;

pub const SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_VERSION: u16 = 1;
pub const SCHEDULED_GRAPH_PROGRESS_PROTOCOL_VERSION: u16 = 1;
pub const SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_DIGEST_DOMAIN: &[u8] =
    b"forge.scheduled-graph-progress-snapshot.v1\0";
pub const MAX_SCHEDULED_GRAPH_PROGRESS_SNAPSHOT_BYTES: usize = 64 * 1024;
pub const SCHEDULED_GRAPH_RECONCILE_DECISION_VERSION: u16 = 1;
pub const SCHEDULED_GRAPH_RECONCILE_DECISION_DIGEST_DOMAIN: &[u8] =
    b"forge.scheduled-graph-reconcile-decision.v1\0";
pub const MAX_SCHEDULED_GRAPH_RECONCILE_DECISION_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledGraphProgressNode {
    pub execution_ordinal: usize,
    pub node_id: String,
    pub attempt: u16,
    pub candidate_id: Option<String>,
    pub candidate_sha256: Option<String>,
    pub provider_request_id: Option<String>,
    pub prepared_request_sha256: Option<String>,
    pub lifecycle_status: Option<GroupAgentScheduledNodeLifecycleStatus>,
    pub terminal_outcome: Option<GroupAgentNodeTerminalOutcome>,
    pub terminal_receipt_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledGraphProgressSnapshot {
    pub v: u16,
    pub progress_protocol_version: u16,
    pub graph_run_id: String,
    pub graph_id: String,
    pub schedule_id: String,
    pub schedule_sha256: String,
    pub node_count: usize,
    pub execution_mode: GroupAgentGraphExecutionMode,
    pub max_in_flight_nodes: usize,
    pub progression_policy: GroupAgentGraphExecutionProgressionPolicy,
    pub attempt_policy: GroupAgentGraphExecutionAttemptPolicy,
    pub failure_policy: GroupAgentGraphExecutionFailurePolicy,
    pub nodes: Vec<ScheduledGraphProgressNode>,
    pub snapshot_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledGraphReconcileDisposition {
    Ready,
    ClaimedUnknown,
    ManualRecoveryRequired,
    Failed,
    FailedUncertain,
    Completed,
    IncompatibleProgress,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledGraphReconcileDecision {
    pub v: u16,
    pub progress_protocol_version: u16,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub schedule_sha256: String,
    pub snapshot_sha256: String,
    pub disposition: ScheduledGraphReconcileDisposition,
    pub next_execution_ordinal: Option<usize>,
    pub next_node_id: Option<String>,
    pub decision_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledGraphProgressValidationError {
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledGraphReconcilePortError {
    Unavailable,
    InvalidDecision,
}

impl ScheduledGraphProgressSnapshot {
    /// Seals a newly assembled snapshot whose digest field is empty.
    ///
    /// # Errors
    ///
    /// Returns an error when already sealed or structurally invalid.
    pub fn seal(mut self) -> Result<Self, ScheduledGraphProgressValidationError> {
        if !self.snapshot_sha256.is_empty() {
            return Err(validation::invalid("progress snapshot is already sealed"));
        }
        self.snapshot_sha256 = self.expected_sha256()?;
        self.validate()?;
        Ok(self)
    }

    /// Strictly decodes exact compact canonical snapshot JSON.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact(json: &str) -> Result<Self, ScheduledGraphProgressValidationError> {
        Self::decode_exact_bytes(json.as_bytes())
    }

    /// Strictly decodes exact compact canonical snapshot bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact_bytes(bytes: &[u8]) -> Result<Self, ScheduledGraphProgressValidationError> {
        codec::decode_snapshot_exact(bytes)
    }

    /// Validates fixed policy, identities, evidence shape, size, and digest.
    ///
    /// # Errors
    ///
    /// Returns an error when any snapshot invariant disagrees.
    pub fn validate(&self) -> Result<(), ScheduledGraphProgressValidationError> {
        validation::validate_snapshot(self)
    }

    /// Encodes the complete snapshot as exact compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_json(&self) -> Result<String, ScheduledGraphProgressValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated payload digest without `snapshot_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical payload encoding fails.
    pub fn expected_sha256(&self) -> Result<String, ScheduledGraphProgressValidationError> {
        codec::snapshot_digest(self)
    }
}

impl ScheduledGraphReconcileDecision {
    /// Seals a newly assembled decision whose digest field is empty.
    ///
    /// # Errors
    ///
    /// Returns an error when already sealed or structurally invalid.
    pub fn seal(mut self) -> Result<Self, ScheduledGraphProgressValidationError> {
        if !self.decision_sha256.is_empty() {
            return Err(validation::invalid("reconcile decision is already sealed"));
        }
        self.decision_sha256 = self.expected_sha256()?;
        self.validate()?;
        Ok(self)
    }

    /// Strictly decodes exact compact canonical decision JSON.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact(json: &str) -> Result<Self, ScheduledGraphProgressValidationError> {
        Self::decode_exact_bytes(json.as_bytes())
    }

    /// Strictly decodes exact compact canonical decision bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact_bytes(bytes: &[u8]) -> Result<Self, ScheduledGraphProgressValidationError> {
        codec::decode_decision_exact(bytes)
    }

    /// Validates identity, disposition shape, size, and digest.
    ///
    /// # Errors
    ///
    /// Returns an error when any decision invariant disagrees.
    pub fn validate(&self) -> Result<(), ScheduledGraphProgressValidationError> {
        validation::validate_decision(self)
    }

    /// Validates decision source and optional next-node bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid source, decision, or binding.
    pub fn validate_against_snapshot(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<(), ScheduledGraphProgressValidationError> {
        validation::validate_decision_against_snapshot(self, snapshot)
    }

    /// Encodes the complete decision as exact compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_json(&self) -> Result<String, ScheduledGraphProgressValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated payload digest without `decision_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical payload encoding fails.
    pub fn expected_sha256(&self) -> Result<String, ScheduledGraphProgressValidationError> {
        codec::decision_digest(self)
    }
}

pub trait ScheduledGraphProgressStore: Send + Sync {
    /// Loads one content-free progress view from one atomic durable snapshot.
    ///
    /// # Errors
    ///
    /// Returns a structured missing, unavailable, conflict, or corruption error.
    fn snapshot_scheduled_graph_progress(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphProgressSnapshot, HubStoreError>;
}

pub trait ScheduledGraphReconcilePort: Send + Sync {
    /// Asks the pinned Core for a pure decision over one exact snapshot.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when Core cannot produce an exact decision.
    fn decide(
        &self,
        snapshot: &ScheduledGraphProgressSnapshot,
    ) -> Result<ScheduledGraphReconcileDecision, ScheduledGraphReconcilePortError>;
}
