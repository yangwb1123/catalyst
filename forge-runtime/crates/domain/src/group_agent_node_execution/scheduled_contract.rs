use serde::{Deserialize, Serialize};

use super::{
    GroupAgentNodeExecutionApproval, GroupAgentNodeExecutionBudgets,
    GroupAgentNodeExecutionFailurePolicy, GroupAgentNodeExecutionProvider,
    GroupAgentNodeExecutionResultPolicy, GroupAgentNodeExecutionWorkspace,
    GroupAgentNodeSameProjectPolicy,
};
use crate::{GroupAgentGraphControlSnapshot, GroupAgentGraphExecutionSchedule, HubStoreError};

#[path = "scheduled_contract_admission_validation.rs"]
mod admission_validation;
#[path = "scheduled_contract_codec.rs"]
mod codec;
#[path = "scheduled_contract_validation.rs"]
mod validation;

#[cfg(test)]
#[path = "scheduled_contract_tests.rs"]
mod tests;

pub const GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION: u16 = 2;
pub const GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION: u16 = 2;
pub const GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION: u16 = 2;
pub const GROUP_AGENT_SCHEDULED_NODE_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-request.v2\0";
pub const GROUP_AGENT_SCHEDULED_NODE_CONTRACT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-contract.v2\0";
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_LIST_LIMIT: usize = 100;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeContractScope {
    ScheduleInitialNodeOnly,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeExecutionNode {
    pub execution_ordinal: usize,
    pub node_id: String,
    pub authored_node_index: usize,
    pub topology_wave_index: usize,
    pub attempt: u16,
    pub project_id: String,
    pub member_role: String,
    pub agent_profile: String,
    pub project_lane_sha256: String,
    pub same_project_policy: GroupAgentNodeSameProjectPolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodePredecessorOutcome {
    Completed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodePredecessorReceipt {
    pub predecessor_node_id: String,
    pub predecessor_attempt: u16,
    pub terminal_event_seq: u64,
    pub terminal_event_sha256: String,
    pub terminal_receipt_id: String,
    pub terminal_receipt_sha256: String,
    pub node_outcome: GroupAgentScheduledNodePredecessorOutcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeRequest {
    pub v: u16,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub schedule_sha256: String,
    pub execution_ordinal: usize,
    pub node_id: String,
    pub attempt: u16,
    pub system_prompt: String,
    pub system_prompt_bytes: usize,
    pub system_prompt_sha256: String,
    pub user_prompt: String,
    pub user_prompt_bytes: usize,
    pub user_prompt_sha256: String,
    pub required_predecessor_node_ids: Vec<String>,
    pub predecessor_terminal_receipts: Vec<GroupAgentScheduledNodePredecessorReceipt>,
    pub predecessor_content_included: bool,
    pub tools: Vec<String>,
    pub request_id: String,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeContractCandidate {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub node_execution_protocol_version: u16,
    pub execution_schedule_protocol_version: u16,
    pub contract_scope: GroupAgentScheduledNodeContractScope,
    pub graph_run_id: String,
    pub graph_id: String,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub core_plan_sha256: String,
    pub control_snapshot_sha256: String,
    pub schedule_id: String,
    pub schedule_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub node: GroupAgentScheduledNodeExecutionNode,
    pub request: GroupAgentScheduledNodeRequest,
    pub workspace: GroupAgentNodeExecutionWorkspace,
    pub provider: GroupAgentNodeExecutionProvider,
    pub budgets: GroupAgentNodeExecutionBudgets,
    pub approval: GroupAgentNodeExecutionApproval,
    pub result: GroupAgentNodeExecutionResultPolicy,
    pub failure: GroupAgentNodeExecutionFailurePolicy,
    pub lifecycle_contract_admitted: bool,
    pub provider_request_present: bool,
    pub execution_authority_released: bool,
    pub dispatch_authority_released: bool,
    pub progress_observed: bool,
    pub successor_advance_authorized: bool,
    pub contract_id: String,
    pub contract_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeContractRecord {
    pub v: u16,
    pub contract_id: String,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub node_id: String,
    pub execution_ordinal: usize,
    pub attempt: u16,
    pub control_snapshot_sha256: String,
    pub schedule_sha256: String,
    pub contract_sha256: String,
    pub contract_bytes: usize,
    pub request_id: String,
    pub request_sha256: String,
    pub project_lane_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub predecessor_receipt_count: usize,
    pub lifecycle_contract_admitted: bool,
    pub provider_request_present: bool,
    pub execution_authority_released: bool,
    pub dispatch_authority_released: bool,
    pub progress_observed: bool,
    pub successor_advance_authorized: bool,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentScheduledNodeContractCandidate {
    pub v: u16,
    pub graph_run_id: String,
    pub control_snapshot: GroupAgentGraphControlSnapshot,
    pub control_snapshot_json: String,
    pub schedule: GroupAgentGraphExecutionSchedule,
    pub schedule_json: String,
    pub candidate: GroupAgentScheduledNodeContractCandidate,
    pub candidate_json: String,
    pub idempotency_key: String,
    pub admitted_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmitGroupAgentScheduledNodeContractDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentScheduledNodeContractResult {
    pub v: u16,
    pub disposition: AdmitGroupAgentScheduledNodeContractDisposition,
    pub inspection: GroupAgentScheduledNodeContractInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeContractInspection {
    pub v: u16,
    pub record: GroupAgentScheduledNodeContractRecord,
    pub candidate_json: String,
    pub candidate: GroupAgentScheduledNodeContractCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeContractValidationError {
    pub message: String,
}

impl GroupAgentScheduledNodeRequest {
    /// Computes the domain-separated logical request identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the canonical digest payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
        codec::request_digest(self)
    }
}

impl GroupAgentScheduledNodeContractCandidate {
    /// Strictly decodes exact compact canonical candidate JSON.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact(
        json: &str,
    ) -> Result<Self, GroupAgentScheduledNodeContractValidationError> {
        codec::decode_exact(json.as_bytes())
    }

    /// Strictly decodes exact compact canonical candidate bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid UTF-8, JSON, canonical form, bounds, or semantics.
    pub fn decode_exact_bytes(
        bytes: &[u8],
    ) -> Result<Self, GroupAgentScheduledNodeContractValidationError> {
        codec::decode_exact(bytes)
    }

    /// Validates the passive initial-node candidate and all fixed policies.
    ///
    /// # Errors
    ///
    /// Returns an error for any invalid field, binding, bound, identity, or digest.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
        validation::validate_candidate(self)
    }

    /// Encodes the complete candidate as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_json(&self) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated candidate digest without its final ID fields.
    ///
    /// # Errors
    ///
    /// Returns an error when the canonical digest payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
        codec::contract_digest(self)
    }

    /// Validates the candidate against one exact control and admitted schedule.
    ///
    /// # Errors
    ///
    /// Returns an error for source, schedule, node, Prompt, head, or lane drift.
    pub fn validate_against_control_and_schedule(
        &self,
        control: &GroupAgentGraphControlSnapshot,
        schedule: &GroupAgentGraphExecutionSchedule,
    ) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
        admission_validation::validate_against_sources(self, control, schedule)
    }
}

impl GroupAgentScheduledNodeContractRecord {
    /// Validates content-free durable candidate metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, effectful, or inconsistent metadata.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
        admission_validation::validate_record(self)
    }
}

impl AdmitGroupAgentScheduledNodeContractCandidate {
    /// Validates exact candidate, control, and schedule admission bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, stale, divergent, or effectful input.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
        admission_validation::validate_admission(self)
    }
}

impl GroupAgentScheduledNodeContractInspection {
    /// Validates one stored candidate and all repeated metadata bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt bytes, metadata, identities, or false flags.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeContractValidationError> {
        admission_validation::validate_inspection(self)
    }
}

/// Builds the exact request-v2 user Prompt without predecessor metadata.
///
/// # Errors
///
/// Returns an error when canonical encoding fails.
pub fn group_agent_scheduled_node_user_prompt(
    node_id: &str,
    task: &str,
    acceptance: &str,
) -> Result<String, GroupAgentScheduledNodeContractValidationError> {
    codec::user_prompt(node_id, task, acceptance)
}

pub trait GroupAgentScheduledNodeContractStore: Send + Sync {
    /// Atomically admits or exactly replays one passive initial-node candidate.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn admit_group_agent_scheduled_node_contract(
        &self,
        request: &AdmitGroupAgentScheduledNodeContractCandidate,
    ) -> Result<AdmitGroupAgentScheduledNodeContractResult, HubStoreError>;

    /// Fully loads one immutable scheduled contract candidate.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or storage error.
    fn inspect_group_agent_scheduled_node_contract(
        &self,
        contract_id: &str,
    ) -> Result<GroupAgentScheduledNodeContractInspection, HubStoreError>;

    /// Lists bounded, content-free scheduled contract metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured validation, corruption, or storage error.
    fn list_group_agent_scheduled_node_contracts(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentScheduledNodeContractRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAgentScheduledNodeContractValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentScheduledNodeContractValidationError {}
