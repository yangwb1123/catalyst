package decisioncapsulecontract

import (
	kd "forgeos/forge-core/internal/kerneldecisioncontract"
	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

type ReplayAttestations struct {
	ApprovalAuthentication     bool `json:"approval_authentication_attestation"`
	AttemptHistoryCompleteness bool `json:"attempt_history_completeness_attestation"`
	Authority                  bool `json:"authority_attestation"`
	Authorization              bool `json:"authorization_attestation"`
	BindingAuthentication      bool `json:"binding_authentication_attestation"`
	CapsuleCompleteness        bool `json:"capsule_completeness_attestation"`
	CAS                        bool `json:"cas_attestation"`
	Completion                 bool `json:"completion_attestation"`
	ContentProvenance          bool `json:"content_provenance_attestation"`
	Effect                     bool `json:"effect_attestation"`
	EvaluationExecution        bool `json:"evaluation_execution_attestation"`
	EvaluatorIndependence      bool `json:"evaluator_independence_attestation"`
	EventAppend                bool `json:"event_append_attestation"`
	Execution                  bool `json:"execution_attestation"`
	ExternalHistoryResolution  bool `json:"external_history_resolution_attestation"`
	GrantAuthentication        bool `json:"grant_authentication_attestation"`
	HardGuard                  bool `json:"hard_guard_attestation"`
	Instruction                bool `json:"instruction_attestation"`
	Outcome                    bool `json:"outcome_attestation"`
	Permission                 bool `json:"permission_attestation"`
	Persistence                bool `json:"persistence_attestation"`
	PrincipalAuthentication    bool `json:"principal_authentication_attestation"`
	ReflectionCompleteness     bool `json:"reflection_completeness_attestation"`
	ReplayEquivalence          bool `json:"replay_equivalence_attestation"`
	ResultAuthentication       bool `json:"result_authentication_attestation"`
	RuleEvaluation             bool `json:"rule_evaluation_attestation"`
	SourceResolution           bool `json:"source_resolution_attestation"`
	Transition                 bool `json:"transition_attestation"`
	Truth                      bool `json:"truth_attestation"`
	UsageMeasurement           bool `json:"usage_measurement_attestation"`
	VerifierIndependence       bool `json:"verifier_independence_attestation"`
	WorldStateResolution       bool `json:"world_state_resolution_attestation"`
}

type ClosureRef struct {
	ClosureID     string `json:"closure_id"`
	ClosureSHA256 string `json:"closure_sha256"`
}

type DecisionTransactionRef struct {
	DecisionTransactionID     string `json:"decision_transaction_id"`
	DecisionTransactionSHA256 string `json:"decision_transaction_sha256"`
}

type ManifestRef struct {
	ManifestID     string `json:"manifest_id"`
	ManifestSHA256 string `json:"manifest_sha256"`
}

type CapsuleRef struct {
	CapsuleID     string `json:"capsule_id"`
	CapsuleSHA256 string `json:"capsule_sha256"`
}

type StructuralReplayManifest struct {
	APIVersion               string                       `json:"api_version"`
	ArtifactReceiptRefs      []op.ArtifactReceiptRef      `json:"artifact_receipt_refs"`
	ArtifactRefs             []op.ArtifactRef             `json:"artifact_refs"`
	Attestations             ReplayAttestations           `json:"attestations"`
	Canonicalization         string                       `json:"canonicalization"`
	CapabilityInvocationRefs []op.CapabilityInvocationRef `json:"capability_invocation_refs"`
	DecisionClosureRef       ClosureRef                   `json:"decision_closure_ref"`
	DecisionTransactionRef   DecisionTransactionRef       `json:"decision_transaction_ref"`
	EffectReplayAllowed      bool                         `json:"effect_replay_allowed"`
	ExecutionReceiptRefs     []op.ExecutionReceiptRef     `json:"execution_receipt_refs"`
	HistoryRewriteAllowed    bool                         `json:"history_rewrite_allowed"`
	InteractionEventRefs     []op.InteractionEventRef     `json:"interaction_event_refs"`
	Kind                     string                       `json:"kind"`
	ManifestID               string                       `json:"manifest_id"`
	ManifestSHA256           string                       `json:"manifest_sha256"`
	OperationalClosureRef    ClosureRef                   `json:"operational_closure_ref"`
	PostdecisionAtomRefs     []kd.AtomRef                 `json:"postdecision_atom_refs"`
	PredecisionAtomRefs      []kd.AtomRef                 `json:"predecision_atom_refs"`
	ReplayMode               string                       `json:"replay_mode"`
}

type DecisionCapsule struct {
	APIVersion       string                            `json:"api_version"`
	Attestations     ReplayAttestations                `json:"attestations"`
	Canonicalization string                            `json:"canonicalization"`
	CapsuleID        string                            `json:"capsule_id"`
	CapsuleMode      string                            `json:"capsule_mode"`
	CapsuleSHA256    string                            `json:"capsule_sha256"`
	DecisionClosure  kd.KernelDecisionReferenceClosure `json:"decision_closure"`
	Kind             string                            `json:"kind"`
	ReplayManifest   StructuralReplayManifest          `json:"replay_manifest"`
	Result           string                            `json:"result"`
}

type EvaluationBranch struct {
	APIVersion            string             `json:"api_version"`
	Attestations          ReplayAttestations `json:"attestations"`
	BranchID              string             `json:"branch_id"`
	BranchMode            string             `json:"branch_mode"`
	BranchSHA256          string             `json:"branch_sha256"`
	Canonicalization      string             `json:"canonicalization"`
	CapsuleRef            CapsuleRef         `json:"capsule_ref"`
	ComparisonResult      string             `json:"comparison_result"`
	DecisionClosureRef    ClosureRef         `json:"decision_closure_ref"`
	EffectReplayAllowed   bool               `json:"effect_replay_allowed"`
	HistoryRewriteAllowed bool               `json:"history_rewrite_allowed"`
	Kind                  string             `json:"kind"`
	ManifestRef           ManifestRef        `json:"manifest_ref"`
}

type StructuralReplayClosure struct {
	APIVersion                   string             `json:"api_version"`
	Attestations                 ReplayAttestations `json:"attestations"`
	Canonicalization             string             `json:"canonicalization"`
	ClosureID                    string             `json:"closure_id"`
	ClosureSHA256                string             `json:"closure_sha256"`
	DecisionCapsule              DecisionCapsule    `json:"decision_capsule"`
	EvaluationBranch             EvaluationBranch   `json:"evaluation_branch"`
	Kind                         string             `json:"kind"`
	ReflectionReportArtifactRefs []op.ArtifactRef   `json:"reflection_report_artifact_refs"`
	Result                       string             `json:"result"`
}
