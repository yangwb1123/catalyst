use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentGraphControlSnapshot, GroupAgentGraphExecutionSchedule,
    GroupAgentGraphExecutionScheduleInspection, GroupAgentGraphExecutionScheduleRecord,
    GroupAgentGraphInspection, GroupAgentGraphRunEvent, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, GroupAgentNodeExecutionBudgets, GroupAgentNodeExecutionFailurePolicy,
    GroupAgentNodeProviderKind, GroupAgentNodeSameProjectPolicy,
    GroupAgentScheduledNodeContractCandidate, GroupAgentScheduledNodeContractRecord,
    GroupAgentScheduledNodeDispatchConsentRequirement,
    GroupAgentScheduledNodeDispatchCredentialPreflight,
    GroupAgentScheduledNodeDispatchDestinationPreflight,
    GroupAgentScheduledNodeDispatchPricingPreflight,
    GroupAgentScheduledNodeDispatchProjectLaneClaim,
    GroupAgentScheduledNodeDispatchProviderHealthCheck,
    GroupAgentScheduledNodeProviderRequestInspection, GroupAgentScheduledNodeProviderRequestRecord,
    GroupAgentScheduledNodeTerminalArtifact, GroupAgentScheduledNodeTerminalReceipt, HubStoreError,
    ScheduledGraphProgressSnapshot, ScheduledGraphReconcileDecision,
};

#[path = "scheduled_ready_dispatch_release_codec.rs"]
mod codec;
#[path = "scheduled_ready_dispatch_release_validation.rs"]
mod validation;

pub const GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_VERSION: u16 = 2;
pub const GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION: u16 = 2;
pub const GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_VERSION: u16 = 2;
pub const GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION: u16 = 2;
pub const GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_CONSENT_CONTRACT_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-ready-node-dispatch-release-control.v2\0";
pub const GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-ready-node-dispatch-authorization.v2\0";
pub const MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_RELEASE_CONTROL_BYTES: usize =
    64 * 1024 * 1024;
pub const MAX_GROUP_AGENT_SCHEDULED_READY_NODE_DISPATCH_AUTHORIZATION_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledReadyNodeDispatchReleaseControl {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub release_control_protocol_version: u16,
    pub graph_run: GroupAgentGraphRunRecord,
    pub journal_events: Vec<GroupAgentGraphRunEvent>,
    pub control_snapshot: GroupAgentGraphControlSnapshot,
    pub schedule_record: GroupAgentGraphExecutionScheduleRecord,
    pub schedule: GroupAgentGraphExecutionSchedule,
    pub progress_snapshot: ScheduledGraphProgressSnapshot,
    pub reconcile_decision: ScheduledGraphReconcileDecision,
    pub scheduled_contract_record: GroupAgentScheduledNodeContractRecord,
    pub scheduled_contract: GroupAgentScheduledNodeContractCandidate,
    pub direct_predecessor_receipts: Vec<GroupAgentScheduledNodeTerminalReceipt>,
    pub predecessor_content_artifact: Option<GroupAgentScheduledNodeTerminalArtifact>,
    pub provider_request: GroupAgentScheduledNodeProviderRequestRecord,
    pub provider_request_json: String,
    pub snapshot_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledReadyNodeDispatchAtomicTransitionRequirement {
    ExactProgressSnapshotSelectedNodeAdmissionReleaseAndLaneClaim,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentScheduledReadyNodeSuccessorRequirement {
    ExactOrderedDirectPredecessorTerminalReceiptsBeforeSuccessor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentScheduledReadyNodeDispatchReleaseRequirements {
    pub consent: GroupAgentScheduledNodeDispatchConsentRequirement,
    pub consent_contract_version: u16,
    pub credential_preflight: GroupAgentScheduledNodeDispatchCredentialPreflight,
    pub destination_preflight: GroupAgentScheduledNodeDispatchDestinationPreflight,
    pub pricing_preflight: GroupAgentScheduledNodeDispatchPricingPreflight,
    pub project_lane_claim: GroupAgentScheduledNodeDispatchProjectLaneClaim,
    pub provider_health_check: GroupAgentScheduledNodeDispatchProviderHealthCheck,
    pub atomic_transition: GroupAgentScheduledReadyNodeDispatchAtomicTransitionRequirement,
    pub successor: GroupAgentScheduledReadyNodeSuccessorRequirement,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledReadyNodeDispatchAuthorization {
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
    pub progress_snapshot_sha256: String,
    pub reconcile_decision_sha256: String,
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
    pub release_requirements: GroupAgentScheduledReadyNodeDispatchReleaseRequirements,
    pub maximum_future_node_releases: u16,
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
pub struct ScheduledReadyNodeReleaseSource {
    pub progress_snapshot: ScheduledGraphProgressSnapshot,
    pub graph_run: GroupAgentGraphRunInspection,
    pub graph: GroupAgentGraphInspection,
    pub schedule: GroupAgentGraphExecutionScheduleInspection,
    pub selected_provider_request: GroupAgentScheduledNodeProviderRequestInspection,
    pub direct_predecessor_receipts: Vec<GroupAgentScheduledNodeTerminalReceipt>,
    pub predecessor_content_artifact: Option<GroupAgentScheduledNodeTerminalArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledReadyNodeDispatchReleaseValidationError {
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScheduledReadyNodeReleasePortError {
    Unavailable,
    InvalidAuthorization,
}

pub trait ScheduledReadyNodeReleasePort: Send + Sync {
    /// Authorizes one exactly sealed ready-node release control.
    ///
    /// # Errors
    /// Returns an error when the authorizer is unavailable or rejects the control.
    fn authorize(
        &self,
        control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ) -> Result<GroupAgentScheduledReadyNodeDispatchAuthorization, ScheduledReadyNodeReleasePortError>;
}

pub trait ScheduledReadyNodeReleaseStore: Send + Sync {
    /// Loads one release-ready aggregate at the caller's exact progress snapshot.
    ///
    /// # Errors
    /// Returns a store error when evidence is absent, stale, corrupt, or unavailable.
    fn inspect_scheduled_ready_node_release(
        &self,
        graph_run_id: &str,
        expected_snapshot_sha256: &str,
        execution_ordinal: usize,
        node_id: &str,
    ) -> Result<ScheduledReadyNodeReleaseSource, HubStoreError>;
}

impl GroupAgentScheduledReadyNodeDispatchReleaseControl {
    /// Computes the control identity and validates every exact source binding.
    ///
    /// # Errors
    /// Returns a validation error when already sealed or any source is invalid.
    pub fn seal(
        mut self,
    ) -> Result<Self, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        if !self.snapshot_sha256.is_empty() {
            return Err(validation::invalid(
                "ready release control is already sealed",
            ));
        }
        self.snapshot_sha256 = self.expected_sha256()?;
        self.validate()?;
        Ok(self)
    }

    /// Validates the sealed control, its source closure, and its digest.
    ///
    /// # Errors
    /// Returns a validation error for any invalid or inconsistent field.
    pub fn validate(
        &self,
    ) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        validation::validate_release_control(self)
    }

    /// Decodes only exact canonical ready release-control JSON.
    ///
    /// # Errors
    /// Returns a validation error for invalid, oversized, or noncanonical JSON.
    pub fn decode_exact(
        json: &str,
    ) -> Result<Self, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        validation::decode_release_control(json.as_bytes())
    }

    /// Decodes only exact canonical ready release-control bytes.
    ///
    /// # Errors
    /// Returns a validation error for invalid, oversized, or noncanonical bytes.
    pub fn decode_exact_bytes(
        bytes: &[u8],
    ) -> Result<Self, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        validation::decode_release_control(bytes)
    }

    /// Encodes the complete control as canonical JSON.
    ///
    /// # Errors
    /// Returns a validation error if serialization fails.
    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated digest of the control payload.
    ///
    /// # Errors
    /// Returns a validation error if the digest payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        codec::release_control_digest(self)
    }
}

impl GroupAgentScheduledReadyNodeDispatchAuthorization {
    /// Computes the authorization identity and validates its fixed policy.
    ///
    /// # Errors
    /// Returns a validation error when already sealed or any field is invalid.
    pub fn seal(
        mut self,
    ) -> Result<Self, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        if !self.authorization_id.is_empty() || !self.authorization_sha256.is_empty() {
            return Err(validation::invalid("ready authorization is already sealed"));
        }
        self.authorization_sha256 = self.expected_sha256()?;
        self.authorization_id =
            group_agent_scheduled_ready_node_dispatch_authorization_id(&self.authorization_sha256);
        self.validate()?;
        Ok(self)
    }

    /// Validates the sealed authorization and its digest identity.
    ///
    /// # Errors
    /// Returns a validation error for any invalid or inconsistent field.
    pub fn validate(
        &self,
    ) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        validation::validate_authorization(self)
    }

    /// Validates every authorization field against one exact release control.
    ///
    /// # Errors
    /// Returns a validation error when the authorization or binding disagrees.
    pub fn validate_against_release_control(
        &self,
        control: &GroupAgentScheduledReadyNodeDispatchReleaseControl,
    ) -> Result<(), GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        validation::validate_authorization_against_release_control(self, control)
    }

    /// Decodes only exact canonical ready authorization JSON.
    ///
    /// # Errors
    /// Returns a validation error for invalid, oversized, or noncanonical JSON.
    pub fn decode_exact(
        json: &str,
    ) -> Result<Self, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        validation::decode_authorization(json.as_bytes())
    }

    /// Decodes only exact canonical ready authorization bytes.
    ///
    /// # Errors
    /// Returns a validation error for invalid, oversized, or noncanonical bytes.
    pub fn decode_exact_bytes(
        bytes: &[u8],
    ) -> Result<Self, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        validation::decode_authorization(bytes)
    }

    /// Encodes the complete authorization as canonical JSON.
    ///
    /// # Errors
    /// Returns a validation error if serialization fails.
    pub fn canonical_json(
        &self,
    ) -> Result<String, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        codec::canonical_json(self)
    }

    /// Encodes the digest payload without identity fields as canonical JSON.
    ///
    /// # Errors
    /// Returns a validation error if serialization fails.
    pub fn canonical_payload_json(
        &self,
    ) -> Result<String, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        codec::authorization_payload_json(self)
    }

    /// Computes the domain-separated digest of the authorization payload.
    ///
    /// # Errors
    /// Returns a validation error if the digest payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledReadyNodeDispatchReleaseValidationError> {
        codec::authorization_digest(self)
    }
}

#[must_use]
pub fn group_agent_scheduled_ready_node_dispatch_authorization_id(sha256: &str) -> String {
    format!("scheduled-ready-node-dispatch-authorization-{sha256}")
}

impl std::fmt::Display for GroupAgentScheduledReadyNodeDispatchReleaseValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentScheduledReadyNodeDispatchReleaseValidationError {}

#[cfg(test)]
#[path = "scheduled_ready_dispatch_release_tests.rs"]
mod tests;
