use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{GroupAgentNodeProviderKind, GroupAgentScheduledNodeContractInspection, HubStoreError};

#[path = "scheduled_provider_request_validation.rs"]
mod validation;

pub const GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_VERSION: u16 = 1;
pub const GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-scheduled-node-provider-request.v1\0";
pub const MAX_GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_LIST_LIMIT: usize = 100;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct GroupAgentScheduledNodeProviderRequestRecord {
    pub v: u16,
    pub provider_request_id: String,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub scheduled_contract_id: String,
    pub execution_ordinal: usize,
    pub node_id: String,
    pub attempt: u16,
    pub scheduled_contract_sha256: String,
    pub logical_request_id: String,
    pub logical_request_sha256: String,
    pub schedule_sha256: String,
    pub project_lane_sha256: String,
    pub provider: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub provider_request_sha256: String,
    pub provider_request_bytes: usize,
    pub prepared_request_sha256: String,
    pub codec_protocol_version: u16,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub provider_request_prepared: bool,
    pub provider_request_sent: bool,
    pub lifecycle_contract_admitted: bool,
    pub execution_authority_released: bool,
    pub dispatch_authority_released: bool,
    pub project_lane_claimed: bool,
    pub progress_observed: bool,
    pub successor_advance_authorized: bool,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct PrepareGroupAgentScheduledNodeProviderRequest {
    pub v: u16,
    pub provider_request_id: String,
    pub graph_run_id: String,
    pub schedule_id: String,
    pub scheduled_contract_id: String,
    pub execution_ordinal: usize,
    pub node_id: String,
    pub attempt: u16,
    pub scheduled_contract_sha256: String,
    pub logical_request_id: String,
    pub logical_request_sha256: String,
    pub schedule_sha256: String,
    pub project_lane_sha256: String,
    pub provider: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: String,
    pub pricing_snapshot_sha256: String,
    pub provider_request_body: Vec<u8>,
    pub provider_request_sha256: String,
    pub prepared_request_sha256: String,
    pub codec_protocol_version: u16,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub provider_request_prepared: bool,
    pub provider_request_sent: bool,
    pub lifecycle_contract_admitted: bool,
    pub execution_authority_released: bool,
    pub dispatch_authority_released: bool,
    pub project_lane_claimed: bool,
    pub progress_observed: bool,
    pub successor_advance_authorized: bool,
    pub idempotency_key: String,
    pub prepared_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareGroupAgentScheduledNodeProviderRequestDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentScheduledNodeProviderRequestResult {
    pub v: u16,
    pub disposition: PrepareGroupAgentScheduledNodeProviderRequestDisposition,
    pub inspection: GroupAgentScheduledNodeProviderRequestInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeProviderRequestInspection {
    pub v: u16,
    pub record: GroupAgentScheduledNodeProviderRequestRecord,
    pub provider_request_body: Vec<u8>,
    pub scheduled_contract: GroupAgentScheduledNodeContractInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentScheduledNodeProviderRequestValidationError {
    pub message: String,
}

impl GroupAgentScheduledNodeProviderRequestRecord {
    /// Validates content-free metadata for one passive prepared request.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identities, bounds, digests, or effect flags.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeProviderRequestValidationError> {
        validation::validate_record(self)
    }

    /// Recomputes the semantic envelope digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed canonical payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeProviderRequestValidationError> {
        prepared_request_digest(&PreparedRequestDigestFields::from_record(self))
    }

    /// Encodes the exact version-1 content-addressed payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed canonical payload cannot be encoded.
    pub fn canonical_payload_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeProviderRequestValidationError> {
        prepared_request_payload_json(&PreparedRequestDigestFields::from_record(self))
    }
}

impl PrepareGroupAgentScheduledNodeProviderRequest {
    /// Validates one exact passive request-preparation candidate.
    ///
    /// # Errors
    ///
    /// Returns an error for substituted source, body, identity, or effect fields.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeProviderRequestValidationError> {
        validation::validate_prepare(self)
    }

    /// Recomputes the semantic envelope digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed canonical payload cannot be encoded.
    pub fn expected_sha256(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeProviderRequestValidationError> {
        prepared_request_digest(&PreparedRequestDigestFields::from_prepare(self))
    }

    /// Encodes the exact version-1 content-addressed payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed canonical payload cannot be encoded.
    pub fn canonical_payload_json(
        &self,
    ) -> Result<String, GroupAgentScheduledNodeProviderRequestValidationError> {
        prepared_request_payload_json(&PreparedRequestDigestFields::from_prepare(self))
    }
}

impl GroupAgentScheduledNodeProviderRequestInspection {
    /// Validates exact body bytes and every stored scheduled-contract binding.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt or inconsistent durable state.
    pub fn validate(&self) -> Result<(), GroupAgentScheduledNodeProviderRequestValidationError> {
        validation::validate_inspection(self)
    }
}

/// Derives a stable content ID from the prepared-request envelope digest.
#[must_use]
pub fn group_agent_scheduled_node_provider_request_id(prepared_request_sha256: &str) -> String {
    format!("scheduled-node-provider-request-{prepared_request_sha256}")
}

#[derive(Serialize)]
#[allow(clippy::struct_excessive_bools)]
struct PreparedRequestDigestFields<'a> {
    v: u16,
    codec_protocol_version: u16,
    graph_run_id: &'a str,
    schedule_id: &'a str,
    schedule_sha256: &'a str,
    scheduled_contract_id: &'a str,
    scheduled_contract_sha256: &'a str,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
    execution_ordinal: usize,
    node_id: &'a str,
    attempt: u16,
    project_lane_sha256: &'a str,
    provider_kind: GroupAgentNodeProviderKind,
    endpoint: &'a str,
    model: &'a str,
    destination_sha256: &'a str,
    logical_request_id: &'a str,
    logical_request_sha256: &'a str,
    pricing_snapshot_sha256: &'a str,
    request_body_bytes: usize,
    request_body_sha256: &'a str,
    provider_request_prepared: bool,
    provider_request_sent: bool,
    lifecycle_contract_admitted: bool,
    execution_authority_released: bool,
    dispatch_authority_released: bool,
    project_lane_claimed: bool,
    progress_observed: bool,
    successor_advance_authorized: bool,
}

impl<'a> PreparedRequestDigestFields<'a> {
    fn from_record(record: &'a GroupAgentScheduledNodeProviderRequestRecord) -> Self {
        Self {
            v: record.v,
            codec_protocol_version: record.codec_protocol_version,
            graph_run_id: &record.graph_run_id,
            schedule_id: &record.schedule_id,
            schedule_sha256: &record.schedule_sha256,
            scheduled_contract_id: &record.scheduled_contract_id,
            scheduled_contract_sha256: &record.scheduled_contract_sha256,
            expected_last_event_seq: record.expected_last_event_seq,
            expected_last_event_sha256: &record.expected_last_event_sha256,
            execution_ordinal: record.execution_ordinal,
            node_id: &record.node_id,
            attempt: record.attempt,
            project_lane_sha256: &record.project_lane_sha256,
            provider_kind: record.provider,
            endpoint: &record.endpoint,
            model: &record.model,
            destination_sha256: &record.destination_sha256,
            logical_request_id: &record.logical_request_id,
            logical_request_sha256: &record.logical_request_sha256,
            pricing_snapshot_sha256: &record.pricing_snapshot_sha256,
            request_body_bytes: record.provider_request_bytes,
            request_body_sha256: &record.provider_request_sha256,
            provider_request_prepared: record.provider_request_prepared,
            provider_request_sent: record.provider_request_sent,
            lifecycle_contract_admitted: record.lifecycle_contract_admitted,
            execution_authority_released: record.execution_authority_released,
            dispatch_authority_released: record.dispatch_authority_released,
            project_lane_claimed: record.project_lane_claimed,
            progress_observed: record.progress_observed,
            successor_advance_authorized: record.successor_advance_authorized,
        }
    }

    fn from_prepare(request: &'a PrepareGroupAgentScheduledNodeProviderRequest) -> Self {
        Self {
            v: request.v,
            codec_protocol_version: request.codec_protocol_version,
            graph_run_id: &request.graph_run_id,
            schedule_id: &request.schedule_id,
            schedule_sha256: &request.schedule_sha256,
            scheduled_contract_id: &request.scheduled_contract_id,
            scheduled_contract_sha256: &request.scheduled_contract_sha256,
            expected_last_event_seq: request.expected_last_event_seq,
            expected_last_event_sha256: &request.expected_last_event_sha256,
            execution_ordinal: request.execution_ordinal,
            node_id: &request.node_id,
            attempt: request.attempt,
            project_lane_sha256: &request.project_lane_sha256,
            provider_kind: request.provider,
            endpoint: &request.endpoint,
            model: &request.model,
            destination_sha256: &request.destination_sha256,
            logical_request_id: &request.logical_request_id,
            logical_request_sha256: &request.logical_request_sha256,
            pricing_snapshot_sha256: &request.pricing_snapshot_sha256,
            request_body_bytes: request.provider_request_body.len(),
            request_body_sha256: &request.provider_request_sha256,
            provider_request_prepared: request.provider_request_prepared,
            provider_request_sent: request.provider_request_sent,
            lifecycle_contract_admitted: request.lifecycle_contract_admitted,
            execution_authority_released: request.execution_authority_released,
            dispatch_authority_released: request.dispatch_authority_released,
            project_lane_claimed: request.project_lane_claimed,
            progress_observed: request.progress_observed,
            successor_advance_authorized: request.successor_advance_authorized,
        }
    }
}

fn prepared_request_digest(
    fields: &PreparedRequestDigestFields<'_>,
) -> Result<String, GroupAgentScheduledNodeProviderRequestValidationError> {
    let bytes = prepared_request_payload_bytes(fields)?;
    let mut digest = Sha256::new();
    digest.update(GROUP_AGENT_SCHEDULED_NODE_PROVIDER_REQUEST_DIGEST_DOMAIN);
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn prepared_request_payload_json(
    fields: &PreparedRequestDigestFields<'_>,
) -> Result<String, GroupAgentScheduledNodeProviderRequestValidationError> {
    String::from_utf8(prepared_request_payload_bytes(fields)?).map_err(|_| invalid_codec())
}

fn prepared_request_payload_bytes(
    fields: &PreparedRequestDigestFields<'_>,
) -> Result<Vec<u8>, GroupAgentScheduledNodeProviderRequestValidationError> {
    serde_json::to_vec(fields).map_err(|_| invalid_codec())
}

fn invalid_codec() -> GroupAgentScheduledNodeProviderRequestValidationError {
    GroupAgentScheduledNodeProviderRequestValidationError {
        message: "scheduled provider request identity cannot be canonically encoded".into(),
    }
}

pub trait GroupAgentScheduledNodeProviderRequestStore: Send + Sync {
    /// Atomically persists or exactly replays one passive provider request.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn prepare_group_agent_scheduled_node_provider_request(
        &self,
        request: &PrepareGroupAgentScheduledNodeProviderRequest,
    ) -> Result<PrepareGroupAgentScheduledNodeProviderRequestResult, HubStoreError>;

    /// Loads one request with its exact bytes and immutable source candidate.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or storage error.
    fn inspect_group_agent_scheduled_node_provider_request(
        &self,
        provider_request_id: &str,
    ) -> Result<GroupAgentScheduledNodeProviderRequestInspection, HubStoreError>;

    /// Lists bounded request metadata without provider body bytes.
    ///
    /// # Errors
    ///
    /// Returns a structured validation, corruption, or storage error.
    fn list_group_agent_scheduled_node_provider_requests(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentScheduledNodeProviderRequestRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAgentScheduledNodeProviderRequestValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentScheduledNodeProviderRequestValidationError {}

#[cfg(test)]
#[path = "scheduled_provider_request_tests.rs"]
mod tests;
