use serde::{Deserialize, Serialize};

pub use crate::capability_grant_contract::{
    CapabilityIdentity, EnvironmentClass, GrantTaskBinding as TaskBinding, Principal, PrincipalType,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct Attestations {
    pub authorization_attestation: bool,
    pub binding_authentication_attestation: bool,
    pub completion_attestation: bool,
    pub content_provenance_attestation: bool,
    pub effect_attestation: bool,
    pub event_append_attestation: bool,
    pub execution_attestation: bool,
    pub grant_authentication_attestation: bool,
    pub outcome_attestation: bool,
    pub permission_attestation: bool,
    pub persistence_attestation: bool,
    pub principal_authentication_attestation: bool,
    pub transition_attestation: bool,
    pub usage_measurement_attestation: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalBindings {
    pub context_sha256: String,
    pub environment_profile_id: String,
    pub environment_sha256: String,
    pub policy_sha256: String,
    pub source_profile_id: String,
    pub source_revision: String,
    pub source_tree_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityGrantRef {
    pub authority_domain: String,
    pub grant_id: String,
    pub grant_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRef {
    pub artifact_kind: String,
    pub artifact_ref: String,
    pub artifact_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReceiptRef {
    pub artifact_receipt_id: String,
    pub artifact_receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInvocationRef {
    pub invocation_id: String,
    pub invocation_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionEventRef {
    pub event_id: String,
    pub event_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceiptRef {
    pub execution_receipt_id: String,
    pub execution_receipt_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedUsage {
    pub call_count: i64,
    pub cost_usd_micros: i64,
    pub elapsed_ms: i64,
    pub input_tokens: i64,
    pub network_bytes: i64,
    pub output_bytes: i64,
    pub output_tokens: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReceipt {
    pub api_version: String,
    pub artifact: ArtifactRef,
    pub artifact_receipt_id: String,
    pub artifact_receipt_sha256: String,
    pub attestations: Attestations,
    pub bindings: OperationalBindings,
    pub canonicalization: String,
    pub content_bytes: i64,
    pub created_at_unix_ms: i64,
    pub kind: String,
    pub producer: Principal,
    pub producer_invocation_ref: Option<CapabilityInvocationRef>,
    pub receipt_role: String,
    pub slot: String,
    pub task_binding: TaskBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityInvocation {
    pub api_version: String,
    pub attempt: i64,
    pub attestations: Attestations,
    pub bindings: OperationalBindings,
    pub canonicalization: String,
    pub capability: CapabilityIdentity,
    pub capability_grant_ref: CapabilityGrantRef,
    pub correlation_id: String,
    pub declared_output_slots: Vec<String>,
    pub idempotency_key: String,
    pub input_artifact_receipt_refs: Vec<ArtifactReceiptRef>,
    pub invocation_id: String,
    pub invocation_sha256: String,
    pub kind: String,
    pub prior_execution_receipt_ref: Option<ExecutionReceiptRef>,
    pub requested_action_sha256: String,
    pub requested_at_unix_ms: i64,
    pub subject: Principal,
    pub task_binding: TaskBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InteractionEvent {
    pub actor: Principal,
    pub api_version: String,
    pub artifact_refs: Vec<ArtifactRef>,
    pub attestations: Attestations,
    pub bindings: OperationalBindings,
    pub canonicalization: String,
    pub causation_event_ref: Option<InteractionEventRef>,
    pub confidence_micros: Option<i64>,
    pub correlation_id: String,
    pub event_id: String,
    pub event_sha256: String,
    pub invocation_ref: CapabilityInvocationRef,
    pub kind: String,
    pub logical_sequence: i64,
    pub object_ref: String,
    pub occurred_at_unix_ms: i64,
    pub target: Option<Principal>,
    pub task_binding: TaskBinding,
    pub verb: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionReceipt {
    pub api_version: String,
    pub attempt: i64,
    pub attestations: Attestations,
    pub bindings: OperationalBindings,
    pub canonicalization: String,
    pub correlation_id: String,
    pub ended_at_unix_ms: i64,
    pub event_refs: Vec<InteractionEventRef>,
    pub execution_receipt_id: String,
    pub execution_receipt_sha256: String,
    pub executor: Principal,
    pub input_artifacts: Vec<ArtifactRef>,
    pub invocation_ref: CapabilityInvocationRef,
    pub kind: String,
    pub observed_usage: ObservedUsage,
    pub outcome: String,
    pub output_artifact_receipt_refs: Vec<ArtifactReceiptRef>,
    pub prior_execution_receipt_ref: Option<ExecutionReceiptRef>,
    pub reason_codes: Vec<String>,
    pub started_at_unix_ms: i64,
    pub task_binding: TaskBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelOperationalReferenceClosure {
    pub api_version: String,
    pub artifact_receipts: Vec<ArtifactReceipt>,
    pub artifacts: Vec<ArtifactRef>,
    pub attestations: Attestations,
    pub canonicalization: String,
    pub capability_invocations: Vec<CapabilityInvocation>,
    pub closure_id: String,
    pub closure_sha256: String,
    pub execution_receipts: Vec<ExecutionReceipt>,
    pub interaction_events: Vec<InteractionEvent>,
    pub kind: String,
    pub result: String,
}
