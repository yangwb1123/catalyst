use serde::{Deserialize, Serialize};

use super::{
    GroupAgentNodeLifecycleValidationError, GroupAgentNodeTerminalArtifact,
    GroupAgentNodeTerminalReceipt, TerminalizeGroupAgentNodeDispatch,
    TerminalizeGroupAgentNodeDispatchResult, codec, validation,
};
use crate::{
    GroupAgentGraphRunEvent, GroupAgentGraphRunInspection, GroupAgentNodeDispatchAuthorization,
    GroupAgentNodeDispatchReleaseControl, GroupAgentNodeDispatchRequestRecord,
    GroupAgentNodePricingSnapshot, HubStoreError,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeDispatchClaim {
    pub v: u16,
    pub graph_run_id: String,
    pub dispatch_id: String,
    pub authorization_id: String,
    pub authorization_sha256: String,
    pub dispatch_request_id: String,
    pub dispatch_request_sha256: String,
    pub logical_request_sha256: String,
    pub request_body_sha256: String,
    pub request_body_bytes: usize,
    pub pricing_snapshot_sha256: String,
    pub node_id: String,
    pub attempt: u16,
    pub max_cost_usd_micros: u64,
    pub consent_contract_version: u16,
    pub lane_ownership_id: String,
    pub project_lane_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub claim_event_sha256: String,
    pub released_at_ms: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeActiveLane {
    pub v: u16,
    pub project_lane_sha256: String,
    pub lane_ownership_id: String,
    pub graph_run_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub dispatch_id: String,
    pub claim_event_sha256: String,
    pub claimed_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimGroupAgentNodeDispatch {
    pub v: u16,
    pub release_control: GroupAgentNodeDispatchReleaseControl,
    pub release_control_json: String,
    pub authorization: GroupAgentNodeDispatchAuthorization,
    pub authorization_json: String,
    pub pricing: GroupAgentNodePricingSnapshot,
    pub pricing_json: String,
    pub claim: GroupAgentNodeDispatchClaim,
    pub claim_json: String,
    pub active_lane: GroupAgentNodeActiveLane,
    pub active_lane_json: String,
    pub event: GroupAgentGraphRunEvent,
    pub event_json: String,
}

#[derive(Debug, Eq, PartialEq)]
pub struct GroupAgentNodeDispatchAuthority {
    v: u16,
    claim: GroupAgentNodeDispatchClaim,
    request_body: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeLifecycleInspection {
    pub v: u16,
    pub graph_run: GroupAgentGraphRunInspection,
    pub claim: GroupAgentNodeDispatchClaim,
    pub claim_json: String,
    pub active_lane: Option<GroupAgentNodeActiveLane>,
    pub active_lane_json: Option<String>,
    pub artifact: Option<GroupAgentNodeTerminalArtifact>,
    pub artifact_json: Option<String>,
    pub terminal_receipt: Option<GroupAgentNodeTerminalReceipt>,
    pub terminal_receipt_json: Option<String>,
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum ClaimGroupAgentNodeDispatchResult {
    Claimed {
        authority: GroupAgentNodeDispatchAuthority,
    },
    AlreadyClaimed {
        inspection: GroupAgentNodeLifecycleInspection,
    },
}

impl GroupAgentNodeDispatchClaim {
    /// Strictly decodes exact compact canonical claim bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, or invalid input.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, GroupAgentNodeLifecycleValidationError> {
        validation::decode_exact_claim(bytes)
    }

    /// Validates one immutable claim record and its content identities.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or unsafe claim metadata.
    pub fn validate(&self) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_claim(self)
    }

    /// Encodes the complete claim as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the claim cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::canonical_json(self)
    }
}

impl GroupAgentNodeActiveLane {
    /// Strictly decodes exact compact canonical lane bytes.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, noncanonical, or invalid input.
    pub fn decode_exact(bytes: &[u8]) -> Result<Self, GroupAgentNodeLifecycleValidationError> {
        validation::decode_exact_active_lane(bytes)
    }

    /// Validates this active lane and every immutable claim binding.
    ///
    /// # Errors
    ///
    /// Returns an error when the lane is malformed or disagrees with the claim.
    pub fn validate_against_claim(
        &self,
        claim: &GroupAgentNodeDispatchClaim,
    ) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_active_lane(self, claim)
    }

    /// Encodes the complete lane as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the lane cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeLifecycleValidationError> {
        codec::canonical_json(self)
    }
}

impl ClaimGroupAgentNodeDispatch {
    /// Validates the complete canonical v3-to-v4 claim transaction input.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, divergent, noncanonical, or unsafe input.
    pub fn validate(&self) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_claim_request(self)
    }
}

impl GroupAgentNodeDispatchAuthority {
    /// Creates the non-cloneable capability for exact persisted request bytes.
    ///
    /// # Errors
    ///
    /// Returns an error unless the claim, seq-4 event, record, and bytes agree.
    pub fn new(
        dispatch_request: &GroupAgentNodeDispatchRequestRecord,
        claim: GroupAgentNodeDispatchClaim,
        claim_event: &GroupAgentGraphRunEvent,
        request_body: Vec<u8>,
    ) -> Result<Self, GroupAgentNodeLifecycleValidationError> {
        validation::validate_dispatch_authority(
            dispatch_request,
            &claim,
            claim_event,
            &request_body,
        )?;
        Ok(Self {
            v: claim.v,
            claim,
            request_body,
        })
    }

    #[must_use]
    pub const fn version(&self) -> u16 {
        self.v
    }

    #[must_use]
    pub const fn claim(&self) -> &GroupAgentNodeDispatchClaim {
        &self.claim
    }

    /// Consumes the sole local authority and releases its exact request body.
    #[must_use]
    pub fn into_parts(self) -> (GroupAgentNodeDispatchClaim, Vec<u8>) {
        (self.claim, self.request_body)
    }
}

impl GroupAgentNodeLifecycleInspection {
    /// Validates one fully reconstructed v4 or v5 lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt bytes, journal, lane, artifact, or receipt state.
    pub fn validate(&self) -> Result<(), GroupAgentNodeLifecycleValidationError> {
        validation::validate_lifecycle_inspection(self)
    }
}

pub trait GroupAgentNodeLifecycleStore: Send + Sync {
    /// Atomically claims the global lane and returns authority only to the winner.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn claim_group_agent_node_dispatch(
        &self,
        request: &ClaimGroupAgentNodeDispatch,
    ) -> Result<ClaimGroupAgentNodeDispatchResult, HubStoreError>;

    /// Atomically persists terminal evidence, appends seq 5, and releases the lane.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn terminalize_group_agent_node_dispatch(
        &self,
        request: &TerminalizeGroupAgentNodeDispatch,
    ) -> Result<TerminalizeGroupAgentNodeDispatchResult, HubStoreError>;

    /// Fully reconstructs and validates one claimed or terminal lifecycle.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or storage error.
    fn inspect_group_agent_node_lifecycle(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentNodeLifecycleInspection, HubStoreError>;
}
