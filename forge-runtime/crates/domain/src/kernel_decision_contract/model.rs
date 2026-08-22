use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::kernel_operational_contract::{
    ArtifactReceiptRef, CapabilityIdentity, KernelOperationalReferenceClosure, OperationalBindings,
    Principal, TaskBinding,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct DecisionAttestations {
    pub approval_authentication_attestation: bool,
    pub authority_attestation: bool,
    pub authorization_attestation: bool,
    pub binding_authentication_attestation: bool,
    pub cas_attestation: bool,
    pub completion_attestation: bool,
    pub content_provenance_attestation: bool,
    pub effect_attestation: bool,
    pub event_append_attestation: bool,
    pub execution_attestation: bool,
    pub grant_authentication_attestation: bool,
    pub hard_guard_attestation: bool,
    pub instruction_attestation: bool,
    pub outcome_attestation: bool,
    pub permission_attestation: bool,
    pub persistence_attestation: bool,
    pub principal_authentication_attestation: bool,
    pub source_resolution_attestation: bool,
    pub transition_attestation: bool,
    pub truth_attestation: bool,
    pub usage_measurement_attestation: bool,
    pub verifier_independence_attestation: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredAuthority {
    pub authority_kind: String,
    pub authority_ref: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Proposition {
    pub object_type: String,
    pub object_value: Value,
    pub predicate: String,
    pub subject: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AtomScope {
    pub module: Option<String>,
    pub object: Option<String>,
    pub project: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AtomSource {
    pub source_kind: String,
    pub source_phase: String,
    pub source_ref: Value,
    pub source_selector: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Validity {
    pub valid_from_unix_ms: i64,
    pub valid_until_unix_ms: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CognitiveAtom {
    pub api_version: String,
    pub atom_id: String,
    pub atom_sha256: String,
    pub atom_type: String,
    pub attestations: DecisionAttestations,
    pub bindings: OperationalBindings,
    pub canonicalization: String,
    pub confidence_micros: Option<i64>,
    pub declared_authority: DeclaredAuthority,
    pub declared_hardness: String,
    pub effective_hardness: String,
    pub epistemic_state: String,
    pub instruction_allowed: bool,
    pub kind: String,
    pub proposition: Proposition,
    pub scope: AtomScope,
    pub source: AtomSource,
    pub task_binding: TaskBinding,
    pub validity: Validity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AtomRef {
    pub atom_id: String,
    pub atom_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub max_calls: i64,
    pub max_cost_usd_micros: i64,
    pub max_input_tokens: i64,
    pub max_network_bytes: i64,
    pub max_output_bytes: i64,
    pub max_output_tokens: i64,
    pub timeout_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionCondition {
    pub condition_ref: String,
    pub condition_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Compensation {
    pub applicability: String,
    pub capability: Option<CapabilityIdentity>,
    pub requested_action_sha256: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionOption {
    pub capability: CapabilityIdentity,
    pub option_id: String,
    pub requested_action_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProofObligation {
    pub obligation_id: String,
    pub predicate_sha256: String,
    pub required_evidence_kinds: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Verifier {
    pub capability: CapabilityIdentity,
    pub independence_basis_sha256: String,
    pub principal: Principal,
    pub timeout_ms: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WritePrecondition {
    pub expected_sha256: String,
    pub precondition_id: String,
    pub resource_ref: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionTransaction {
    pub accountable_owner: Principal,
    pub actor: Principal,
    pub api_version: String,
    pub attestations: DecisionAttestations,
    pub bindings: OperationalBindings,
    pub budget: Budget,
    pub canonicalization: String,
    pub compensation: Compensation,
    pub completion_condition: CompletionCondition,
    pub created_at_unix_ms: i64,
    pub decision_transaction_id: String,
    pub decision_transaction_sha256: String,
    pub goal_atom_ref: AtomRef,
    pub guard_atom_refs: Vec<AtomRef>,
    pub idempotency_key: String,
    pub kind: String,
    pub options: Vec<DecisionOption>,
    pub proof_obligations: Vec<ProofObligation>,
    pub read_artifact_receipt_refs: Vec<ArtifactReceiptRef>,
    pub selected_option_id: String,
    pub selection_basis_sha256: String,
    pub task_binding: TaskBinding,
    pub transaction_mode: String,
    pub trigger_atom_refs: Vec<AtomRef>,
    pub verifier: Verifier,
    pub write_preconditions: Vec<WritePrecondition>,
    pub write_slots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KernelDecisionReferenceClosure {
    pub api_version: String,
    pub attestations: DecisionAttestations,
    pub canonicalization: String,
    pub closure_id: String,
    pub closure_sha256: String,
    pub cognitive_atoms: Vec<CognitiveAtom>,
    pub decision_transaction: DecisionTransaction,
    pub kind: String,
    pub operational_closure: KernelOperationalReferenceClosure,
    pub result: String,
}
