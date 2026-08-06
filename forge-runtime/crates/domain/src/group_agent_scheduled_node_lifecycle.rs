#![allow(clippy::missing_errors_doc)]

use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentGraphRunInspection, GroupAgentNodePricingSnapshot,
    GroupAgentNodeTerminalClassification, GroupAgentNodeTerminalOutcome,
    GroupAgentScheduledNodeDispatchAuthorization, GroupAgentScheduledNodeDispatchReleaseControl,
    GroupAgentScheduledNodeProviderRequestRecord, HubStoreError,
};

#[path = "group_agent_scheduled_node_lifecycle_api.rs"]
mod api;
pub use api::{
    group_agent_scheduled_node_terminal_artifact_id,
    group_agent_scheduled_node_terminal_output_sha256,
    group_agent_scheduled_node_terminal_receipt_id,
};
#[path = "group_agent_scheduled_node_lifecycle_validation.rs"]
mod validation;

pub const GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_TERMINAL_ARTIFACT_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_TERMINAL_CONTROL_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_TERMINAL_RECEIPT_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_TERMINAL_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_CLAIM_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-claim.v1\0";
pub const GROUP_AGENT_SCHEDULED_NODE_ARTIFACT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-terminal-artifact.v1\0";
pub const GROUP_AGENT_SCHEDULED_NODE_OUTPUT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-terminal-output.v1\0";
pub const GROUP_AGENT_SCHEDULED_NODE_CONTROL_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-terminal-control.v1\0";
pub const GROUP_AGENT_SCHEDULED_NODE_RECEIPT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-terminal-receipt.v1\0";
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_ARTIFACT_BYTES: usize = 1024 * 1024;
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_CONTROL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_RECEIPT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeLifecycleStatus {
    Claimed,
    Terminalized,
    Quarantined,
    Adjudicated,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeTerminalArtifactKind {
    Result,
    Uncertainty,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeDispatchClaim {
    pub v: u16,
    pub graph_run_id: String,
    pub provider_request_id: String,
    pub dispatch_id: String,
    pub authorization_id: String,
    pub authorization_sha256: String,
    pub provider_request_sha256: String,
    pub request_body_sha256: String,
    pub request_body_bytes: usize,
    pub pricing_snapshot_sha256: String,
    pub node_id: String,
    pub attempt: u16,
    pub max_cost_usd_micros: u64,
    pub lane_ownership_id: String,
    pub project_lane_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub claim_event_sha256: String,
    pub released_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeActiveLane {
    pub v: u16,
    pub project_lane_sha256: String,
    pub lane_ownership_id: String,
    pub graph_run_id: String,
    pub provider_request_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub claim_event_sha256: String,
    pub claimed_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeDispatchClaimEvent {
    pub v: u16,
    pub graph_run_id: String,
    pub provider_request_id: String,
    pub dispatch_id: String,
    pub authorization_id: String,
    pub authorization_sha256: String,
    pub provider_request_sha256: String,
    pub project_lane_sha256: String,
    pub node_id: String,
    pub attempt: u16,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub lane_ownership_id: String,
    pub released_at_ms: u64,
    pub event_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeTerminalArtifact {
    pub v: u16,
    pub terminal_artifact_protocol_version: u16,
    pub artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind,
    pub graph_run_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub provider_request_id: String,
    pub claim_event_sha256: String,
    pub authorization_sha256: String,
    pub provider_request_sha256: String,
    pub request_body_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub lane_ownership_id: String,
    pub project_lane_sha256: String,
    pub provider_poll_started: bool,
    pub terminal_seen: bool,
    pub stream_eof_seen: bool,
    pub classification: GroupAgentNodeTerminalClassification,
    pub output_text: String,
    pub output_bytes: usize,
    pub output_sha256: String,
    pub usage_observed: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub actual_cost_calculated: bool,
    pub actual_cost_usd_micros: u64,
    pub retry_authorized: bool,
    pub created_at_ms: u64,
    pub artifact_id: String,
    pub artifact_bytes: usize,
    pub artifact_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeTerminalControl {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub terminal_control_protocol_version: u16,
    pub release_control_snapshot_sha256: String,
    pub graph_run_id: String,
    pub graph_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub provider_request_id: String,
    pub authorization_sha256: String,
    pub provider_request_sha256: String,
    pub request_body_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub claim_event_sha256: String,
    pub project_lane_sha256: String,
    pub artifact: GroupAgentScheduledNodeTerminalArtifact,
    pub snapshot_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeTerminalReceipt {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub terminal_receipt_protocol_version: u16,
    pub terminal_control_sha256: String,
    pub graph_run_id: String,
    pub graph_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub provider_request_id: String,
    pub project_lane_sha256: String,
    pub artifact_kind: GroupAgentScheduledNodeTerminalArtifactKind,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub node_outcome: GroupAgentNodeTerminalOutcome,
    pub retry_authorized: bool,
    pub lane_release_authorized: bool,
    pub successor_advance_authorized: bool,
    pub receipt_id: String,
    pub receipt_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeCoreTerminalReceiptEnvelope {
    pub receipt: GroupAgentScheduledNodeTerminalReceipt,
    pub receipt_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeTerminalReceiptPortError {
    pub message: String,
}

pub trait GroupAgentScheduledNodeTerminalReceiptPort: Send + Sync {
    fn decide(
        &self,
        control: &GroupAgentScheduledNodeTerminalControl,
    ) -> Result<
        GroupAgentScheduledNodeCoreTerminalReceiptEnvelope,
        GroupAgentScheduledNodeTerminalReceiptPortError,
    >;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeLifecycleValidationError {
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimGroupAgentScheduledNodeDispatch {
    pub v: u16,
    pub release_control: GroupAgentScheduledNodeDispatchReleaseControl,
    pub release_control_json: String,
    pub authorization: GroupAgentScheduledNodeDispatchAuthorization,
    pub authorization_json: String,
    pub pricing: GroupAgentNodePricingSnapshot,
    pub pricing_json: String,
    pub provider_request: GroupAgentScheduledNodeProviderRequestRecord,
    pub provider_request_body: Vec<u8>,
    pub claim: GroupAgentScheduledNodeDispatchClaim,
    pub claim_json: String,
    pub active_lane: GroupAgentScheduledNodeActiveLane,
    pub active_lane_json: String,
    pub claim_event: GroupAgentScheduledNodeDispatchClaimEvent,
    pub claim_event_json: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeDispatchAuthority {
    claim: GroupAgentScheduledNodeDispatchClaim,
    request_body: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeLifecycleInspection {
    pub v: u16,
    pub graph_run: GroupAgentGraphRunInspection,
    pub release_control: GroupAgentScheduledNodeDispatchReleaseControl,
    pub authorization: GroupAgentScheduledNodeDispatchAuthorization,
    pub pricing: GroupAgentNodePricingSnapshot,
    pub provider_request: GroupAgentScheduledNodeProviderRequestRecord,
    pub provider_request_body: Vec<u8>,
    pub claim: GroupAgentScheduledNodeDispatchClaim,
    pub claim_json: String,
    pub active_lane: Option<GroupAgentScheduledNodeActiveLane>,
    pub active_lane_json: Option<String>,
    pub artifact: Option<GroupAgentScheduledNodeTerminalArtifact>,
    pub artifact_json: Option<String>,
    pub terminal_control: Option<GroupAgentScheduledNodeTerminalControl>,
    pub terminal_control_json: Option<String>,
    pub terminal_receipt: Option<GroupAgentScheduledNodeTerminalReceipt>,
    pub terminal_receipt_json: Option<String>,
    pub status: GroupAgentScheduledNodeLifecycleStatus,
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ClaimGroupAgentScheduledNodeDispatchResult {
    Claimed {
        authority: GroupAgentScheduledNodeDispatchAuthority,
    },
    AlreadyClaimed {
        inspection: GroupAgentScheduledNodeLifecycleInspection,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalizeGroupAgentScheduledNodeDispatch {
    pub v: u16,
    pub control: Option<GroupAgentScheduledNodeTerminalControl>,
    pub control_json: Option<String>,
    pub artifact_json: String,
    pub receipt: Option<GroupAgentScheduledNodeTerminalReceipt>,
    pub receipt_json: Option<String>,
    pub terminalized_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalizeGroupAgentScheduledNodeDispatchResult {
    pub v: u16,
    pub inspection: GroupAgentScheduledNodeLifecycleInspection,
}

pub trait GroupAgentScheduledNodeLifecycleStore: Send + Sync {
    fn claim_group_agent_scheduled_node_dispatch(
        &self,
        request: &ClaimGroupAgentScheduledNodeDispatch,
    ) -> Result<ClaimGroupAgentScheduledNodeDispatchResult, HubStoreError>;

    fn terminalize_group_agent_scheduled_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentScheduledNodeDispatch,
    ) -> Result<TerminalizeGroupAgentScheduledNodeDispatchResult, HubStoreError>;

    fn inspect_group_agent_scheduled_node_lifecycle(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError>;

    /// Releases the stranded Project lane of a hard-crashed claim after the
    /// operator proves the old executor stopped (local pid-liveness check).
    /// The lifecycle becomes `adjudicated`; no provider request is re-sent.
    fn adjudicate_group_agent_scheduled_node_dispatch(
        &self,
        request: &AdjudicateGroupAgentScheduledNodeDispatch,
    ) -> Result<GroupAgentScheduledNodeLifecycleInspection, HubStoreError>;
}

/// Operator-invoked hard-crash adjudication: proves (via pid liveness) that
/// the executor which claimed this dispatch has stopped, then releases the
/// lane. Never automatic, never time-based.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdjudicateGroupAgentScheduledNodeDispatch {
    pub v: u16,
    pub provider_request_id: String,
    pub adjudicated_at_ms: u64,
}

pub trait GroupAgentScheduledNodeProviderFactory: Send + Sync {
    fn resolve(
        &self,
        authorization: &GroupAgentScheduledNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentScheduledNodeResolvedDispatch, GroupAgentScheduledNodeProviderFactoryError>;

    fn build(
        &self,
        resolved: GroupAgentScheduledNodeResolvedDispatch,
        credential: String,
    ) -> Result<Box<dyn crate::PreparedModelProvider>, GroupAgentScheduledNodeProviderFactoryError>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeResolvedDispatch {
    pub authorization_sha256: String,
    pub provider_kind: crate::GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub quote: crate::GroupAgentNodePricingQuote,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeProviderFactoryError {
    pub message: String,
}

impl std::fmt::Display for GroupAgentScheduledNodeProviderFactoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentScheduledNodeProviderFactoryError {}

// ── Canonical JSON / digest codec (merged from lifecycle_codec.rs) ──────────
use sha2::{Digest, Sha256};

pub(super) fn canonical_json<T: Serialize>(
    value: &T,
) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("scheduled lifecycle JSON cannot be encoded"))
}

pub(super) fn claim_digest(
    value: &GroupAgentScheduledNodeDispatchClaim,
) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
    digest_value(GROUP_AGENT_SCHEDULED_NODE_CLAIM_DIGEST_DOMAIN, value, &[])
}

pub(super) fn claim_event_digest(
    value: &GroupAgentScheduledNodeDispatchClaimEvent,
) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
    digest_value(
        GROUP_AGENT_SCHEDULED_NODE_CLAIM_DIGEST_DOMAIN,
        value,
        &["event_sha256"],
    )
}

pub(super) fn artifact_digest(
    value: &GroupAgentScheduledNodeTerminalArtifact,
) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
    digest_value(
        GROUP_AGENT_SCHEDULED_NODE_ARTIFACT_DIGEST_DOMAIN,
        value,
        &["artifact_id", "artifact_bytes", "artifact_sha256"],
    )
}

pub(super) fn control_digest(
    value: &GroupAgentScheduledNodeTerminalControl,
) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
    digest_value(
        GROUP_AGENT_SCHEDULED_NODE_CONTROL_DIGEST_DOMAIN,
        value,
        &["snapshot_sha256"],
    )
}

pub(super) fn receipt_digest(
    value: &GroupAgentScheduledNodeTerminalReceipt,
) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
    digest_value(
        GROUP_AGENT_SCHEDULED_NODE_RECEIPT_DIGEST_DOMAIN,
        value,
        &["receipt_id", "receipt_sha256"],
    )
}

pub(super) fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn digest_value<T: Serialize>(
    domain: &[u8],
    value: &T,
    removed: &[&str],
) -> Result<String, GroupAgentScheduledNodeLifecycleValidationError> {
    let mut json = serde_json::to_value(value)
        .map_err(|_| invalid("scheduled lifecycle digest payload cannot be encoded"))?;
    let object = json
        .as_object_mut()
        .ok_or_else(|| invalid("scheduled lifecycle digest payload is not an object"))?;
    for field in removed {
        object.remove(*field);
    }
    let bytes = serde_json::to_vec(&json)
        .map_err(|_| invalid("scheduled lifecycle digest payload cannot be encoded"))?;
    Ok(digest_hex(domain, &bytes))
}

fn invalid(message: &str) -> GroupAgentScheduledNodeLifecycleValidationError {
    GroupAgentScheduledNodeLifecycleValidationError {
        message: message.into(),
    }
}
