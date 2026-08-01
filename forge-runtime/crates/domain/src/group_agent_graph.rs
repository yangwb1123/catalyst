use serde::{Deserialize, Serialize};

use crate::HubStoreError;

#[path = "group_agent_graph_validation.rs"]
mod validation;

pub const GROUP_AGENT_GRAPH_VERSION: u16 = 1;
pub const GROUP_AGENT_GRAPH_MANIFEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-graph-manifest.v1\0";
pub const MAX_GROUP_AGENT_GRAPH_NODES: usize = 32;
pub const MAX_GROUP_AGENT_GRAPH_EDGES: usize = 512;
pub const MAX_GROUP_AGENT_GRAPH_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_IDENTIFIER_BYTES: usize = 128;
pub const MAX_GROUP_AGENT_GRAPH_AGENT_PROFILE_BYTES: usize = 128;
pub const MAX_GROUP_AGENT_GRAPH_MEMBER_ROLE_BYTES: usize = 64;
pub const MAX_GROUP_AGENT_GRAPH_MANAGER_INSTRUCTION_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_NODE_TASK_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_NODE_ACCEPTANCE_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_IDEMPOTENCY_KEY_BYTES: usize = 256;
pub const MAX_GROUP_AGENT_GRAPH_LIST_LIMIT: usize = 100;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphSource {
    pub group_run_version: u16,
    pub group_run_id: String,
    pub group_id: String,
    pub context_version: u16,
    pub context_slice_sha256: String,
    pub snapshot_sha256: String,
    pub snapshot_bytes: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphManager {
    pub agent_profile: String,
    pub instruction: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphNode {
    pub node_id: String,
    pub project_id: String,
    pub member_role: String,
    pub agent_profile: String,
    pub task: String,
    pub acceptance: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
/// A dependency-order constraint only; edges never carry node output or result data.
pub struct GroupAgentGraphEdge {
    pub from_node_id: String,
    pub to_node_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphManifest {
    pub v: u16,
    pub source: GroupAgentGraphSource,
    pub manager: GroupAgentGraphManager,
    pub nodes: Vec<GroupAgentGraphNode>,
    pub edges: Vec<GroupAgentGraphEdge>,
    pub waves: Vec<Vec<String>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphStatus {
    Prepared,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphRecord {
    pub v: u16,
    pub graph_id: String,
    pub group_run_id: String,
    pub status: GroupAgentGraphStatus,
    pub source_snapshot_sha256: String,
    pub manifest_sha256: String,
    pub manifest_bytes: usize,
    pub node_count: usize,
    pub edge_count: usize,
    pub wave_count: usize,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentGraph {
    pub v: u16,
    pub graph_id: String,
    pub manifest: GroupAgentGraphManifest,
    pub manifest_json: String,
    pub manifest_sha256: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareGroupAgentGraphDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentGraphResult {
    pub v: u16,
    pub disposition: PrepareGroupAgentGraphDisposition,
    pub inspection: GroupAgentGraphInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentGraphInspection {
    pub v: u16,
    pub graph: GroupAgentGraphRecord,
    pub manifest: GroupAgentGraphManifest,
    pub manifest_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentGraphValidationError {
    pub message: String,
}

impl GroupAgentGraphSource {
    /// Validates one exact prepared Group Run binding.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed versions, identities, digests, or size.
    pub fn validate(&self) -> Result<(), GroupAgentGraphValidationError> {
        validation::validate_source(self)
    }
}

impl GroupAgentGraphManager {
    /// Validates one inert manager label and bounded instruction.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty, oversized, or unsafe field.
    pub fn validate(&self) -> Result<(), GroupAgentGraphValidationError> {
        validation::validate_manager(self)
    }
}

impl GroupAgentGraphNode {
    /// Validates one inert member-project Agent assignment.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid identity, label, task, or acceptance text.
    pub fn validate(&self) -> Result<(), GroupAgentGraphValidationError> {
        validation::validate_node(self)
    }
}

impl GroupAgentGraphManifest {
    /// Validates the bounded DAG and its deterministic authored-order waves.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed content, edges, cycles, or wave divergence.
    pub fn validate(&self) -> Result<(), GroupAgentGraphValidationError> {
        validation::validate_manifest(self)
    }

    /// Computes the canonical, domain-separated manifest digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest cannot be canonically encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentGraphValidationError> {
        validation::manifest_digest(self)
    }
}

impl GroupAgentGraphRecord {
    /// Validates content-free durable graph metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata violates the versioned contract.
    pub fn validate(&self) -> Result<(), GroupAgentGraphValidationError> {
        validation::validate_record(self)
    }
}

impl PrepareGroupAgentGraph {
    /// Validates one bounded, local-only graph preparation candidate.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, manifest, bytes, digest, key, or time.
    pub fn validate(&self) -> Result<(), GroupAgentGraphValidationError> {
        validation::validate_prepare(self)
    }
}

impl GroupAgentGraphInspection {
    /// Validates durable metadata against its manifest and stored byte envelope.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or divergent graph state.
    pub fn validate(&self) -> Result<(), GroupAgentGraphValidationError> {
        validation::validate_inspection(self)
    }
}

/// Computes deterministic Kahn waves, preserving authored node order.
///
/// # Errors
///
/// Returns an error for duplicate nodes or edges, bad endpoints, self edges, or cycles.
pub fn compute_group_agent_graph_waves(
    nodes: &[GroupAgentGraphNode],
    edges: &[GroupAgentGraphEdge],
) -> Result<Vec<Vec<String>>, GroupAgentGraphValidationError> {
    validation::compute_waves(nodes, edges)
}

pub trait GroupAgentGraphStore: Send + Sync {
    /// Atomically freezes one canonical local graph definition.
    ///
    /// Implementations must load and fully validate the referenced prepared
    /// Group Run plus every frozen project-role binding in the same transaction
    /// used for key lookup, replay validation, insert, and durable reread.
    /// Treating the caller's source projection as authority violates this port.
    ///
    /// # Errors
    ///
    /// Returns a structured error for conflict, corruption, or unavailable storage.
    fn prepare_group_agent_graph(
        &self,
        request: &PrepareGroupAgentGraph,
    ) -> Result<PrepareGroupAgentGraphResult, HubStoreError>;

    /// Loads one graph together with its exact canonical manifest bytes.
    ///
    /// Implementations must read the graph and referenced Group Run in one
    /// consistent snapshot and revalidate source and member bindings before
    /// returning. A metadata-only list is the sole operation exempt from this
    /// full source validation.
    ///
    /// # Errors
    ///
    /// Returns a structured error when the graph is missing, corrupt, or unavailable.
    fn inspect_group_agent_graph(
        &self,
        graph_id: &str,
    ) -> Result<GroupAgentGraphInspection, HubStoreError>;

    /// Lists bounded, content-free graph metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured error for invalid filters, corrupt metadata, or storage failure.
    fn list_group_agent_graphs(
        &self,
        group_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAgentGraphValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentGraphValidationError {}
