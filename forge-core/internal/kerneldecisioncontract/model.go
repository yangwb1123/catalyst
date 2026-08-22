package kerneldecisioncontract

import (
	"encoding/json"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

type DecisionAttestations struct {
	ApprovalAuthentication  bool `json:"approval_authentication_attestation"`
	Authority               bool `json:"authority_attestation"`
	Authorization           bool `json:"authorization_attestation"`
	BindingAuthentication   bool `json:"binding_authentication_attestation"`
	CAS                     bool `json:"cas_attestation"`
	Completion              bool `json:"completion_attestation"`
	ContentProvenance       bool `json:"content_provenance_attestation"`
	Effect                  bool `json:"effect_attestation"`
	EventAppend             bool `json:"event_append_attestation"`
	Execution               bool `json:"execution_attestation"`
	GrantAuthentication     bool `json:"grant_authentication_attestation"`
	HardGuard               bool `json:"hard_guard_attestation"`
	Instruction             bool `json:"instruction_attestation"`
	Outcome                 bool `json:"outcome_attestation"`
	Permission              bool `json:"permission_attestation"`
	Persistence             bool `json:"persistence_attestation"`
	PrincipalAuthentication bool `json:"principal_authentication_attestation"`
	SourceResolution        bool `json:"source_resolution_attestation"`
	Transition              bool `json:"transition_attestation"`
	Truth                   bool `json:"truth_attestation"`
	UsageMeasurement        bool `json:"usage_measurement_attestation"`
	VerifierIndependence    bool `json:"verifier_independence_attestation"`
}

type DeclaredAuthority struct {
	AuthorityKind string          `json:"authority_kind"`
	AuthorityRef  json.RawMessage `json:"authority_ref"`
}

type Proposition struct {
	ObjectType  string `json:"object_type"`
	ObjectValue any    `json:"object_value"`
	Predicate   string `json:"predicate"`
	Subject     string `json:"subject"`
}

type AtomScope struct {
	Module  *string `json:"module"`
	Object  *string `json:"object"`
	Project string  `json:"project"`
}

type AtomSource struct {
	SourceKind     string          `json:"source_kind"`
	SourcePhase    string          `json:"source_phase"`
	SourceRef      json.RawMessage `json:"source_ref"`
	SourceSelector *string         `json:"source_selector"`
}

type Validity struct {
	ValidFromUnixMS  int64  `json:"valid_from_unix_ms"`
	ValidUntilUnixMS *int64 `json:"valid_until_unix_ms"`
}

type CognitiveAtom struct {
	APIVersion         string                 `json:"api_version"`
	AtomID             string                 `json:"atom_id"`
	AtomSHA256         string                 `json:"atom_sha256"`
	AtomType           string                 `json:"atom_type"`
	Attestations       DecisionAttestations   `json:"attestations"`
	Bindings           op.OperationalBindings `json:"bindings"`
	Canonicalization   string                 `json:"canonicalization"`
	ConfidenceMicros   *int64                 `json:"confidence_micros"`
	DeclaredAuthority  DeclaredAuthority      `json:"declared_authority"`
	DeclaredHardness   string                 `json:"declared_hardness"`
	EffectiveHardness  string                 `json:"effective_hardness"`
	EpistemicState     string                 `json:"epistemic_state"`
	InstructionAllowed bool                   `json:"instruction_allowed"`
	Kind               string                 `json:"kind"`
	Proposition        Proposition            `json:"proposition"`
	Scope              AtomScope              `json:"scope"`
	Source             AtomSource             `json:"source"`
	TaskBinding        op.TaskBinding         `json:"task_binding"`
	Validity           Validity               `json:"validity"`
}

type AtomRef struct {
	AtomID     string `json:"atom_id"`
	AtomSHA256 string `json:"atom_sha256"`
}

type Budget struct {
	MaxCalls         int64 `json:"max_calls"`
	MaxCostUSDMicros int64 `json:"max_cost_usd_micros"`
	MaxInputTokens   int64 `json:"max_input_tokens"`
	MaxNetworkBytes  int64 `json:"max_network_bytes"`
	MaxOutputBytes   int64 `json:"max_output_bytes"`
	MaxOutputTokens  int64 `json:"max_output_tokens"`
	TimeoutMS        int64 `json:"timeout_ms"`
}

type CompletionCondition struct {
	ConditionRef    string `json:"condition_ref"`
	ConditionSHA256 string `json:"condition_sha256"`
}

type Compensation struct {
	Applicability         string                 `json:"applicability"`
	Capability            *op.CapabilityIdentity `json:"capability"`
	RequestedActionSHA256 *string                `json:"requested_action_sha256"`
}

type DecisionOption struct {
	Capability            op.CapabilityIdentity `json:"capability"`
	OptionID              string                `json:"option_id"`
	RequestedActionSHA256 string                `json:"requested_action_sha256"`
}

type ProofObligation struct {
	ObligationID          string   `json:"obligation_id"`
	PredicateSHA256       string   `json:"predicate_sha256"`
	RequiredEvidenceKinds []string `json:"required_evidence_kinds"`
}

type Verifier struct {
	Capability              op.CapabilityIdentity `json:"capability"`
	IndependenceBasisSHA256 string                `json:"independence_basis_sha256"`
	Principal               op.Principal          `json:"principal"`
	TimeoutMS               int64                 `json:"timeout_ms"`
}

type WritePrecondition struct {
	ExpectedSHA256 string `json:"expected_sha256"`
	PreconditionID string `json:"precondition_id"`
	ResourceRef    string `json:"resource_ref"`
}

type DecisionTransaction struct {
	AccountableOwner          op.Principal            `json:"accountable_owner"`
	Actor                     op.Principal            `json:"actor"`
	APIVersion                string                  `json:"api_version"`
	Attestations              DecisionAttestations    `json:"attestations"`
	Bindings                  op.OperationalBindings  `json:"bindings"`
	Budget                    Budget                  `json:"budget"`
	Canonicalization          string                  `json:"canonicalization"`
	Compensation              Compensation            `json:"compensation"`
	CompletionCondition       CompletionCondition     `json:"completion_condition"`
	CreatedAtUnixMS           int64                   `json:"created_at_unix_ms"`
	DecisionTransactionID     string                  `json:"decision_transaction_id"`
	DecisionTransactionSHA256 string                  `json:"decision_transaction_sha256"`
	GoalAtomRef               AtomRef                 `json:"goal_atom_ref"`
	GuardAtomRefs             []AtomRef               `json:"guard_atom_refs"`
	IdempotencyKey            string                  `json:"idempotency_key"`
	Kind                      string                  `json:"kind"`
	Options                   []DecisionOption        `json:"options"`
	ProofObligations          []ProofObligation       `json:"proof_obligations"`
	ReadArtifactReceiptRefs   []op.ArtifactReceiptRef `json:"read_artifact_receipt_refs"`
	SelectedOptionID          string                  `json:"selected_option_id"`
	SelectionBasisSHA256      string                  `json:"selection_basis_sha256"`
	TaskBinding               op.TaskBinding          `json:"task_binding"`
	TransactionMode           string                  `json:"transaction_mode"`
	TriggerAtomRefs           []AtomRef               `json:"trigger_atom_refs"`
	Verifier                  Verifier                `json:"verifier"`
	WritePreconditions        []WritePrecondition     `json:"write_preconditions"`
	WriteSlots                []string                `json:"write_slots"`
}

type KernelDecisionReferenceClosure struct {
	APIVersion          string                               `json:"api_version"`
	Attestations        DecisionAttestations                 `json:"attestations"`
	Canonicalization    string                               `json:"canonicalization"`
	ClosureID           string                               `json:"closure_id"`
	ClosureSHA256       string                               `json:"closure_sha256"`
	CognitiveAtoms      []CognitiveAtom                      `json:"cognitive_atoms"`
	DecisionTransaction DecisionTransaction                  `json:"decision_transaction"`
	Kind                string                               `json:"kind"`
	OperationalClosure  op.KernelOperationalReferenceClosure `json:"operational_closure"`
	Result              string                               `json:"result"`
}
