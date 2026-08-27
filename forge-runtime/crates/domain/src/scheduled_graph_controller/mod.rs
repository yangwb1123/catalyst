#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentGraphControlSnapshot, GroupAgentScheduledNodeContractCandidate,
    GroupAgentScheduledNodeTerminalReceipt, HubStoreError,
};

mod codec;
mod lineage;
mod transitions;
mod validation;

#[cfg(test)]
mod tests;

pub const SCHEDULED_GRAPH_CONTROLLER_VERSION: u16 = 1;
pub const SCHEDULED_GRAPH_CONTROLLER_PROTOCOL_VERSION: u16 = 1;
pub const SCHEDULED_GRAPH_CONTROLLER_PROFILE_DIGEST_DOMAIN: &[u8] =
    b"forge.scheduled-graph-controller-profile.v1\0";
pub const SCHEDULED_GRAPH_CONTROLLER_HEADER_DIGEST_DOMAIN: &[u8] =
    b"forge.scheduled-graph-controller-header.v1\0";
pub const SCHEDULED_GRAPH_CONTROLLER_EVENT_DIGEST_DOMAIN: &[u8] =
    b"forge.scheduled-graph-controller-event.v1\0";
pub const MAX_SCHEDULED_GRAPH_CONTROLLER_EVENTS: usize = 512;
pub const MAX_SCHEDULED_GRAPH_CONTROLLER_EVENT_BYTES: usize = 64 * 1024;
pub const MAX_SCHEDULED_GRAPH_CONTROLLER_HEADER_BYTES: usize = 64 * 1024;
pub const MAX_SCHEDULED_GRAPH_CONTROLLER_JOURNAL_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SCHEDULED_GRAPH_CONTROLLER_EFFECTFUL_STEPS: u16 = 32;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledGraphControllerExecutionProfile {
    pub endpoint: String,
    pub model: String,
    pub max_output_tokens: u64,
    pub max_model_output_bytes: u64,
    pub max_model_events: u64,
    pub timeout_ms: u64,
    pub max_cost_usd_micros: u64,
    pub pricing_snapshot_sha256: String,
    pub max_result_bytes: u64,
    pub profile_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledGraphControllerHeader {
    pub v: u16,
    pub controller_protocol_version: u16,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub schedule_sha256: String,
    pub schedule_version: u16,
    pub progress_protocol_version: u16,
    pub core_bin_sha256: String,
    pub node_count: usize,
    pub max_effectful_steps: u16,
    pub max_total_cost_usd_micros: u64,
    pub execution_profile: ScheduledGraphControllerExecutionProfile,
    pub created_at_ms: u64,
    pub controller_id: String,
    pub controller_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledGraphControllerRetryableFailure {
    CredentialUnavailable,
    ProviderUnavailable,
    OwnerEvidenceUnavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduledGraphControllerStopReason {
    ClaimedUnknown,
    Quarantined,
    Adjudicated,
    Failed,
    FailedUncertain,
    IncompatibleProgress,
    IncompatibleSchedule,
    BudgetExhausted,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ScheduledGraphControllerEventPayload {
    Started {
        snapshot_sha256: String,
        decision_sha256: String,
    },
    MaterializePlanned {
        execution_ordinal: usize,
        node_id: String,
        snapshot_sha256: String,
        decision_sha256: String,
        idempotency_key: String,
    },
    MaterializeObserved {
        execution_ordinal: usize,
        node_id: String,
        contract_id: String,
    },
    PreparePlanned {
        execution_ordinal: usize,
        node_id: String,
        contract_id: String,
        idempotency_key: String,
    },
    PrepareObserved {
        execution_ordinal: usize,
        node_id: String,
        contract_id: String,
        provider_request_id: String,
    },
    AwaitingFreshConsent {
        execution_ordinal: usize,
        node_id: String,
        provider_request_id: String,
        authorization_sha256: String,
        snapshot_sha256: String,
        decision_sha256: String,
        predecessor_content_included: bool,
    },
    DispatchPlanned {
        execution_ordinal: usize,
        node_id: String,
        provider_request_id: String,
        authorization_sha256: String,
        snapshot_sha256: String,
        decision_sha256: String,
        effectful_step_reservation: u16,
        reserved_cost_usd_micros: u64,
        off_machine_consent_observed: bool,
        predecessor_content_consent_observed: bool,
    },
    NodeCompleted {
        execution_ordinal: usize,
        node_id: String,
        provider_request_id: String,
        terminal_receipt_sha256: String,
    },
    RetryablePreclaimFailure {
        execution_ordinal: usize,
        node_id: String,
        provider_request_id: String,
        reason: ScheduledGraphControllerRetryableFailure,
    },
    Stopped {
        reason: ScheduledGraphControllerStopReason,
        provider_request_id: Option<String>,
        snapshot_sha256: Option<String>,
        decision_sha256: Option<String>,
    },
    Completed {
        snapshot_sha256: String,
        decision_sha256: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduledGraphControllerEvent {
    pub v: u16,
    pub controller_id: String,
    pub graph_run_id: String,
    pub sequence: usize,
    pub previous_event_sha256: Option<String>,
    pub payload: ScheduledGraphControllerEventPayload,
    pub created_at_ms: u64,
    pub event_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledGraphControllerJournal {
    pub header: ScheduledGraphControllerHeader,
    pub events: Vec<ScheduledGraphControllerEvent>,
}

/// Exact private input for one passive, pinned-Core candidate materialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledGraphNodeMaterializationInput {
    pub control_snapshot: GroupAgentGraphControlSnapshot,
    pub schedule_sha256: String,
    pub execution_ordinal: usize,
    pub node_id: String,
    pub predecessor_receipts: Vec<GroupAgentScheduledNodeTerminalReceipt>,
    pub execution_profile: ScheduledGraphControllerExecutionProfile,
}

/// One exact candidate emitted by the pinned Core materializer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledGraphNodeMaterialization {
    pub candidate: GroupAgentScheduledNodeContractCandidate,
    pub candidate_json: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledGraphNodeMaterializationPortError {
    Unavailable,
    InvalidCandidate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppendScheduledGraphControllerDisposition {
    Stored,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppendScheduledGraphControllerResult {
    pub disposition: AppendScheduledGraphControllerDisposition,
    pub journal: ScheduledGraphControllerJournal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledGraphControllerValidationError {
    pub message: String,
}

impl ScheduledGraphControllerExecutionProfile {
    pub fn seal(mut self) -> Result<Self, ScheduledGraphControllerValidationError> {
        if !self.profile_sha256.is_empty() {
            return Err(validation::invalid(
                "controller execution profile is already sealed",
            ));
        }
        self.profile_sha256 = codec::profile_digest(&self)?;
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ScheduledGraphControllerValidationError> {
        validation::validate_profile(self)
    }

    pub fn canonical_json(&self) -> Result<String, ScheduledGraphControllerValidationError> {
        codec::canonical_json(self)
    }
}

impl ScheduledGraphControllerHeader {
    pub fn seal(mut self) -> Result<Self, ScheduledGraphControllerValidationError> {
        if !self.controller_id.is_empty() || !self.controller_sha256.is_empty() {
            return Err(validation::invalid("controller header is already sealed"));
        }
        self.controller_sha256 = codec::header_digest(&self)?;
        self.controller_id = format!("scheduled-graph-controller-{}", self.controller_sha256);
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), ScheduledGraphControllerValidationError> {
        validation::validate_header(self)
    }

    pub fn canonical_json(&self) -> Result<String, ScheduledGraphControllerValidationError> {
        codec::canonical_json(self)
    }

    pub fn decode_exact(json: &str) -> Result<Self, ScheduledGraphControllerValidationError> {
        codec::decode_header_exact(json.as_bytes())
    }
}

impl ScheduledGraphControllerEvent {
    pub fn seal(mut self) -> Result<Self, ScheduledGraphControllerValidationError> {
        if !self.event_sha256.is_empty() {
            return Err(validation::invalid("controller event is already sealed"));
        }
        self.event_sha256 = codec::event_digest(&self)?;
        validation::validate_event_shape(&self)?;
        Ok(self)
    }

    pub fn canonical_json(&self) -> Result<String, ScheduledGraphControllerValidationError> {
        codec::canonical_json(self)
    }

    pub fn decode_exact(json: &str) -> Result<Self, ScheduledGraphControllerValidationError> {
        codec::decode_event_exact(json.as_bytes())
    }
}

impl ScheduledGraphControllerJournal {
    pub fn validate(&self) -> Result<(), ScheduledGraphControllerValidationError> {
        validation::validate_journal(self)
    }

    #[must_use]
    pub fn head(&self) -> &ScheduledGraphControllerEvent {
        self.events.last().expect("validated controller journal")
    }

    #[must_use]
    pub fn effectful_steps_reserved(&self) -> u16 {
        self.events
            .iter()
            .filter(|event| {
                matches!(
                    event.payload,
                    ScheduledGraphControllerEventPayload::DispatchPlanned { .. }
                )
            })
            .fold(0_u16, |count, _| count.saturating_add(1))
    }

    #[must_use]
    pub fn cost_usd_micros_reserved(&self) -> u64 {
        self.events
            .iter()
            .filter_map(|event| match event.payload {
                ScheduledGraphControllerEventPayload::DispatchPlanned {
                    reserved_cost_usd_micros,
                    ..
                } => Some(reserved_cost_usd_micros),
                _ => None,
            })
            .fold(0_u64, u64::saturating_add)
    }

    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.head().payload,
            ScheduledGraphControllerEventPayload::Stopped { .. }
                | ScheduledGraphControllerEventPayload::Completed { .. }
        )
    }
}

pub trait ScheduledGraphControllerStore: Send + Sync {
    fn start_scheduled_graph_controller(
        &self,
        header: &ScheduledGraphControllerHeader,
        event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError>;

    fn append_scheduled_graph_controller_event(
        &self,
        event: &ScheduledGraphControllerEvent,
    ) -> Result<AppendScheduledGraphControllerResult, HubStoreError>;

    fn inspect_scheduled_graph_controller(
        &self,
        graph_run_id: &str,
    ) -> Result<ScheduledGraphControllerJournal, HubStoreError>;
}

pub trait ScheduledGraphNodeMaterializationPort: Send + Sync {
    /// Runs one passive candidate derivation through an explicitly pinned Core.
    ///
    /// # Errors
    ///
    /// Returns a redacted error when Core is unavailable or emits a candidate
    /// that is not exact, canonical, or bound to the selected schedule node.
    fn materialize(
        &self,
        input: &ScheduledGraphNodeMaterializationInput,
    ) -> Result<ScheduledGraphNodeMaterialization, ScheduledGraphNodeMaterializationPortError>;
}

impl std::fmt::Display for ScheduledGraphControllerValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ScheduledGraphControllerValidationError {}
