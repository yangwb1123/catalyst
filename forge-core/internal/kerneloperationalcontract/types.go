package kerneloperationalcontract

type Attestations struct {
	Authorization         bool `json:"authorization_attestation"`
	BindingAuthentication bool `json:"binding_authentication_attestation"`
	Completion            bool `json:"completion_attestation"`
	ContentProvenance     bool `json:"content_provenance_attestation"`
	Effect                bool `json:"effect_attestation"`
	EventAppend           bool `json:"event_append_attestation"`
	Execution             bool `json:"execution_attestation"`
	GrantAuthentication   bool `json:"grant_authentication_attestation"`
	Outcome               bool `json:"outcome_attestation"`
	Permission            bool `json:"permission_attestation"`
	Persistence           bool `json:"persistence_attestation"`
	PrincipalAuth         bool `json:"principal_authentication_attestation"`
	Transition            bool `json:"transition_attestation"`
	UsageMeasurement      bool `json:"usage_measurement_attestation"`
}

type Principal struct {
	AuthorityDomain string `json:"authority_domain"`
	PrincipalID     string `json:"principal_id"`
	PrincipalType   string `json:"principal_type"`
}

type TaskBinding struct {
	AttemptID        *string `json:"attempt_id"`
	ChangeID         string  `json:"change_id"`
	EnvironmentClass string  `json:"environment_class"`
	EnvironmentID    string  `json:"environment_id"`
	NodeID           string  `json:"node_id"`
	ProjectID        string  `json:"project_id"`
	Role             string  `json:"role"`
	RunID            string  `json:"run_id"`
	TargetID         *string `json:"target_id"`
	TaskID           string  `json:"task_id"`
}

type OperationalBindings struct {
	ContextSHA256        string `json:"context_sha256"`
	EnvironmentProfileID string `json:"environment_profile_id"`
	EnvironmentSHA256    string `json:"environment_sha256"`
	PolicySHA256         string `json:"policy_sha256"`
	SourceProfileID      string `json:"source_profile_id"`
	SourceRevision       string `json:"source_revision"`
	SourceTreeSHA256     string `json:"source_tree_sha256"`
}

type CapabilityIdentity struct {
	CapabilityContractSHA256 string `json:"capability_contract_sha256"`
	CapabilityID             string `json:"capability_id"`
	CapabilityVersion        string `json:"capability_version"`
}

type CapabilityGrantRef struct {
	AuthorityDomain string `json:"authority_domain"`
	GrantID         string `json:"grant_id"`
	GrantSHA256     string `json:"grant_sha256"`
}

type ArtifactRef struct {
	ArtifactKind   string `json:"artifact_kind"`
	ArtifactRef    string `json:"artifact_ref"`
	ArtifactSHA256 string `json:"artifact_sha256"`
}

type ArtifactReceiptRef struct {
	ArtifactReceiptID     string `json:"artifact_receipt_id"`
	ArtifactReceiptSHA256 string `json:"artifact_receipt_sha256"`
}

type CapabilityInvocationRef struct {
	InvocationID     string `json:"invocation_id"`
	InvocationSHA256 string `json:"invocation_sha256"`
}

type InteractionEventRef struct {
	EventID     string `json:"event_id"`
	EventSHA256 string `json:"event_sha256"`
}

type ExecutionReceiptRef struct {
	ExecutionReceiptID     string `json:"execution_receipt_id"`
	ExecutionReceiptSHA256 string `json:"execution_receipt_sha256"`
}

type ObservedUsage struct {
	CallCount     int64 `json:"call_count"`
	CostUSDMicros int64 `json:"cost_usd_micros"`
	ElapsedMS     int64 `json:"elapsed_ms"`
	InputTokens   int64 `json:"input_tokens"`
	NetworkBytes  int64 `json:"network_bytes"`
	OutputBytes   int64 `json:"output_bytes"`
	OutputTokens  int64 `json:"output_tokens"`
}

type ArtifactReceipt struct {
	APIVersion            string                   `json:"api_version"`
	Artifact              ArtifactRef              `json:"artifact"`
	ArtifactReceiptID     string                   `json:"artifact_receipt_id"`
	ArtifactReceiptSHA256 string                   `json:"artifact_receipt_sha256"`
	Attestations          Attestations             `json:"attestations"`
	Bindings              OperationalBindings      `json:"bindings"`
	Canonicalization      string                   `json:"canonicalization"`
	ContentBytes          int64                    `json:"content_bytes"`
	CreatedAtUnixMS       int64                    `json:"created_at_unix_ms"`
	Kind                  string                   `json:"kind"`
	Producer              Principal                `json:"producer"`
	ProducerInvocationRef *CapabilityInvocationRef `json:"producer_invocation_ref"`
	ReceiptRole           string                   `json:"receipt_role"`
	Slot                  string                   `json:"slot"`
	TaskBinding           TaskBinding              `json:"task_binding"`
}

type CapabilityInvocation struct {
	APIVersion               string               `json:"api_version"`
	Attempt                  int64                `json:"attempt"`
	Attestations             Attestations         `json:"attestations"`
	Bindings                 OperationalBindings  `json:"bindings"`
	Canonicalization         string               `json:"canonicalization"`
	Capability               CapabilityIdentity   `json:"capability"`
	CapabilityGrantRef       CapabilityGrantRef   `json:"capability_grant_ref"`
	CorrelationID            string               `json:"correlation_id"`
	DeclaredOutputSlots      []string             `json:"declared_output_slots"`
	IdempotencyKey           string               `json:"idempotency_key"`
	InputArtifactReceiptRefs []ArtifactReceiptRef `json:"input_artifact_receipt_refs"`
	InvocationID             string               `json:"invocation_id"`
	InvocationSHA256         string               `json:"invocation_sha256"`
	Kind                     string               `json:"kind"`
	PriorExecutionReceiptRef *ExecutionReceiptRef `json:"prior_execution_receipt_ref"`
	RequestedActionSHA256    string               `json:"requested_action_sha256"`
	RequestedAtUnixMS        int64                `json:"requested_at_unix_ms"`
	Subject                  Principal            `json:"subject"`
	TaskBinding              TaskBinding          `json:"task_binding"`
}

type InteractionEvent struct {
	Actor             Principal               `json:"actor"`
	APIVersion        string                  `json:"api_version"`
	ArtifactRefs      []ArtifactRef           `json:"artifact_refs"`
	Attestations      Attestations            `json:"attestations"`
	Bindings          OperationalBindings     `json:"bindings"`
	Canonicalization  string                  `json:"canonicalization"`
	CausationEventRef *InteractionEventRef    `json:"causation_event_ref"`
	ConfidenceMicros  *int64                  `json:"confidence_micros"`
	CorrelationID     string                  `json:"correlation_id"`
	EventID           string                  `json:"event_id"`
	EventSHA256       string                  `json:"event_sha256"`
	InvocationRef     CapabilityInvocationRef `json:"invocation_ref"`
	Kind              string                  `json:"kind"`
	LogicalSequence   int64                   `json:"logical_sequence"`
	ObjectRef         string                  `json:"object_ref"`
	OccurredAtUnixMS  int64                   `json:"occurred_at_unix_ms"`
	Target            *Principal              `json:"target"`
	TaskBinding       TaskBinding             `json:"task_binding"`
	Verb              string                  `json:"verb"`
}

type ExecutionReceipt struct {
	APIVersion                string                  `json:"api_version"`
	Attempt                   int64                   `json:"attempt"`
	Attestations              Attestations            `json:"attestations"`
	Bindings                  OperationalBindings     `json:"bindings"`
	Canonicalization          string                  `json:"canonicalization"`
	CorrelationID             string                  `json:"correlation_id"`
	EndedAtUnixMS             int64                   `json:"ended_at_unix_ms"`
	EventRefs                 []InteractionEventRef   `json:"event_refs"`
	ExecutionReceiptID        string                  `json:"execution_receipt_id"`
	ExecutionReceiptSHA256    string                  `json:"execution_receipt_sha256"`
	Executor                  Principal               `json:"executor"`
	InputArtifacts            []ArtifactRef           `json:"input_artifacts"`
	InvocationRef             CapabilityInvocationRef `json:"invocation_ref"`
	Kind                      string                  `json:"kind"`
	ObservedUsage             ObservedUsage           `json:"observed_usage"`
	Outcome                   string                  `json:"outcome"`
	OutputArtifactReceiptRefs []ArtifactReceiptRef    `json:"output_artifact_receipt_refs"`
	PriorExecutionReceiptRef  *ExecutionReceiptRef    `json:"prior_execution_receipt_ref"`
	ReasonCodes               []string                `json:"reason_codes"`
	StartedAtUnixMS           int64                   `json:"started_at_unix_ms"`
	TaskBinding               TaskBinding             `json:"task_binding"`
}

type KernelOperationalReferenceClosure struct {
	APIVersion            string                 `json:"api_version"`
	ArtifactReceipts      []ArtifactReceipt      `json:"artifact_receipts"`
	Artifacts             []ArtifactRef          `json:"artifacts"`
	Attestations          Attestations           `json:"attestations"`
	Canonicalization      string                 `json:"canonicalization"`
	CapabilityInvocations []CapabilityInvocation `json:"capability_invocations"`
	ClosureID             string                 `json:"closure_id"`
	ClosureSHA256         string                 `json:"closure_sha256"`
	ExecutionReceipts     []ExecutionReceipt     `json:"execution_receipts"`
	InteractionEvents     []InteractionEvent     `json:"interaction_events"`
	Kind                  string                 `json:"kind"`
	Result                string                 `json:"result"`
}
