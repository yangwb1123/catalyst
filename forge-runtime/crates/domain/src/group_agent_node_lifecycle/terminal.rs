use serde::{Deserialize, Serialize};

use super::{
    GroupAgentNodeActiveLane, GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleInspection,
    GroupAgentNodeLifecycleValidationError, GroupAgentNodeTerminalArtifact,
    GroupAgentNodeTerminalArtifactKind, GroupAgentNodeTerminalOutcome, codec, validation,
};
use crate::{
    GroupAgentGraphCorePlan, GroupAgentGraphManifest, GroupAgentGraphRunEvent,
    GroupAgentGraphRunRecord, GroupAgentGraphRunStatus, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchRequestRecord, GroupAgentNodeExecutionContract,
    GroupAgentNodeExecutionContractRecord, GroupAgentNodePricingSnapshot,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeTerminalControl {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub terminal_control_protocol_version: u16,
    pub graph_run: GroupAgentGraphRunRecord,
    pub plan: GroupAgentGraphCorePlan,
    pub manifest: GroupAgentGraphManifest,
    pub journal_events: Vec<GroupAgentGraphRunEvent>,
    pub contract_record: GroupAgentNodeExecutionContractRecord,
    pub contract: GroupAgentNodeExecutionContract,
    pub dispatch_request: GroupAgentNodeDispatchRequestRecord,
    pub provider_request_json: String,
    pub authorization: GroupAgentNodeDispatchAuthorization,
    pub pricing: GroupAgentNodePricingSnapshot,
    pub active_lane: GroupAgentNodeActiveLane,
    pub claim: GroupAgentNodeDispatchClaim,
    pub artifact: GroupAgentNodeTerminalArtifact,
    pub snapshot_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentNodeTerminalReceipt {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub terminal_receipt_protocol_version: u16,
    pub terminal_control_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub graph_run_id: String,
    pub graph_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub lane_ownership_id: String,
    pub project_lane_sha256: String,
    pub artifact_kind: GroupAgentNodeTerminalArtifactKind,
    pub artifact_id: String,
    pub artifact_sha256: String,
    pub node_outcome: GroupAgentNodeTerminalOutcome,
    pub wave_index: usize,
    pub wave_outcome: GroupAgentNodeTerminalOutcome,
    pub graph_status: GroupAgentGraphRunStatus,
    pub retry_authorized: bool,
    pub lane_release_authorized: bool,
    pub receipt_id: String,
    pub receipt_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeCoreTerminalReceiptEnvelope {
    pub receipt: GroupAgentNodeTerminalReceipt,
    pub receipt_json: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeCoreTerminalReceiptPortError {
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalizeGroupAgentNodeDispatch {
    pub v: u16,
    pub control: GroupAgentNodeTerminalControl,
    pub control_json: String,
    pub artifact_json: String,
    pub receipt: GroupAgentNodeTerminalReceipt,
    pub receipt_json: String,
    pub event: GroupAgentGraphRunEvent,
    pub event_json: String,
    pub terminalized_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalizeGroupAgentNodeDispatchResult {
    pub v: u16,
    pub inspection: GroupAgentNodeLifecycleInspection,
}

impl GroupAgentNodeTerminalControl {
    /// Strictly decodes exact compact canonical terminal control bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, GroupAgentNodeLifecycleValidationError> {
        validation::decode_exact_control(bytes)
    }

    /// Validates the complete private v4 claim snapshot and terminal artifact.
    ///
    /// # Errors
    ///
    /// Returns an error for any corrupt source, binding, identity, topology, or byte count.
    pub fn validate(&self) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_terminal_control(self)
    }

    /// Encodes the complete private control as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed control cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::canonical_json(self)
    }

    /// Encodes the control content payload excluding `snapshot_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed control payload cannot be encoded.
    pub fn canonical_payload_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::control_payload_json(self)
    }

    /// Computes the domain-separated terminal control content identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed control payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::control_digest(self)
    }
}

impl GroupAgentNodeTerminalReceipt {
    /// Strictly decodes exact compact canonical Core receipt bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, GroupAgentNodeLifecycleValidationError> {
        validation::decode_exact_receipt(bytes)
    }

    /// Validates this receipt's fixed outcomes, identity, and safe flags.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed fields or digest divergence.
    pub fn validate(&self) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_terminal_receipt(self)
    }

    /// Validates every Core receipt field against one exact terminal control.
    ///
    /// # Errors
    ///
    /// Returns an error when the Core decision does not match the terminal evidence.
    pub fn validate_against_control(
        &self,
        control: &GroupAgentNodeTerminalControl,
    ) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_receipt_against_control(self, control)
    }

    /// Encodes the complete receipt as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed receipt cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::canonical_json(self)
    }

    /// Encodes the receipt content payload excluding its final ID and digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed receipt payload cannot be encoded.
    pub fn canonical_payload_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::receipt_payload_json(self)
    }

    /// Computes the domain-separated Core receipt content identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed receipt payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::receipt_digest(self)
    }
}

impl GroupAgentNodeCoreTerminalReceiptEnvelope {
    /// Validates exact canonical receipt output against its private control input.
    ///
    /// # Errors
    ///
    /// Returns an error for noncanonical bytes or a divergent Core decision.
    pub fn validate_against_control(
        &self,
        control: &GroupAgentNodeTerminalControl,
    ) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_exact_json(&self.receipt, &self.receipt_json)?;
        self.receipt.validate_against_control(control)
    }
}

impl TerminalizeGroupAgentNodeDispatch {
    /// Validates all canonical evidence for the atomic v4-to-v5 transition.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, divergent, noncanonical, or unsafe evidence.
    pub fn validate(&self) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_terminalize_request(self)
    }
}

pub trait GroupAgentNodeCoreTerminalReceiptPort: Send + Sync {
    /// Obtains the pure Core scheduler decision for one exact private control.
    ///
    /// # Errors
    ///
    /// Returns a redacted error if the pinned Core bridge rejects or cannot decide.
    fn decide(
        &self,
        control: &GroupAgentNodeTerminalControl,
    ) -> Result<GroupAgentNodeCoreTerminalReceiptEnvelope, GroupAgentNodeCoreTerminalReceiptPortError>;
}

impl std::fmt::Display for GroupAgentNodeCoreTerminalReceiptPortError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentNodeCoreTerminalReceiptPortError {}
