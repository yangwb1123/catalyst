use serde::{Deserialize, Serialize};

use crate::{
    kernel_decision_contract::{AtomRef, KernelDecisionReferenceClosure},
    kernel_operational_contract::{
        ArtifactReceiptRef, ArtifactRef, CapabilityInvocationRef, ExecutionReceiptRef,
        InteractionEventRef,
    },
};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct ReplayAttestations {
    pub approval_authentication_attestation: bool,
    pub attempt_history_completeness_attestation: bool,
    pub authority_attestation: bool,
    pub authorization_attestation: bool,
    pub binding_authentication_attestation: bool,
    pub capsule_completeness_attestation: bool,
    pub cas_attestation: bool,
    pub completion_attestation: bool,
    pub content_provenance_attestation: bool,
    pub effect_attestation: bool,
    pub evaluation_execution_attestation: bool,
    pub evaluator_independence_attestation: bool,
    pub event_append_attestation: bool,
    pub execution_attestation: bool,
    pub external_history_resolution_attestation: bool,
    pub grant_authentication_attestation: bool,
    pub hard_guard_attestation: bool,
    pub instruction_attestation: bool,
    pub outcome_attestation: bool,
    pub permission_attestation: bool,
    pub persistence_attestation: bool,
    pub principal_authentication_attestation: bool,
    pub reflection_completeness_attestation: bool,
    pub replay_equivalence_attestation: bool,
    pub result_authentication_attestation: bool,
    pub rule_evaluation_attestation: bool,
    pub source_resolution_attestation: bool,
    pub transition_attestation: bool,
    pub truth_attestation: bool,
    pub usage_measurement_attestation: bool,
    pub verifier_independence_attestation: bool,
    pub world_state_resolution_attestation: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionClosureRef {
    pub closure_id: String,
    pub closure_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OperationalClosureRef {
    pub closure_id: String,
    pub closure_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionTransactionRef {
    pub decision_transaction_id: String,
    pub decision_transaction_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuralReplayManifestRef {
    pub manifest_id: String,
    pub manifest_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionCapsuleRef {
    pub capsule_id: String,
    pub capsule_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuralReplayManifest {
    pub api_version: String,
    pub artifact_receipt_refs: Vec<ArtifactReceiptRef>,
    pub artifact_refs: Vec<ArtifactRef>,
    pub attestations: ReplayAttestations,
    pub canonicalization: String,
    pub capability_invocation_refs: Vec<CapabilityInvocationRef>,
    pub decision_closure_ref: DecisionClosureRef,
    pub decision_transaction_ref: DecisionTransactionRef,
    pub effect_replay_allowed: bool,
    pub execution_receipt_refs: Vec<ExecutionReceiptRef>,
    pub history_rewrite_allowed: bool,
    pub interaction_event_refs: Vec<InteractionEventRef>,
    pub kind: String,
    pub manifest_id: String,
    pub manifest_sha256: String,
    pub operational_closure_ref: OperationalClosureRef,
    pub postdecision_atom_refs: Vec<AtomRef>,
    pub predecision_atom_refs: Vec<AtomRef>,
    pub replay_mode: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionCapsule {
    pub api_version: String,
    pub attestations: ReplayAttestations,
    pub canonicalization: String,
    pub capsule_id: String,
    pub capsule_mode: String,
    pub capsule_sha256: String,
    pub decision_closure: KernelDecisionReferenceClosure,
    pub kind: String,
    pub replay_manifest: StructuralReplayManifest,
    pub result: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvaluationBranch {
    pub api_version: String,
    pub attestations: ReplayAttestations,
    pub branch_id: String,
    pub branch_mode: String,
    pub branch_sha256: String,
    pub canonicalization: String,
    pub capsule_ref: DecisionCapsuleRef,
    pub comparison_result: String,
    pub decision_closure_ref: DecisionClosureRef,
    pub effect_replay_allowed: bool,
    pub history_rewrite_allowed: bool,
    pub kind: String,
    pub manifest_ref: StructuralReplayManifestRef,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StructuralReplayClosure {
    pub api_version: String,
    pub attestations: ReplayAttestations,
    pub canonicalization: String,
    pub closure_id: String,
    pub closure_sha256: String,
    pub decision_capsule: DecisionCapsule,
    pub evaluation_branch: EvaluationBranch,
    pub kind: String,
    pub reflection_report_artifact_refs: Vec<ArtifactRef>,
    pub result: String,
}
