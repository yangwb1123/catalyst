// Package approvalcontext defines the authority-neutral local context and
// positive-decision marker used to bind a human-gate signal to accepted output
// observations. These values are local references, not authenticated approval.
package approvalcontext

const (
	ContextFormat = "forgeos.approval-context.v1"
	MarkerFormat  = "forgeos.approval.v3"
	ContextDomain = "forgeos.local-approval-context.v1\x00"
	maxWireBytes  = 64 << 10
	maxSafeInt    = int64(1<<53 - 1)
)

type Context struct {
	Format                   string `json:"_format"`
	AgentOutputReceiptSHA256 string `json:"agent_output_receipt_sha256"`
	ArtifactInputsSHA256     string `json:"artifact_inputs_sha256"`
	ArtifactOutputsSHA256    string `json:"artifact_outputs_sha256"`
	CreatedAtUnixMS          int64  `json:"created_at_unix_ms"`
	LocalRuntimePolicySHA256 string `json:"local_runtime_policy_sha256"`
	PromptContextSHA256      string `json:"prompt_context_sha256"`
	RunID                    string `json:"run_id"`
	SourceAfterSHA256        string `json:"source_after_sha256"`
	Stage                    string `json:"stage"`
	Workflow                 string `json:"workflow"`
	WorkflowSHA256           string `json:"workflow_sha256"`
}

type PositiveMarker struct {
	Format                   string `json:"_format"`
	ActorHint                string `json:"actor_hint"`
	AgentOutputReceiptSHA256 string `json:"agent_output_receipt_sha256"`
	ApprovalContextSHA256    string `json:"approval_context_sha256"`
	ArtifactInputsSHA256     string `json:"artifact_inputs_sha256"`
	ArtifactOutputsSHA256    string `json:"artifact_outputs_sha256"`
	CreatedAtUnixMS          int64  `json:"created_at_unix_ms"`
	Decision                 string `json:"decision"`
	LocalRuntimePolicySHA256 string `json:"local_runtime_policy_sha256"`
	PromptContextSHA256      string `json:"prompt_context_sha256"`
	RunID                    string `json:"run_id"`
	SourceAfterSHA256        string `json:"source_after_sha256"`
	Stage                    string `json:"stage"`
	Workflow                 string `json:"workflow"`
	WorkflowSHA256           string `json:"workflow_sha256"`
}
