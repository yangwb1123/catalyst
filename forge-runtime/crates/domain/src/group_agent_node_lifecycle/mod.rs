use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodePricingSnapshot, GroupAgentNodeProviderKind,
    PreparedModelProvider,
};

mod artifact;
mod artifact_validation;
mod claim;
mod codec;
mod inspection_validation;
mod terminal;
mod terminal_validation;
mod validation;

pub use artifact::*;
pub use claim::*;
pub use terminal::*;

pub const GROUP_AGENT_NODE_DISPATCH_CLAIM_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_ACTIVE_LANE_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_LIFECYCLE_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_TERMINAL_ARTIFACT_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_TERMINAL_ARTIFACT_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_TERMINAL_CONTROL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_TERMINAL_CONTROL_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_TERMINAL_RECEIPT_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_TERMINAL_RECEIPT_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_TERMINAL_ARTIFACT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-terminal-artifact.v1\0";
pub const GROUP_AGENT_NODE_TERMINAL_OUTPUT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-terminal-output.v1\0";
pub const GROUP_AGENT_NODE_TERMINAL_CONTROL_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-terminal-control.v1\0";
pub const GROUP_AGENT_NODE_TERMINAL_RECEIPT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-terminal-receipt.v1\0";
pub const MAX_GROUP_AGENT_NODE_TERMINAL_ARTIFACT_BYTES: usize = 1024 * 1024;
pub const MAX_GROUP_AGENT_NODE_TERMINAL_CONTROL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_GROUP_AGENT_NODE_TERMINAL_RECEIPT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeTerminalArtifactKind {
    Result,
    Uncertainty,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeTerminalClassification {
    Completed,
    Length,
    ProviderError,
    HttpError,
    TransportError,
    Timeout,
    Cancelled,
    EofBeforeTerminal,
    MissingUsage,
    ToolCall,
    ProtocolError,
    TrailingData,
    LocalLimit,
    HardCrash,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeTerminalOutcome {
    Completed,
    Failed,
    FailedUncertain,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeLifecycleValidationError {
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeResolvedDispatch {
    pub authorization_sha256: String,
    pub provider_kind: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub max_input_tokens: u64,
    pub max_output_tokens: u32,
    pub max_cost_usd_micros: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeDispatchProviderFactoryError {
    pub message: String,
}

pub trait GroupAgentNodeDispatchProviderFactory: Send + Sync {
    /// Resolves an exact registered destination without credentials or I/O.
    ///
    /// # Errors
    ///
    /// Returns a redacted rejection when authorization or pricing is unsupported.
    fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodeResolvedDispatch, GroupAgentNodeDispatchProviderFactoryError>;

    /// Constructs the provider with one explicitly supplied credential and no request.
    ///
    /// # Errors
    ///
    /// Returns a redacted rejection when safe provider construction fails.
    fn build(
        &self,
        resolved: GroupAgentNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn PreparedModelProvider>, GroupAgentNodeDispatchProviderFactoryError>;
}

/// Derives the content-addressed terminal artifact ID.
#[must_use]
pub fn group_agent_node_terminal_artifact_id(artifact_sha256: &str) -> String {
    format!("graph-node-terminal-artifact-{artifact_sha256}")
}

/// Derives the content-addressed Core terminal receipt ID.
#[must_use]
pub fn group_agent_node_terminal_receipt_id(receipt_sha256: &str) -> String {
    format!("graph-node-terminal-receipt-{receipt_sha256}")
}

/// Computes the domain-separated identity of raw UTF-8 terminal output bytes.
#[must_use]
pub fn group_agent_node_terminal_output_sha256(output: &str) -> String {
    codec::digest_hex(
        GROUP_AGENT_NODE_TERMINAL_OUTPUT_DIGEST_DOMAIN,
        output.as_bytes(),
    )
}

impl std::fmt::Display for GroupAgentNodeLifecycleValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentNodeLifecycleValidationError {}

impl std::fmt::Display for GroupAgentNodeDispatchProviderFactoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentNodeDispatchProviderFactoryError {}
