package outputbinding

// ManifestItem is a content-addressed, regular-file artifact observation.
// Capture and stable-read enforcement belong to the producer, not this value.
type ManifestItem struct {
	Bytes  int64  `json:"bytes"`
	Path   string `json:"path"`
	SHA256 string `json:"sha256"`
}

// ArtifactManifest is a sorted, unique set of observed artifact files.
type ArtifactManifest struct {
	APIVersion       string         `json:"api_version"`
	Canonicalization string         `json:"canonicalization"`
	Items            []ManifestItem `json:"items"`
	ManifestSHA256   string         `json:"manifest_sha256"`
}

// RuntimePolicyBinding records the effective local runtime inputs actually
// used for one command. It is neither a PDP decision nor an authority record.
type RuntimePolicyBinding struct {
	ADR                   bool     `json:"adr"`
	Agent                 string   `json:"agent"`
	APIVersion            string   `json:"api_version"`
	BindingSHA256         string   `json:"binding_sha256"`
	BuildHalt             bool     `json:"build_halt"`
	Canonicalization      string   `json:"canonicalization"`
	DesignDepth           string   `json:"design_depth"`
	DiscoverDepth         string   `json:"discover_depth"`
	Effect                string   `json:"effect"`
	EvolveAuthority       string   `json:"evolve_authority"`
	EvolveDepth           string   `json:"evolve_depth"`
	Executor              string   `json:"executor"`
	FreshContext          bool     `json:"fresh_context"`
	Gates                 []string `json:"gates"`
	Lifecycle             string   `json:"lifecycle"`
	Materiality           string   `json:"materiality"`
	Mode                  string   `json:"mode"`
	Model                 string   `json:"model"`
	OutputBindingContract string   `json:"output_binding_contract"`
	Phase                 string   `json:"phase"`
	Readonly              bool     `json:"readonly"`
	ReviewDepth           string   `json:"review_depth"`
	Reviewer              bool     `json:"reviewer"`
	Stage                 string   `json:"stage"`
	VerdictContract       string   `json:"verdict_contract"`
	WorkflowSHA256        string   `json:"workflow_sha256"`
}

// PreflightBinding is the challenge-bearing set of facts fixed immediately
// before an agent process is started.
type PreflightBinding struct {
	APIVersion               string `json:"api_version"`
	ArtifactInputsSHA256     string `json:"artifact_inputs_sha256"`
	Attempt                  int64  `json:"attempt"`
	BindingSHA256            string `json:"binding_sha256"`
	Canonicalization         string `json:"canonicalization"`
	Challenge                string `json:"challenge"`
	LocalRuntimePolicySHA256 string `json:"local_runtime_policy_sha256"`
	Phase                    string `json:"phase"`
	ProfileID                string `json:"profile_id"`
	PromptContextSHA256      string `json:"prompt_context_sha256"`
	RunID                    string `json:"run_id"`
	SourceBeforeSHA256       string `json:"source_before_sha256"`
	Workflow                 string `json:"workflow"`
	WorkflowSHA256           string `json:"workflow_sha256"`
}

// AgentOutputReceipt records the exact accepted output bytes and all local
// digest inputs without storing the raw or semantic output content itself.
type AgentOutputReceipt struct {
	Agent                    string               `json:"agent"`
	APIVersion               string               `json:"api_version"`
	ArtifactInputs           ArtifactManifest     `json:"artifact_inputs"`
	ArtifactInputsSHA256     string               `json:"artifact_inputs_sha256"`
	ArtifactOutputs          ArtifactManifest     `json:"artifact_outputs"`
	ArtifactOutputsSHA256    string               `json:"artifact_outputs_sha256"`
	Attempt                  int64                `json:"attempt"`
	BindingSHA256            string               `json:"binding_sha256"`
	Canonicalization         string               `json:"canonicalization"`
	Challenge                string               `json:"challenge"`
	Executor                 string               `json:"executor"`
	FinalPromptSHA256        string               `json:"final_prompt_sha256"`
	Kind                     string               `json:"kind"`
	LedgerSequence           int64                `json:"ledger_sequence"`
	LocalRuntimePolicySHA256 string               `json:"local_runtime_policy_sha256"`
	Model                    string               `json:"model"`
	ObservedAtUnixMS         int64                `json:"observed_at_unix_ms"`
	Phase                    string               `json:"phase"`
	PriorReceiptSHA256       *string              `json:"prior_receipt_sha256"`
	ProfileID                string               `json:"profile_id"`
	PromptContextSHA256      string               `json:"prompt_context_sha256"`
	RawOutputBytes           int64                `json:"raw_output_bytes"`
	RawOutputSHA256          string               `json:"raw_output_sha256"`
	ReceiptSHA256            string               `json:"receipt_sha256"`
	RunID                    string               `json:"run_id"`
	RuntimePolicy            RuntimePolicyBinding `json:"runtime_policy"`
	SemanticOutputBytes      int64                `json:"semantic_output_bytes"`
	SemanticOutputSHA256     string               `json:"semantic_output_sha256"`
	SourceAfterSHA256        string               `json:"source_after_sha256"`
	SourceBeforeSHA256       string               `json:"source_before_sha256"`
	SourceRevision           string               `json:"source_revision"`
	SourceStateProfile       string               `json:"source_state_profile"`
	Verdict                  *string              `json:"verdict"`
	Workflow                 string               `json:"workflow"`
}
