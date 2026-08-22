// Package contextpackagecontract builds a deterministic, authority-free
// ContextPackage v1 projection from exact caller-supplied bytes.
package contextpackagecontract

const (
	requestAPIVersion = "forgeos.context-package-build-request/v1"
	packageAPIVersion = "forgeos.context-package/v1"
	canonicalization  = "forgeos.canonical-json/v1"
	assemblyMode      = "authority_free_deterministic_context_projection"
	assemblyResult    = "ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, completion, persistence, or effect attestation)"
	normalization     = "exact_lf_utf8_after_declared_redactions"
	delimiter         = "structured_json_lane_no_text_delimiter"
	redactionMarker   = "[REDACTED]"
)

type TokenizerIdentity struct {
	TokenizerID     string `json:"tokenizer_id"`
	TokenizerSHA256 string `json:"tokenizer_sha256"`
}

type TokenCounter interface {
	Identity() TokenizerIdentity
	Count([]byte) (uint64, error)
}

type Budget struct {
	MaxContentBytes uint64 `json:"max_content_bytes"`
	MaxSnippets     uint64 `json:"max_snippets"`
	MaxTokens       uint64 `json:"max_tokens"`
	TokenizerID     string `json:"tokenizer_id"`
	TokenizerSHA256 string `json:"tokenizer_sha256"`
}

type TaskBinding struct {
	ChangeID  string `json:"change_id"`
	NodeID    string `json:"node_id"`
	Phase     string `json:"phase"`
	ProjectID string `json:"project_id"`
	Role      string `json:"role"`
	RunID     string `json:"run_id"`
	TaskID    string `json:"task_id"`
}

type SourceBinding struct {
	AsOfUnixMS       int64  `json:"as_of_unix_ms"`
	PolicySHA256     string `json:"policy_sha256"`
	RoutesSHA256     string `json:"routes_sha256"`
	SourceRevision   string `json:"source_revision"`
	SourceTreeSHA256 string `json:"source_tree_sha256"`
}

type Source struct {
	Availability    string  `json:"availability"`
	Category        string  `json:"category"`
	Content         *string `json:"content"`
	ContentSHA256   *string `json:"content_sha256"`
	DeclaredLane    string  `json:"declared_lane"`
	DeclaredTrust   string  `json:"declared_trust"`
	Disposition     string  `json:"disposition"`
	ExpiresAtUnixMS *int64  `json:"expires_at_unix_ms"`
	Freshness       string  `json:"freshness"`
	InjectionRisk   string  `json:"injection_risk"`
	MaxBytes        uint64  `json:"max_bytes"`
	Priority        uint64  `json:"priority"`
	Required        bool    `json:"required"`
	SourceClass     string  `json:"source_class"`
	SourceID        string  `json:"source_id"`
	SourceRef       string  `json:"source_ref"`
	SourceRevision  string  `json:"source_revision"`
	Truncation      string  `json:"truncation"`
}

type RedactionRange struct {
	EndByte   uint64 `json:"end_byte"`
	RuleID    string `json:"rule_id"`
	StartByte uint64 `json:"start_byte"`
}

type Redaction struct {
	Ranges   []RedactionRange `json:"ranges"`
	SourceID string           `json:"source_id"`
}

type BuildRequest struct {
	APIVersion       string        `json:"api_version"`
	Budget           Budget        `json:"budget"`
	Canonicalization string        `json:"canonicalization"`
	Redactions       []Redaction   `json:"redactions"`
	SourceBinding    SourceBinding `json:"source_binding"`
	Sources          []Source      `json:"sources"`
	TaskBinding      TaskBinding   `json:"task_binding"`
}

type Truncation struct {
	OriginalRedactedBytes uint64 `json:"original_redacted_bytes"`
	Reason                string `json:"reason"`
	RetainedBytes         uint64 `json:"retained_bytes"`
}

type Snippet struct {
	Category               string      `json:"category"`
	Content                string      `json:"content"`
	DeclaredLane           string      `json:"declared_lane"`
	DeclaredTrust          string      `json:"declared_trust"`
	Delimiter              string      `json:"delimiter"`
	InstructionAllowed     bool        `json:"instruction_allowed"`
	Lane                   string      `json:"lane"`
	Normalization          string      `json:"normalization"`
	ProjectedContentSHA256 string      `json:"projected_content_sha256"`
	Required               bool        `json:"required"`
	SelectionReason        string      `json:"selection_reason"`
	SnippetSHA256          string      `json:"snippet_sha256"`
	SourceClass            string      `json:"source_class"`
	SourceContentSHA256    string      `json:"source_content_sha256"`
	SourceID               string      `json:"source_id"`
	SourceRef              string      `json:"source_ref"`
	SourceRevision         string      `json:"source_revision"`
	Truncation             *Truncation `json:"truncation"`
}

type Omission struct {
	Reason    string `json:"reason"`
	SourceID  string `json:"source_id"`
	SourceRef string `json:"source_ref"`
}

type RedactionReceipt struct {
	Ranges   []RedactionRange `json:"ranges"`
	SourceID string           `json:"source_id"`
}

type Accounting struct {
	ActualTokens          uint64 `json:"actual_tokens"`
	CandidateCount        uint64 `json:"candidate_count"`
	ContentBytes          uint64 `json:"content_bytes"`
	OmittedSourceCount    uint64 `json:"omitted_source_count"`
	RedactedRangeCount    uint64 `json:"redacted_range_count"`
	SelectedSnippetCount  uint64 `json:"selected_snippet_count"`
	TruncatedSnippetCount uint64 `json:"truncated_snippet_count"`
}

type Freshness struct {
	EvaluatedAtUnixMS int64  `json:"evaluated_at_unix_ms"`
	ExpiresAtUnixMS   *int64 `json:"expires_at_unix_ms"`
}

type Lanes struct {
	InstructionCandidates []Snippet `json:"instruction_candidates"`
	TrustedContext        []Snippet `json:"trusted_context"`
	UntrustedData         []Snippet `json:"untrusted_data"`
}

type ContextPackage struct {
	Accounting        Accounting         `json:"accounting"`
	APIVersion        string             `json:"api_version"`
	AssemblyMode      string             `json:"assembly_mode"`
	Budget            Budget             `json:"budget"`
	CacheKeySHA256    string             `json:"cache_key_sha256"`
	Canonicalization  string             `json:"canonicalization"`
	ContextSHA256     string             `json:"context_sha256"`
	Freshness         Freshness          `json:"freshness"`
	Lanes             Lanes              `json:"lanes"`
	Omissions         []Omission         `json:"omissions"`
	ProjectionSHA256  string             `json:"projection_sha256"`
	RedactionReceipts []RedactionReceipt `json:"redaction_receipts"`
	RequestSHA256     string             `json:"request_sha256"`
	Result            string             `json:"result"`
	SourceBinding     SourceBinding      `json:"source_binding"`
	TaskBinding       TaskBinding        `json:"task_binding"`
}
