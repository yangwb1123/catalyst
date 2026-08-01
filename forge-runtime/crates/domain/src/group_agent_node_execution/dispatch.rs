use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    GroupAgentGraphRunEvent, GroupAgentNodeExecutionContractInspection, GroupAgentNodeProviderKind,
    HubStoreError,
};

#[path = "dispatch_validation.rs"]
mod validation;

pub const GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_PROVIDER_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-provider-request.v1\0";
pub const GROUP_AGENT_NODE_DISPATCH_REQUEST_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-dispatch-request.v1\0";
pub const GROUP_AGENT_NODE_DESTINATION_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-destination.v1\0";
pub const MAX_GROUP_AGENT_NODE_PROVIDER_REQUEST_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_GROUP_AGENT_NODE_DISPATCH_REQUEST_LIST_LIMIT: usize = 100;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodeDispatchRequestRecord {
    pub v: u16,
    pub dispatch_request_id: String,
    pub graph_run_id: String,
    pub contract_id: String,
    pub node_id: String,
    pub attempt: u16,
    pub contract_sha256: String,
    pub request_sha256: String,
    pub project_lane_sha256: String,
    pub provider: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub pricing_snapshot_sha256: String,
    pub provider_request_sha256: String,
    pub provider_request_bytes: usize,
    pub destination_sha256: String,
    pub dispatch_request_sha256: String,
    pub codec_protocol_version: u16,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentNodeDispatchRequest {
    pub v: u16,
    pub dispatch_request_id: String,
    pub graph_run_id: String,
    pub contract_id: String,
    pub contract_sha256: String,
    pub node_id: String,
    pub attempt: u16,
    pub request_sha256: String,
    pub project_lane_sha256: String,
    pub provider: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub pricing_snapshot_sha256: String,
    pub provider_request_body: Vec<u8>,
    pub provider_request_sha256: String,
    pub destination_sha256: String,
    pub dispatch_request_sha256: String,
    pub codec_protocol_version: u16,
    pub expected_last_event_seq: u64,
    pub expected_last_event_sha256: String,
    pub event: GroupAgentGraphRunEvent,
    pub event_json: String,
    pub idempotency_key: String,
    pub prepared_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrepareGroupAgentNodeDispatchRequestDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepareGroupAgentNodeDispatchRequestResult {
    pub v: u16,
    pub disposition: PrepareGroupAgentNodeDispatchRequestDisposition,
    pub inspection: GroupAgentNodeDispatchRequestInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeDispatchRequestInspection {
    pub v: u16,
    pub record: GroupAgentNodeDispatchRequestRecord,
    pub provider_request_body: Vec<u8>,
    pub preparation_event_json: String,
    pub preparation_event: GroupAgentGraphRunEvent,
    pub contract: GroupAgentNodeExecutionContractInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodeDispatchRequestValidationError {
    pub message: String,
}

impl GroupAgentNodeDispatchRequestRecord {
    /// Validates content-bounded prepared-request metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed identities, bindings, or bounds.
    pub fn validate(&self) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
        validation::validate_record(self)
    }

    /// Validates the seq-3 preparation receipt against this exact record.
    ///
    /// # Errors
    ///
    /// Returns an error when the event is invalid or any durable binding differs.
    pub fn validate_preparation_event(
        &self,
        event: &GroupAgentGraphRunEvent,
    ) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
        validation::validate_record_event(self, event)
    }

    /// Recomputes the semantic identity excluding ID and creation metadata.
    ///
    /// # Errors
    ///
    /// Returns an error when the canonical identity payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeDispatchRequestValidationError> {
        dispatch_digest(&DispatchDigestFields::from_record(self))
    }

    /// Encodes the exact version-1 semantic payload used for content addressing.
    ///
    /// The public payload deliberately uses `logical_request_sha256` for the
    /// admitted contract request and `request_body_*` for the provider bytes,
    /// even though the durable Rust record retains its older internal field
    /// names.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded as compact JSON.
    pub fn canonical_payload_json(
        &self,
    ) -> Result<String, GroupAgentNodeDispatchRequestValidationError> {
        dispatch_payload_json(&DispatchDigestFields::from_record(self))
    }
}

impl PrepareGroupAgentNodeDispatchRequest {
    /// Validates one exact, passive provider-request preparation candidate.
    ///
    /// # Errors
    ///
    /// Returns an error when exact bytes, digests, event, or bindings diverge.
    pub fn validate(&self) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
        validation::validate_prepare(self)
    }

    /// Recomputes the semantic identity excluding ID, idempotency key, and time.
    ///
    /// # Errors
    ///
    /// Returns an error when the canonical identity payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodeDispatchRequestValidationError> {
        dispatch_digest(&DispatchDigestFields::from_prepare(self))
    }

    /// Encodes the exact version-1 semantic payload used for content addressing.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixed payload cannot be encoded as compact JSON.
    pub fn canonical_payload_json(
        &self,
    ) -> Result<String, GroupAgentNodeDispatchRequestValidationError> {
        dispatch_payload_json(&DispatchDigestFields::from_prepare(self))
    }
}

impl GroupAgentNodeDispatchRequestInspection {
    /// Fully validates durable request bytes and their contract and journal bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt or inconsistent durable state.
    pub fn validate(&self) -> Result<(), GroupAgentNodeDispatchRequestValidationError> {
        validation::validate_inspection(self)
    }
}

/// Computes the domain-separated identity of exact provider request bytes.
#[must_use]
pub fn group_agent_node_provider_request_sha256(body: &[u8]) -> String {
    digest_hex(GROUP_AGENT_NODE_PROVIDER_REQUEST_DIGEST_DOMAIN, body)
}

/// Computes the provider destination identity without copying plaintext into events.
///
/// # Panics
///
/// Panics only if Serde cannot encode the fixed string-and-enum payload shape.
#[must_use]
pub fn group_agent_node_destination_sha256(
    provider: GroupAgentNodeProviderKind,
    endpoint: &str,
    model: &str,
) -> String {
    let payload = DestinationDigestPayload {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        provider_kind: provider,
        endpoint,
        model,
    };
    let bytes = serde_json::to_vec(&payload)
        .expect("the fixed provider-destination payload is always JSON encodable");
    digest_hex(GROUP_AGENT_NODE_DESTINATION_DIGEST_DOMAIN, &bytes)
}

/// Derives a stable request ID from the complete dispatch-request identity.
#[must_use]
pub fn group_agent_node_dispatch_request_id(dispatch_request_sha256: &str) -> String {
    format!("node-dispatch-request-{dispatch_request_sha256}")
}

#[derive(Serialize)]
struct DestinationDigestPayload<'a> {
    v: u16,
    provider_kind: GroupAgentNodeProviderKind,
    endpoint: &'a str,
    model: &'a str,
}

#[derive(Serialize)]
struct DispatchDigestFields<'a> {
    v: u16,
    codec_protocol_version: u16,
    graph_run_id: &'a str,
    contract_id: &'a str,
    contract_sha256: &'a str,
    expected_last_event_seq: u64,
    expected_last_event_sha256: &'a str,
    node_id: &'a str,
    attempt: u16,
    project_lane_sha256: &'a str,
    provider_kind: GroupAgentNodeProviderKind,
    endpoint: &'a str,
    model: &'a str,
    destination_sha256: &'a str,
    logical_request_sha256: &'a str,
    pricing_snapshot_sha256: &'a str,
    request_body_bytes: usize,
    request_body_sha256: &'a str,
}

impl<'a> DispatchDigestFields<'a> {
    fn from_record(record: &'a GroupAgentNodeDispatchRequestRecord) -> Self {
        Self {
            v: record.v,
            codec_protocol_version: record.codec_protocol_version,
            graph_run_id: &record.graph_run_id,
            contract_id: &record.contract_id,
            contract_sha256: &record.contract_sha256,
            expected_last_event_seq: record.expected_last_event_seq,
            expected_last_event_sha256: &record.expected_last_event_sha256,
            node_id: &record.node_id,
            attempt: record.attempt,
            project_lane_sha256: &record.project_lane_sha256,
            provider_kind: record.provider,
            endpoint: &record.endpoint,
            model: &record.model,
            destination_sha256: &record.destination_sha256,
            logical_request_sha256: &record.request_sha256,
            pricing_snapshot_sha256: &record.pricing_snapshot_sha256,
            request_body_bytes: record.provider_request_bytes,
            request_body_sha256: &record.provider_request_sha256,
        }
    }

    fn from_prepare(request: &'a PrepareGroupAgentNodeDispatchRequest) -> Self {
        Self {
            v: request.v,
            codec_protocol_version: request.codec_protocol_version,
            graph_run_id: &request.graph_run_id,
            contract_id: &request.contract_id,
            contract_sha256: &request.contract_sha256,
            expected_last_event_seq: request.expected_last_event_seq,
            expected_last_event_sha256: &request.expected_last_event_sha256,
            node_id: &request.node_id,
            attempt: request.attempt,
            project_lane_sha256: &request.project_lane_sha256,
            provider_kind: request.provider,
            endpoint: &request.endpoint,
            model: &request.model,
            destination_sha256: &request.destination_sha256,
            logical_request_sha256: &request.request_sha256,
            pricing_snapshot_sha256: &request.pricing_snapshot_sha256,
            request_body_bytes: request.provider_request_body.len(),
            request_body_sha256: &request.provider_request_sha256,
        }
    }
}

fn dispatch_digest(
    fields: &DispatchDigestFields<'_>,
) -> Result<String, GroupAgentNodeDispatchRequestValidationError> {
    let bytes = dispatch_payload_bytes(fields)?;
    Ok(digest_hex(
        GROUP_AGENT_NODE_DISPATCH_REQUEST_DIGEST_DOMAIN,
        &bytes,
    ))
}

fn dispatch_payload_json(
    fields: &DispatchDigestFields<'_>,
) -> Result<String, GroupAgentNodeDispatchRequestValidationError> {
    String::from_utf8(dispatch_payload_bytes(fields)?).map_err(|_| {
        GroupAgentNodeDispatchRequestValidationError {
            message: "dispatch request identity was not canonical UTF-8 JSON".into(),
        }
    })
}

fn dispatch_payload_bytes(
    fields: &DispatchDigestFields<'_>,
) -> Result<Vec<u8>, GroupAgentNodeDispatchRequestValidationError> {
    serde_json::to_vec(fields).map_err(|_| GroupAgentNodeDispatchRequestValidationError {
        message: "dispatch request identity cannot be canonically encoded".into(),
    })
}

fn digest_hex(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

pub trait GroupAgentNodeDispatchRequestStore: Send + Sync {
    /// Atomically persists or exactly replays one passive provider request.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn prepare_group_agent_node_dispatch_request(
        &self,
        request: &PrepareGroupAgentNodeDispatchRequest,
    ) -> Result<PrepareGroupAgentNodeDispatchRequestResult, HubStoreError>;

    /// Loads one request with its exact bytes and source contract.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or storage error.
    fn inspect_group_agent_node_dispatch_request(
        &self,
        dispatch_request_id: &str,
    ) -> Result<GroupAgentNodeDispatchRequestInspection, HubStoreError>;

    /// Lists bounded request metadata without provider request bodies.
    ///
    /// # Errors
    ///
    /// Returns a structured validation, corruption, or storage error.
    fn list_group_agent_node_dispatch_requests(
        &self,
        graph_run_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentNodeDispatchRequestRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAgentNodeDispatchRequestValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentNodeDispatchRequestValidationError {}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
