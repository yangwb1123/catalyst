use serde::{Deserialize, Serialize};

use super::{
    GroupAgentNodeActiveLane, GroupAgentNodeDispatchClaim, GroupAgentNodeLifecycleValidationError,
    GroupAgentNodeTerminalArtifactKind, GroupAgentNodeTerminalClassification, codec, validation,
};
use crate::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeExecutionContract,
    GroupAgentNodePricingSnapshot,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentNodeTerminalArtifact {
    pub v: u16,
    pub terminal_artifact_protocol_version: u16,
    pub artifact_kind: GroupAgentNodeTerminalArtifactKind,
    pub graph_run_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub claim_event_sha256: String,
    pub authorization_sha256: String,
    pub dispatch_request_sha256: String,
    pub logical_request_sha256: String,
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

impl GroupAgentNodeTerminalArtifact {
    /// Strictly decodes exact compact canonical artifact bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, oversized, or invalid input.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, GroupAgentNodeLifecycleValidationError> {
        validation::decode_exact_artifact(bytes)
    }

    /// Validates the artifact shape, identity, bounds, and fixed classification.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed fields, inconsistent flags, or digest drift.
    pub fn validate(&self) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_artifact(self)
    }

    /// Validates all immutable claim, authorization, pricing, and contract bindings.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal evidence is not valid for the claimed request.
    pub fn validate_against_claim(
        &self,
        claim: &GroupAgentNodeDispatchClaim,
        active_lane: &GroupAgentNodeActiveLane,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
        contract: &GroupAgentNodeExecutionContract,
    ) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_artifact_against_claim(
            self,
            claim,
            active_lane,
            authorization,
            pricing,
            contract,
        )
    }

    /// Encodes the complete artifact as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed artifact cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::canonical_json(self)
    }

    /// Encodes the content identity payload before the final three identity fields.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed artifact payload cannot be encoded.
    pub fn canonical_payload_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::artifact_payload_json(self)
    }

    /// Computes the domain-separated artifact content identity.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed artifact payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::artifact_digest(self)
    }
}
