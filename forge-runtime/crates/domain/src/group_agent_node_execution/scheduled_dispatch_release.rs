use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentGraphControlSnapshot, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleRecord, GroupAgentGraphRunEvent, GroupAgentGraphRunRecord,
    GroupAgentNodeExecutionBudgets, GroupAgentNodeExecutionFailurePolicy,
    GroupAgentNodeProviderKind, GroupAgentNodeSameProjectPolicy,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractRecord,
    GroupAgentScheduledNodeProviderRequestRecord,
};

#[path = "scheduled_dispatch_release_codec.rs"]
mod codec;
#[path = "scheduled_dispatch_release_validation.rs"]
mod validation;

pub const GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_DISPATCH_CONSENT_CONTRACT_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-dispatch-release-control.v1\0";
pub const GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-dispatch-authorization.v1\0";
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_RELEASE_CONTROL_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_DISPATCH_AUTHORIZATION_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeDispatchReleaseControl {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub release_control_protocol_version: u16,
    pub graph_run: GroupAgentGraphRunRecord,
    pub journal_events: Vec<GroupAgentGraphRunEvent>,
    pub control_snapshot: GroupAgentGraphControlSnapshot,
    pub schedule_record: GroupAgentGraphExecutionScheduleRecord,
    pub schedule: GroupAgentGraphExecutionSchedule,
    pub scheduled_contract_record: GroupAgentScheduledNodeContractRecord,
    pub scheduled_contract: GroupAgentScheduledNodeContractCandidate,
    pub provider_request: GroupAgentScheduledNodeProviderRequestRecord,
    pub provider_request_json: String,
    pub snapshot_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeDispatchConsentRequirement {
    FreshOffMachine,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeDispatchCredentialPreflight {
    HeaderSafeEnvironment,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeDispatchDestinationPreflight {
    ExactRegisteredDestination,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeDispatchPricingPreflight {
    ExactSnapshotWithinMaxCost,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeDispatchProjectLaneClaim {
    GlobalExclusiveUntilTerminal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeDispatchProviderHealthCheck {
    Forbidden,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeSuccessorRequirement {
    VerifiedIntermediateTerminalReceiptBeforeSuccessor,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledNodeDispatchAtomicTransitionRequirement {
    ExactPristineHeadAdmissionReleaseAndLaneClaim,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledNodeDispatchReleaseRequirements {
    pub consent: GroupAgentScheduledNodeDispatchConsentRequirement,
    pub consent_contract_version: u16,
    pub credential_preflight: GroupAgentScheduledNodeDispatchCredentialPreflight,
    pub destination_preflight: GroupAgentScheduledNodeDispatchDestinationPreflight,
    pub pricing_preflight: GroupAgentScheduledNodeDispatchPricingPreflight,
    pub project_lane_claim: GroupAgentScheduledNodeDispatchProjectLaneClaim,
    pub provider_health_check: GroupAgentScheduledNodeDispatchProviderHealthCheck,
    pub atomic_transition: GroupAgentScheduledNodeDispatchAtomicTransitionRequirement,
    pub successor: GroupAgentScheduledNodeSuccessorRequirement,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeDispatchAuthorization {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub dispatch_authorization_protocol_version: u16,
    pub graph_run_id: String,
    pub graph_id: String,
    pub group_run_id: String,
    pub group_id: String,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub core_plan_sha256: String,
    pub control_snapshot_sha256: String,
    pub release_control_snapshot_sha256: String,
    pub schedule_id: String,
    pub schedule_sha256: String,
    pub scheduled_contract_id: String,
    pub scheduled_contract_sha256: String,
    pub scheduled_provider_request_id: String,
    pub scheduled_provider_request_sha256: String,
    pub logical_request_id: String,
    pub logical_request_sha256: String,
    pub request_body_sha256: String,
    pub request_body_bytes: usize,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub execution_ordinal: usize,
    pub node_id: String,
    pub attempt: u16,
    pub project_id: String,
    pub project_lane_sha256: String,
    pub same_project_policy: GroupAgentNodeSameProjectPolicy,
    pub provider_kind: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub budgets: GroupAgentNodeExecutionBudgets,
    pub release_requirements: GroupAgentScheduledNodeDispatchReleaseRequirements,
    pub failure: GroupAgentNodeExecutionFailurePolicy,
    pub lifecycle_contract_admission_authorized: bool,
    pub execution_authority_release_authorized: bool,
    pub dispatch_authority_release_authorized: bool,
    pub scheduled_contract_candidate_present: bool,
    pub provider_request_prepared: bool,
    pub lifecycle_contract_admitted: bool,
    pub execution_authority_released: bool,
    pub dispatch_authority_released: bool,
    pub project_lane_claimed: bool,
    pub provider_request_sent: bool,
    pub progress_observed: bool,
    pub terminal_receipt_recorded: bool,
    pub successor_advance_authorized: bool,
    pub authorization_id: String,
    pub authorization_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeDispatchReleaseValidationError {
    pub message: String,
}

impl GroupAgentScheduledNodeDispatchReleaseControl {
    /// Validates this passive control's domain structure, identities, and exact-byte bindings.
    ///
    /// Provider-specific request semantics are intentionally enforced by the
    /// application request-codec port before a production control is exported;
    /// this domain check alone is not provider-codec proof or release authority.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed state, bytes, bindings, policy, or digest.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
        validation::validate_release_control(self)
    }

    /// Strictly decodes one exact compact canonical release-control document.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, malformed, noncanonical, or
    /// self-inconsistent input. Input bytes are never included in errors.
    pub fn decode_exact(
        json: &str,
    ) -> Result<Self, GroupAgentScheduledNodeDispatchReleaseValidationError> {
        validation::decode_release_control(json)
    }

    /// Encodes the complete snapshot as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed snapshot cannot be encoded.
    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated snapshot digest excluding `snapshot_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
        codec::release_control_digest(self)
    }
}

impl GroupAgentScheduledNodeDispatchAuthorization {
    /// Validates the authorization envelope and its self-addressed identity.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed fields, unsafe assertions, or digest divergence.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
        validation::validate_authorization(self)
    }

    /// Validates every field against one domain-valid scheduled release control.
    ///
    /// Production callers must use the application service, which first
    /// revalidates the stored request with the configured provider codec. This
    /// cross-binding check alone is not dispatch authority.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot or any authorization binding disagrees.
    pub fn validate_against_release_control(
        &self,
        control: &GroupAgentScheduledNodeDispatchReleaseControl,
    ) -> Result<(), GroupAgentScheduledNodeDispatchReleaseValidationError> {
        validation::validate_authorization_against_release_control(self, control)
    }

    /// Strictly decodes one exact compact canonical authorization document.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, malformed, noncanonical, or
    /// self-inconsistent input. Input bytes are never included in errors.
    pub fn decode_exact(
        json: &str,
    ) -> Result<Self, GroupAgentScheduledNodeDispatchReleaseValidationError> {
        validation::decode_authorization(json)
    }

    /// Encodes the complete authorization as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed authorization cannot be encoded.
    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
        codec::canonical_json(self)
    }

    /// Encodes the exact identity payload excluding authorization ID and digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded.
    pub fn canonical_payload_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
        codec::authorization_payload_json(self)
    }

    /// Computes the domain-separated digest excluding authorization ID and digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeDispatchReleaseValidationError> {
        codec::authorization_digest(self)
    }
}

/// Derives the stable scheduled authorization ID from its complete content digest.
#[must_use]
pub fn group_agent_scheduled_node_dispatch_authorization_id(sha256: &str) -> String {
    format!("scheduled-node-dispatch-authorization-{sha256}")
}

impl std::fmt::Display for GroupAgentScheduledNodeDispatchReleaseValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentScheduledNodeDispatchReleaseValidationError {}

#[cfg(test)]
#[path = "scheduled_dispatch_release_tests.rs"]
mod tests;
