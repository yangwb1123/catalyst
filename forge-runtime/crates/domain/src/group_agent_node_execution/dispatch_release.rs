use serde::{Deserialize, Serialize};

use crate::{
    GroupAgentGraphCorePlan, GroupAgentGraphManifest, GroupAgentGraphRunEvent,
    GroupAgentGraphRunRecord, GroupAgentNodeDispatchRequestRecord, GroupAgentNodeExecutionBudgets,
    GroupAgentNodeExecutionContract, GroupAgentNodeExecutionContractRecord,
    GroupAgentNodeExecutionFailurePolicy, GroupAgentNodeProviderKind,
    GroupAgentNodeSameProjectPolicy,
};

#[path = "dispatch_release_codec.rs"]
mod codec;
#[path = "dispatch_release_validation.rs"]
mod validation;

pub const GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_DISPATCH_CONSENT_CONTRACT_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-dispatch-release-control.v1\0";
pub const GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-dispatch-authorization.v1\0";
pub const MAX_GROUP_AGENT_NODE_DISPATCH_RELEASE_CONTROL_BYTES: usize = 48 * 1024 * 1024;
pub const MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeDispatchReleaseControl {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub release_control_protocol_version: u16,
    pub graph_run: GroupAgentGraphRunRecord,
    pub plan: GroupAgentGraphCorePlan,
    pub manifest: GroupAgentGraphManifest,
    pub journal_events: Vec<GroupAgentGraphRunEvent>,
    pub contract_record: GroupAgentNodeExecutionContractRecord,
    pub contract: GroupAgentNodeExecutionContract,
    pub dispatch_request: GroupAgentNodeDispatchRequestRecord,
    pub provider_request_json: String,
    pub snapshot_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeDispatchConsentRequirement {
    FreshOffMachine,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeDispatchCredentialPreflight {
    HeaderSafeEnvironment,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeDispatchDestinationPreflight {
    ExactRegisteredDestination,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeDispatchPricingPreflight {
    ExactSnapshotWithinMaxCost,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeDispatchProjectLaneClaim {
    GlobalExclusiveUntilTerminal,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentNodeDispatchProviderHealthCheck {
    Forbidden,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeDispatchReleaseRequirements {
    pub consent: GroupAgentNodeDispatchConsentRequirement,
    pub consent_contract_version: u16,
    pub credential_preflight: GroupAgentNodeDispatchCredentialPreflight,
    pub destination_preflight: GroupAgentNodeDispatchDestinationPreflight,
    pub pricing_preflight: GroupAgentNodeDispatchPricingPreflight,
    pub project_lane_claim: GroupAgentNodeDispatchProjectLaneClaim,
    pub provider_health_check: GroupAgentNodeDispatchProviderHealthCheck,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentNodeDispatchAuthorization {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub dispatch_authorization_protocol_version: u16,
    pub graph_run_id: String,
    pub graph_id: String,
    pub group_run_id: String,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub core_plan_sha256: String,
    pub release_control_snapshot_sha256: String,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub contract_id: String,
    pub contract_sha256: String,
    pub dispatch_request_id: String,
    pub dispatch_request_sha256: String,
    pub logical_request_sha256: String,
    pub request_body_sha256: String,
    pub request_body_bytes: usize,
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
    pub release_requirements: GroupAgentNodeDispatchReleaseRequirements,
    pub failure: GroupAgentNodeExecutionFailurePolicy,
    pub execution_contract_present: bool,
    pub dispatch_request_present: bool,
    pub dispatch_authority_release_authorized: bool,
    pub dispatch_authority_released: bool,
    pub authorization_id: String,
    pub authorization_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeDispatchReleaseValidationError {
    pub message: String,
}

impl GroupAgentNodeDispatchReleaseControl {
    /// Fully validates this exact, passive release-control snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed state, bytes, bindings, policy, or digest.
    pub fn validate(&self) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
        validation::validate_release_control(self)
    }

    /// Encodes the complete snapshot as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed snapshot cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated snapshot digest excluding `snapshot_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
        codec::release_control_digest(self)
    }
}

impl GroupAgentNodeDispatchAuthorization {
    /// Validates the authorization envelope and its self-addressed identity.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed fields, unsafe assertions, or digest divergence.
    pub fn validate(&self) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
        validation::validate_authorization(self)
    }

    /// Validates every authorization field against one exact release-control snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot or any authorization binding disagrees.
    pub fn validate_against_release_control(
        &self,
        control: &GroupAgentNodeDispatchReleaseControl,
    ) -> Result<(), GroupAgentNodeDispatchReleaseValidationError> {
        validation::validate_authorization_against_release_control(self, control)
    }

    /// Encodes the complete authorization as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed authorization cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
        codec::canonical_json(self)
    }

    /// Encodes the exact identity payload excluding authorization ID and digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded.
    pub fn canonical_payload_json(
        &self,
    ) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
        codec::authorization_payload_json(self)
    }

    /// Computes the domain-separated digest excluding authorization ID and digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeDispatchReleaseValidationError> {
        codec::authorization_digest(self)
    }
}

/// Derives a stable authorization ID from the complete authorization identity.
#[must_use]
pub fn group_agent_node_dispatch_authorization_id(authorization_sha256: &str) -> String {
    format!("node-dispatch-authorization-{authorization_sha256}")
}

impl std::fmt::Display for GroupAgentNodeDispatchReleaseValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentNodeDispatchReleaseValidationError {}

#[cfg(test)]
#[path = "dispatch_release_tests.rs"]
mod tests;
