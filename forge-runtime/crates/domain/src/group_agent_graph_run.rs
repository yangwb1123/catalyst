use serde::{Deserialize, Serialize};

use crate::{GroupAgentGraphEdge, HubStoreError};

#[path = "group_agent_graph_run_codec.rs"]
mod codec;
#[path = "group_agent_graph_run/event_wire.rs"]
mod event_wire;
#[path = "group_agent_graph_run_journal_validation.rs"]
mod journal_validation;
mod terminal_validation;
#[path = "group_agent_graph_run_validation.rs"]
mod validation;

pub const GROUP_AGENT_GRAPH_CORE_PLAN_VERSION: u16 = 1;
pub const GROUP_AGENT_GRAPH_RUN_VERSION: u16 = 1;
pub const GROUP_AGENT_GRAPH_RUN_CONTRACT_VERSION: u16 = 2;
pub const GROUP_AGENT_GRAPH_RUN_DISPATCH_REQUEST_VERSION: u16 = 3;
pub const GROUP_AGENT_GRAPH_RUN_DISPATCH_CLAIM_VERSION: u16 = 4;
pub const GROUP_AGENT_GRAPH_RUN_TERMINAL_VERSION: u16 = 5;
pub const GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_GRAPH_CORE_PLAN_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-graph-core-plan.v1\0";
pub const GROUP_AGENT_GRAPH_RUN_EVENT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-graph-run-event.v1\0";
pub const GROUP_AGENT_GRAPH_RUN_CONTROL_EVENT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-graph-run-control-event.v1\0";
pub const MAX_GROUP_AGENT_GRAPH_CORE_PLAN_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES: usize = 64 * 1024;
pub const MAX_GROUP_AGENT_GRAPH_RUN_EVENTS: usize = 5;
pub const MAX_GROUP_AGENT_GRAPH_RUN_JOURNAL_BYTES: usize =
    MAX_GROUP_AGENT_GRAPH_RUN_EVENTS * MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES;
pub const MAX_GROUP_AGENT_GRAPH_RUN_LIST_LIMIT: usize = 100;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphCorePlan {
    pub v: u16,
    pub scheduler_protocol_version: u16,
    pub graph_version: u16,
    pub graph_id: String,
    pub graph_manifest_sha256: String,
    pub authored_node_ids: Vec<String>,
    pub edges: Vec<GroupAgentGraphEdge>,
    pub waves: Vec<Vec<String>>,
    pub execution_contract_present: bool,
    pub dispatch_authority_released: bool,
    pub plan_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupAgentGraphRunStatus {
    AwaitingExecutionContract,
    AwaitingCoreDispatch,
    AwaitingDispatchAuthorization,
    DispatchUnknown,
    Completed,
    Failed,
    FailedUncertain,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentGraphRunRecord {
    pub v: u16,
    pub graph_run_id: String,
    pub graph_id: String,
    pub status: GroupAgentGraphRunStatus,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub scheduler_protocol_version: u16,
    pub plan_sha256: String,
    pub plan_bytes: usize,
    pub node_count: usize,
    pub wave_count: usize,
    pub execution_contract_present: bool,
    pub dispatch_request_present: bool,
    pub dispatch_authority_released: bool,
    pub last_event_seq: u64,
    pub journal_bytes: usize,
    pub created_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GroupAgentGraphRunEvent {
    pub v: u16,
    pub graph_run_id: String,
    pub seq: u64,
    #[serde(flatten)]
    pub kind: GroupAgentGraphRunEventKind,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GroupAgentGraphRunEventKind {
    GraphRunPrepared {
        graph_id: String,
        graph_manifest_sha256: String,
        plan_sha256: String,
        scheduler_protocol_version: u16,
        prepared_at_ms: u64,
    },
    NodeExecutionContractAdmitted {
        previous_event_sha256: String,
        control_snapshot_sha256: String,
        contract_id: String,
        contract_sha256: String,
        contract_bytes: usize,
        node_id: String,
        attempt: u16,
        request_sha256: String,
        project_lane_sha256: String,
        admitted_at_ms: u64,
    },
    NodeDispatchRequestPrepared {
        previous_event_sha256: String,
        contract_id: String,
        contract_sha256: String,
        dispatch_request_id: String,
        dispatch_request_sha256: String,
        request_body_sha256: String,
        request_body_bytes: usize,
        logical_request_sha256: String,
        node_id: String,
        attempt: u16,
        project_lane_sha256: String,
        codec_protocol_version: u16,
        provider_kind: crate::GroupAgentNodeProviderKind,
        destination_sha256: String,
        pricing_snapshot_sha256: String,
        prepared_at_ms: u64,
    },
    NodeDispatchReleased {
        previous_event_sha256: String,
        dispatch_id: String,
        authorization_id: String,
        authorization_sha256: String,
        dispatch_request_id: String,
        dispatch_request_sha256: String,
        logical_request_sha256: String,
        request_body_sha256: String,
        request_body_bytes: usize,
        pricing_snapshot_sha256: String,
        node_id: String,
        attempt: u16,
        max_cost_usd_micros: u64,
        consent_contract_version: u16,
        lane_ownership_id: String,
        project_lane_sha256: String,
        released_at_ms: u64,
    },
    NodeLifecycleTerminalized {
        previous_event_sha256: String,
        dispatch_id: String,
        lane_ownership_id: String,
        project_lane_sha256: String,
        artifact_id: String,
        artifact_sha256: String,
        terminal_receipt_id: String,
        terminal_receipt_sha256: String,
        node_id: String,
        attempt: u16,
        node_outcome: crate::GroupAgentNodeTerminalOutcome,
        wave_index: usize,
        wave_outcome: crate::GroupAgentNodeTerminalOutcome,
        graph_status: GroupAgentGraphRunStatus,
        retry_authorized: bool,
        lane_released: bool,
        terminalized_at_ms: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginGroupAgentGraphRun {
    pub v: u16,
    pub graph_run_id: String,
    pub graph_id: String,
    pub source_snapshot_sha256: String,
    pub graph_manifest_sha256: String,
    pub plan: GroupAgentGraphCorePlan,
    pub plan_json: String,
    pub event: GroupAgentGraphRunEvent,
    pub event_json: String,
    pub idempotency_key: String,
    pub created_at_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BeginGroupAgentGraphRunDisposition {
    Created,
    Replayed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginGroupAgentGraphRunResult {
    pub v: u16,
    pub disposition: BeginGroupAgentGraphRunDisposition,
    pub inspection: GroupAgentGraphRunInspection,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentGraphRunInspection {
    pub v: u16,
    pub run: GroupAgentGraphRunRecord,
    pub plan_json: String,
    pub plan: GroupAgentGraphCorePlan,
    pub event_jsons: Vec<String>,
    pub events: Vec<GroupAgentGraphRunEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentGraphRunValidationError {
    pub message: String,
}

impl GroupAgentGraphCorePlan {
    /// Validates one passive, non-executable scheduler plan.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid topology, flags, canonical digest, or bounds.
    pub fn validate(&self) -> Result<(), GroupAgentGraphRunValidationError> {
        validation::validate_plan(self)
    }

    /// Encodes the complete plan, including its digest, as canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the plan cannot be canonically encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentGraphRunValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated digest over canonical plan fields excluding
    /// `plan_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error when the digest payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentGraphRunValidationError> {
        codec::plan_digest(self)
    }
}

impl GroupAgentGraphRunRecord {
    /// Validates content-free passive Graph Run metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported, unsafe, or inconsistent metadata.
    pub fn validate(&self) -> Result<(), GroupAgentGraphRunValidationError> {
        journal_validation::validate_record(self)
    }
}

impl GroupAgentGraphRunEvent {
    /// Strictly decodes one exact compact canonical journal event.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, malformed, noncanonical, or
    /// protocol-invalid input.
    pub fn decode_exact(json: &str) -> Result<Self, GroupAgentGraphRunValidationError> {
        Self::decode_exact_bytes(json.as_bytes())
    }

    /// Strictly decodes exact compact canonical event bytes, including UTF-8.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, oversized, non-UTF-8, malformed,
    /// noncanonical, or protocol-invalid input.
    pub fn decode_exact_bytes(bytes: &[u8]) -> Result<Self, GroupAgentGraphRunValidationError> {
        if bytes.is_empty() || bytes.len() > MAX_GROUP_AGENT_GRAPH_RUN_EVENT_BYTES {
            return Err(GroupAgentGraphRunValidationError {
                message: "Graph Run event input is outside its byte bound".into(),
            });
        }
        let event: Self =
            serde_json::from_slice(bytes).map_err(|_| GroupAgentGraphRunValidationError {
                message: "Graph Run event input is invalid JSON".into(),
            })?;
        event.validate()?;
        if event.canonical_json()?.as_bytes() != bytes {
            return Err(GroupAgentGraphRunValidationError {
                message: "Graph Run event input is not exact canonical JSON".into(),
            });
        }
        Ok(event)
    }

    /// Validates one versioned passive Graph Run journal event.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, binding, protocol, sequence, or time.
    pub fn validate(&self) -> Result<(), GroupAgentGraphRunValidationError> {
        journal_validation::validate_event(self)
    }

    /// Encodes this event as canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the event cannot be canonically encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentGraphRunValidationError> {
        codec::canonical_json(self)
    }

    /// Computes the domain-separated digest over the exact canonical event JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when the event cannot be canonically encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentGraphRunValidationError> {
        codec::event_digest(self)
    }
}

impl BeginGroupAgentGraphRun {
    /// Validates one exact passive Graph Run persistence request.
    ///
    /// # Errors
    ///
    /// Returns an error when input, canonical bytes, or cross-bindings diverge.
    pub fn validate(&self) -> Result<(), GroupAgentGraphRunValidationError> {
        journal_validation::validate_begin(self)
    }
}

impl GroupAgentGraphRunInspection {
    /// Fully validates record, plan, event, bytes, and exact cross-bindings.
    ///
    /// # Errors
    ///
    /// Returns an error for any corrupt or inconsistent durable state.
    pub fn validate(&self) -> Result<(), GroupAgentGraphRunValidationError> {
        journal_validation::validate_inspection(self)
    }
}

pub trait GroupAgentGraphRunStore: Send + Sync {
    /// Atomically begins or exactly replays one passive Graph Run.
    ///
    /// # Errors
    ///
    /// Returns a structured conflict, corruption, or storage error.
    fn begin_group_agent_graph_run(
        &self,
        request: &BeginGroupAgentGraphRun,
    ) -> Result<BeginGroupAgentGraphRunResult, HubStoreError>;

    /// Fully loads one passive Graph Run.
    ///
    /// # Errors
    ///
    /// Returns a structured not-found, corruption, or storage error.
    fn inspect_group_agent_graph_run(
        &self,
        graph_run_id: &str,
    ) -> Result<GroupAgentGraphRunInspection, HubStoreError>;

    /// Lists bounded, content-free Graph Run metadata.
    ///
    /// # Errors
    ///
    /// Returns a structured validation, corruption, or storage error.
    fn list_group_agent_graph_runs(
        &self,
        graph_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<GroupAgentGraphRunRecord>, HubStoreError>;
}

impl std::fmt::Display for GroupAgentGraphRunValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentGraphRunValidationError {}
