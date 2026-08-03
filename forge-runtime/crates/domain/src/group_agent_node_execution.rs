use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentGraphCorePlan, GroupAgentGraphManifest, GroupAgentGraphRunEvent,
    GroupAgentGraphRunInspection, HubStoreError,
};

#[path = "group_agent_node_execution_admission_validation.rs"]
mod admission_validation;
#[path = "group_agent_node_execution_codec.rs"]
mod codec;
#[path = "group_agent_node_execution/dispatch.rs"]
mod dispatch;
#[path = "group_agent_node_execution/dispatch_release.rs"]
mod dispatch_release;
#[path = "group_agent_node_execution/schedule.rs"]
mod schedule;
#[path = "group_agent_node_execution/scheduled_contract.rs"]
mod scheduled_contract;
#[path = "group_agent_node_execution/scheduled_dispatch_release.rs"]
mod scheduled_dispatch_release;
#[path = "group_agent_node_execution/scheduled_provider_request.rs"]
mod scheduled_provider_request;
#[path = "group_agent_node_execution_validation.rs"]
mod validation;

pub use codec::{
    group_agent_node_system_prompt, group_agent_node_user_prompt, group_agent_prompt_sha256,
};
pub use dispatch::*;
pub use dispatch_release::*;
pub use schedule::*;
pub use scheduled_contract::*;
pub use scheduled_dispatch_release::*;
pub use scheduled_provider_request::*;

pub const GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_EXECUTION_CONTRACT_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_EXECUTION_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-graph-control-snapshot.v1\0";
pub const GROUP_AGENT_NODE_EXECUTION_CONTRACT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-execution-contract.v1\0";
pub const GROUP_AGENT_NODE_REQUEST_DIGEST_DOMAIN: &[u8] = b"forge.group-agent-node-request.v1\0";
pub const GROUP_AGENT_PROJECT_LANE_DIGEST_DOMAIN: &[u8] = b"forge.group-agent-project-lane.v1\0";
pub const MAX_GROUP_AGENT_GRAPH_CONTROL_SNAPSHOT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_NODE_EXECUTION_CONTRACT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_GROUP_AGENT_NODE_PROVIDER_ENDPOINT_BYTES: usize = 2_048;
pub const MAX_GROUP_AGENT_NODE_MODEL_BYTES: usize = 128;
pub const MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS: u32 = 32_768;
pub const MAX_GROUP_AGENT_NODE_MODEL_OUTPUT_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_AGENT_NODE_MODEL_EVENTS: u32 = 4_096;
pub const MAX_GROUP_AGENT_NODE_TIMEOUT_MS: u64 = 86_400_000;
pub const MAX_GROUP_AGENT_NODE_COST_USD_MICROS: u64 = 1_000_000_000_000;
pub const MAX_GROUP_AGENT_NODE_RESULT_BYTES: usize = 512 * 1024;
pub const MAX_GROUP_AGENT_NODE_EXECUTION_CONTRACT_LIST_LIMIT: usize = 100;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphControlSnapshot {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub graph_run_version: u16,
    pub graph_run_id: String,
    pub graph_id: String,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub core_plan_sha256: String,
    pub last_event_seq: u64,
    pub last_event_sha256: String,
    pub execution_contract_present: bool,
    pub dispatch_authority_released: bool,
    pub plan: GroupAgentGraphCorePlan,
    pub manifest: GroupAgentGraphManifest,
    pub snapshot_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionNode {
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
pub enum GroupAgentNodeSameProjectPolicy {
    ExclusiveUntilTerminal,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionWorkspace {
    pub mode: GroupAgentNodeWorkspaceMode,
    pub root_identity: Option<String>,
    pub isolation_id: Option<String>,
    pub allowed_read_paths: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeWorkspaceMode {
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionProvider {
    pub kind: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub store: bool,
    pub stream: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeProviderKind {
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionRequest {
    pub system_prompt: String,
    pub system_prompt_bytes: usize,
    pub system_prompt_sha256: String,
    pub user_prompt: String,
    pub user_prompt_bytes: usize,
    pub user_prompt_sha256: String,
    pub predecessor_result_receipts: Vec<String>,
    pub tools: Vec<String>,
    pub request_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionBudgets {
    pub max_turns: u16,
    pub max_tool_calls: u16,
    pub max_output_tokens: u32,
    pub max_model_output_bytes: usize,
    pub max_model_events: u32,
    pub timeout_ms: u64,
    pub max_cost_usd_micros: u64,
    pub pricing_snapshot_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionApproval {
    pub provider_dispatch: GroupAgentNodeProviderApproval,
    pub workspace: GroupAgentNodeEffectApproval,
    pub tools: GroupAgentNodeEffectApproval,
    pub writeback: GroupAgentNodeEffectApproval,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeProviderApproval {
    FreshOffMachineConsent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeEffectApproval {
    Forbidden,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionResultPolicy {
    pub artifact_kind: GroupAgentNodeArtifactKind,
    pub max_result_bytes: usize,
    pub predecessor_dataflow: GroupAgentNodeDataflowPolicy,
    pub conversation_writeback: GroupAgentNodeWritebackPolicy,
    pub prompt_writeback: GroupAgentNodeWritebackPolicy,
    pub memory_writeback: GroupAgentNodeWritebackPolicy,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeArtifactKind {
    LocalGraphNodeArtifact,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeDataflowPolicy {
    None,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeWritebackPolicy {
    None,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionFailurePolicy {
    pub automatic_retry: bool,
    pub lease_retry: bool,
    pub post_claim_uncertainty: GroupAgentNodePostClaimUncertainty,
    pub failure_propagation_owner: GroupAgentNodeFailurePropagationOwner,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodePostClaimUncertainty {
    DispatchUnknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeFailurePropagationOwner {
    ForgeCore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionContract {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub node_execution_protocol_version: u16,
    pub graph_run_id: String,
    pub graph_id: String,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub core_plan_sha256: String,
    pub control_snapshot_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub node: GroupAgentNodeExecutionNode,
    pub workspace: GroupAgentNodeExecutionWorkspace,
    pub provider: GroupAgentNodeExecutionProvider,
    pub request: GroupAgentNodeExecutionRequest,
    pub budgets: GroupAgentNodeExecutionBudgets,
    pub approval: GroupAgentNodeExecutionApproval,
    pub result: GroupAgentNodeExecutionResultPolicy,
    pub failure: GroupAgentNodeExecutionFailurePolicy,
    pub execution_contract_present: bool,
    pub dispatch_authority_released: bool,
    pub contract_id: String,
    pub contract_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeExecutionContractRecord {
    pub v: u16,
    pub contract_id: String,
    pub graph_run_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub control_snapshot_sha256: String,
    pub contract_sha256: String,
    pub contract_bytes: usize,
    pub request_sha256: String,
    pub project_lane_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentNodeExecutionContract {
    pub v: u16,
    pub graph_run_id: String,
    pub control_snapshot: GroupAgentGraphControlSnapshot,
    pub control_snapshot_json: String,
    pub contract: GroupAgentNodeExecutionContract,
    pub contract_json: String,
    pub event: GroupAgentGraphRunEvent,
    pub event_json: String,
    pub idempotency_key: String,
    pub admitted_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmitGroupAgentNodeExecutionContractDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitGroupAgentNodeExecutionContractResult {
    pub v: u16,
    pub disposition: AdmitGroupAgentNodeExecutionContractDisposition,
    pub inspection: GroupAgentNodeExecutionContractInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeExecutionContractInspection {
    pub v: u16,
    pub record: GroupAgentNodeExecutionContractRecord,
    pub contract_json: String,
    pub contract: GroupAgentNodeExecutionContract,
    pub admission_event_json: String,
    pub admission_event: GroupAgentGraphRunEvent,
    pub graph_run: GroupAgentGraphRunInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeExecutionValidationError {
    pub message: String,
}

impl GroupAgentGraphControlSnapshot {
    /// Validates one canonical, effect-free Core control snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid source, cursor, authority flag, or digest.
    pub fn validate(&self) -> Result<(), GroupAgentNodeExecutionValidationError> {
        validation::validate_control_snapshot(self)
    }

    /// Encodes the complete snapshot as exact compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeExecutionValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated digest excluding `snapshot_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeExecutionValidationError> {
        codec::control_snapshot_digest(self)
    }
}

impl GroupAgentNodeExecutionRequest {
    /// Computes the domain-separated request digest excluding `request_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeExecutionValidationError> {
        codec::request_digest(self)
    }
}

impl GroupAgentNodeExecutionContract {
    /// Validates the complete passive version-1 Node Execution Contract.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed identity, policy, budget, request, or digest.
    pub fn validate(&self) -> Result<(), GroupAgentNodeExecutionValidationError> {
        validation::validate_contract(self)
    }

    /// Encodes the complete contract as exact compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeExecutionValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the payload digest excluding `contract_id` and `contract_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical encoding fails.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeExecutionValidationError> {
        codec::contract_digest(self)
    }

    /// Validates this contract against one complete reconstructed control snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when source bindings, first-node selection, or Prompts disagree.
    pub fn validate_against_control(
        &self,
        snapshot: &GroupAgentGraphControlSnapshot,
    ) -> Result<(), GroupAgentNodeExecutionValidationError> {
        admission_validation::validate_against_control(self, snapshot)
    }
}

impl GroupAgentNodeExecutionContractRecord {
    /// Validates content-free admitted-contract metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or inconsistent metadata.
    pub fn validate(&self) -> Result<(), GroupAgentNodeExecutionValidationError> {
        admission_validation::validate_record(self)
    }
}

impl AdmitGroupAgentNodeExecutionContract {
    /// Validates exact canonical admission bytes and cross-bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, divergent, or effectful input.
    pub fn validate(&self) -> Result<(), GroupAgentNodeExecutionValidationError> {
        admission_validation::validate_admission(self)
    }
}

impl GroupAgentNodeExecutionContractInspection {
    /// Fully validates an admitted contract and its updated Graph Run aggregate.
    ///
    /// # Errors
    ///
    /// Returns an error for any corrupt bytes, receipt, source, or journal binding.
    pub fn validate(&self) -> Result<(), GroupAgentNodeExecutionValidationError> {
        admission_validation::validate_inspection(self)
    }
}

/// Computes the unkeyed project-lane identity used by the global single-flight rule.
#[must_use]
pub fn group_agent_project_lane_sha256(project_id: &str) -> String {
    codec::digest_hex(
        GROUP_AGENT_PROJECT_LANE_DIGEST_DOMAIN,
        project_id.as_bytes(),
    )
}

pub trait GroupAgentNodeExecutionContractStore: Send + Sync {
    /// Atomically admits or exactly replays one first-node contract.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn admit_group_agent_node_execution_contract(
        &self,
        request: &AdmitGroupAgentNodeExecutionContract,
    ) -> Result<AdmitGroupAgentNodeExecutionContractResult, HubStoreError>;

    /// Fully loads one contract and its exact Graph Run aggregate.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or storage error.
    fn inspect_group_agent_node_execution_contract(
        &self,
        contract_id: &str,
    ) -> Result<GroupAgentNodeExecutionContractInspection, HubStoreError>;

    /// Lists bounded, content-free admitted-contract metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured validation, corruption, or storage error.
    fn list_group_agent_node_execution_contracts(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentNodeExecutionContractRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAgentNodeExecutionValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentNodeExecutionValidationError {}
