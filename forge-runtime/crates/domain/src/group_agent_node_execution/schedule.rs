use serde::{Deserialize, Serialize};

use crate::{GroupAgentGraphControlSnapshot, HubStoreError};

#[path = "schedule_admission_validation.rs"]
mod admission_validation;
#[path = "schedule_codec.rs"]
mod codec;
#[path = "schedule_validation.rs"]
mod validation;

#[cfg(test)]
#[path = "schedule_tests.rs"]
mod tests;

pub const GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_VERSION: u16 = 1;
pub const GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-graph-execution-schedule.v1\0";
pub const MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_BYTES: usize = 1024 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_EXECUTION_SCHEDULE_LIST_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphExecutionMode {
    Serial,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphExecutionSelectionPolicy {
    TopologyWaveThenAuthoredOrder,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphExecutionProgressionPolicy {
    CompletedContiguousPrefix,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphExecutionAttemptPolicy {
    ExactlyOne,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphExecutionFailurePolicy {
    FailFastNoRetry,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphCompletedOutcomePolicy {
    AdvanceOrComplete,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphLengthOutcomePolicy {
    FailGraph,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphUncertaintyOutcomePolicy {
    FailGraphUncertain,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphDispatchUnknownOutcomePolicy {
    QuarantineNoAdvance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphExecutionScheduleOutcomePolicy {
    pub completed: GroupAgentGraphCompletedOutcomePolicy,
    pub length: GroupAgentGraphLengthOutcomePolicy,
    pub uncertainty: GroupAgentGraphUncertaintyOutcomePolicy,
    pub dispatch_unknown: GroupAgentGraphDispatchUnknownOutcomePolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphPredecessorSemantics {
    OrderingOnly,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphPredecessorDataflow {
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphReceiptHandling {
    FutureVerifiedIdentitySlots,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphExecutionScheduleNode {
    pub execution_ordinal: usize,
    pub node_id: String,
    pub authored_node_index: usize,
    pub topology_wave_index: usize,
    pub project_lane_sha256: String,
    pub attempt: u16,
    pub direct_predecessor_node_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentGraphExecutionSchedule {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub execution_schedule_protocol_version: u16,
    pub control_snapshot_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub graph_run_id: String,
    pub graph_id: String,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub core_plan_sha256: String,
    pub node_count: usize,
    pub wave_count: usize,
    pub execution_mode: GroupAgentGraphExecutionMode,
    pub max_in_flight_nodes: usize,
    pub selection_policy: GroupAgentGraphExecutionSelectionPolicy,
    pub progression_policy: GroupAgentGraphExecutionProgressionPolicy,
    pub attempt_policy: GroupAgentGraphExecutionAttemptPolicy,
    pub failure_policy: GroupAgentGraphExecutionFailurePolicy,
    pub outcome_policy: GroupAgentGraphExecutionScheduleOutcomePolicy,
    pub predecessor_semantics: GroupAgentGraphPredecessorSemantics,
    pub predecessor_dataflow: GroupAgentGraphPredecessorDataflow,
    pub partial_output_dataflow: bool,
    pub receipt_handling: GroupAgentGraphReceiptHandling,
    pub nodes: Vec<GroupAgentGraphExecutionScheduleNode>,
    pub initial_frontier: Vec<String>,
    pub initial_node: String,
    pub execution_contract_present: bool,
    pub dispatch_authority_released: bool,
    pub progress_observed: bool,
    pub successor_advanced: bool,
    pub schedule_id: String,
    pub schedule_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphExecutionScheduleRecord {
    pub v: u16,
    pub schedule_id: String,
    pub graph_run_id: String,
    pub graph_id: String,
    pub control_snapshot_sha256: String,
    pub schedule_sha256: String,
    pub schedule_bytes: usize,
    pub node_count: usize,
    pub wave_count: usize,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub execution_contract_present: bool,
    pub dispatch_authority_released: bool,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentGraphExecutionSchedule {
    pub v: u16,
    pub graph_run_id: String,
    pub control_snapshot: GroupAgentGraphControlSnapshot,
    pub control_snapshot_json: String,
    pub schedule: GroupAgentGraphExecutionSchedule,
    pub schedule_json: String,
    pub idempotency_key: String,
    pub admitted_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmitGroupAgentGraphExecutionScheduleDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentGraphExecutionScheduleResult {
    pub v: u16,
    pub disposition: AdmitGroupAgentGraphExecutionScheduleDisposition,
    pub inspection: GroupAgentGraphExecutionScheduleInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentGraphExecutionScheduleInspection {
    pub v: u16,
    pub record: GroupAgentGraphExecutionScheduleRecord,
    pub schedule_json: String,
    pub schedule: GroupAgentGraphExecutionSchedule,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentGraphExecutionScheduleValidationError {
    pub message: String,
}

impl GroupAgentGraphExecutionSchedule {
    /// Strictly decodes one exact compact canonical schedule.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact(
        json: &str,
    ) -> Result<Self, GroupAgentGraphExecutionScheduleValidationError> {
        Self::decode_exact_bytes(json.as_bytes())
    }

    /// Strictly decodes exact canonical schedule bytes, including UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact_bytes(
        bytes: &[u8],
    ) -> Result<Self, GroupAgentGraphExecutionScheduleValidationError> {
        codec::decode_exact(bytes)
    }

    /// Validates the passive schedule's shape, fixed policy, identity, and digest.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid field, policy, bound, identity, or digest.
    pub fn validate(&self) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
        validation::validate_schedule(self)
    }

    /// Encodes the complete schedule as exact compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentGraphExecutionScheduleValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated digest excluding `schedule_id` and `schedule_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentGraphExecutionScheduleValidationError> {
        codec::schedule_digest(self)
    }

    /// Validates every identity, topology, lane, and head against one exact v1 control.
    ///
    /// # Errors
    ///
    /// Returns an error when the schedule is not the unique schedule for the control.
    pub fn validate_against_control(
        &self,
        snapshot: &GroupAgentGraphControlSnapshot,
    ) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
        admission_validation::validate_against_control(self, snapshot)
    }
}

impl GroupAgentGraphExecutionScheduleRecord {
    /// Validates content-free durable schedule metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or inconsistent metadata.
    pub fn validate(&self) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
        admission_validation::validate_record(self)
    }
}

impl AdmitGroupAgentGraphExecutionSchedule {
    /// Validates exact canonical sidecar admission bytes and cross-bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, divergent, stale, or effectful input.
    pub fn validate(&self) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
        admission_validation::validate_admission(self)
    }
}

impl GroupAgentGraphExecutionScheduleInspection {
    /// Fully validates an admitted schedule and its exact durable bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt bytes, metadata, or schedule bindings.
    pub fn validate(&self) -> Result<(), GroupAgentGraphExecutionScheduleValidationError> {
        admission_validation::validate_inspection(self)
    }
}

pub trait GroupAgentGraphExecutionScheduleStore: Send + Sync {
    /// Atomically admits or exactly replays one immutable passive schedule sidecar.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn admit_group_agent_graph_execution_schedule(
        &self,
        request: &AdmitGroupAgentGraphExecutionSchedule,
    ) -> Result<AdmitGroupAgentGraphExecutionScheduleResult, HubStoreError>;

    /// Fully loads one immutable passive schedule sidecar.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or storage error.
    fn inspect_group_agent_graph_execution_schedule(
        &self,
        schedule_id: &str,
    ) -> Result<GroupAgentGraphExecutionScheduleInspection, HubStoreError>;

    /// Lists bounded, content-free schedule metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured validation, corruption, or storage error.
    fn list_group_agent_graph_execution_schedules(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphExecutionScheduleRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAgentGraphExecutionScheduleValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentGraphExecutionScheduleValidationError {}
