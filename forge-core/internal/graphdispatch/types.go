// Package graphdispatch validates one private Graph Run control snapshot and
// builds the sole passive first-node execution contract owned by forge-core.
// It never reads credentials or releases provider, tool, or workspace effects.
package graphdispatch

import "forgeos/forge-core/internal/graphplan"

const (
	ControlSnapshotVersion       uint16 = 1
	NodeExecutionProtocolVersion uint16 = 1
	MaxControlSnapshotBytes             = 4 * 1024 * 1024
	MaxEndpointBytes                    = 2 * 1024
	MaxModelBytes                       = 128
	MaxOutputTokens              uint64 = 32_768
	MaxModelOutputBytes          uint64 = 64 * 1024
	MaxModelEvents               uint64 = 4_096
	MaxTimeoutMilliseconds       uint64 = 86_400_000
	MaxCostUSDMicros             uint64 = 1_000_000_000_000
	MaxResultBytes               uint64 = 512 * 1024
)

const (
	snapshotDigestDomain = "forge.group-agent-graph-control-snapshot.v1\x00"
	manifestDigestDomain = "forge.group-agent-graph-manifest.v1\x00"
	projectLaneDomain    = "forge.group-agent-project-lane.v1\x00"
	requestDigestDomain  = "forge.group-agent-node-request.v1\x00"
	contractDigestDomain = "forge.group-agent-node-execution-contract.v1\x00"
)

// ControlSnapshot is Rust's exact, private, fully validated Graph Run export.
type ControlSnapshot struct {
	V                         uint16         `json:"v"`
	SchedulerProtocolVersion  uint16         `json:"scheduler_protocol_version"`
	GraphRunVersion           uint16         `json:"graph_run_version"`
	GraphRunID                string         `json:"graph_run_id"`
	GraphID                   string         `json:"graph_id"`
	SourceSnapshotSHA256      string         `json:"source_snapshot_sha256"`
	GraphManifestSHA256       string         `json:"graph_manifest_sha256"`
	CorePlanSHA256            string         `json:"core_plan_sha256"`
	LastEventSeq              uint64         `json:"last_event_seq"`
	LastEventSHA256           string         `json:"last_event_sha256"`
	ExecutionContractPresent  bool           `json:"execution_contract_present"`
	DispatchAuthorityReleased bool           `json:"dispatch_authority_released"`
	Plan                      graphplan.Plan `json:"plan"`
	Manifest                  GraphManifest  `json:"manifest"`
	SnapshotSHA256            string         `json:"snapshot_sha256"`
}

type controlSnapshotPayload struct {
	V                         uint16         `json:"v"`
	SchedulerProtocolVersion  uint16         `json:"scheduler_protocol_version"`
	GraphRunVersion           uint16         `json:"graph_run_version"`
	GraphRunID                string         `json:"graph_run_id"`
	GraphID                   string         `json:"graph_id"`
	SourceSnapshotSHA256      string         `json:"source_snapshot_sha256"`
	GraphManifestSHA256       string         `json:"graph_manifest_sha256"`
	CorePlanSHA256            string         `json:"core_plan_sha256"`
	LastEventSeq              uint64         `json:"last_event_seq"`
	LastEventSHA256           string         `json:"last_event_sha256"`
	ExecutionContractPresent  bool           `json:"execution_contract_present"`
	DispatchAuthorityReleased bool           `json:"dispatch_authority_released"`
	Plan                      graphplan.Plan `json:"plan"`
	Manifest                  GraphManifest  `json:"manifest"`
}

// GraphManifest is the exact version-1 Group Agent Graph manifest.
type GraphManifest struct {
	V       uint16            `json:"v"`
	Source  GraphSource       `json:"source"`
	Manager graphplan.Manager `json:"manager"`
	Nodes   []graphplan.Node  `json:"nodes"`
	Edges   []graphplan.Edge  `json:"edges"`
	Waves   [][]string        `json:"waves"`
}

// GraphSource binds the Graph to one exact frozen Group Run snapshot.
type GraphSource struct {
	GroupRunVersion    uint16 `json:"group_run_version"`
	GroupRunID         string `json:"group_run_id"`
	GroupID            string `json:"group_id"`
	ContextVersion     uint16 `json:"context_version"`
	ContextSliceSHA256 string `json:"context_slice_sha256"`
	SnapshotSHA256     string `json:"snapshot_sha256"`
	SnapshotBytes      uint64 `json:"snapshot_bytes"`
}

// ExecutionOptions are all caller-pinned, required model and budget inputs.
type ExecutionOptions struct {
	Endpoint              string
	Model                 string
	MaxOutputTokens       uint64
	MaxModelOutputBytes   uint64
	MaxModelEvents        uint64
	TimeoutMilliseconds   uint64
	MaxCostUSDMicros      uint64
	PricingSnapshotSHA256 string
	MaxResultBytes        uint64
}

// NodeExecutionContract is complete but passive: authority remains false.
type NodeExecutionContract struct {
	V                            uint16           `json:"v"`
	SchedulerProtocolVersion     uint16           `json:"scheduler_protocol_version"`
	NodeExecutionProtocolVersion uint16           `json:"node_execution_protocol_version"`
	GraphRunID                   string           `json:"graph_run_id"`
	GraphID                      string           `json:"graph_id"`
	SourceSnapshotSHA256         string           `json:"source_snapshot_sha256"`
	GraphManifestSHA256          string           `json:"graph_manifest_sha256"`
	CorePlanSHA256               string           `json:"core_plan_sha256"`
	ControlSnapshotSHA256        string           `json:"control_snapshot_sha256"`
	ExpectedLastEventSeq         uint64           `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256      string           `json:"expected_last_event_sha256"`
	Node                         ContractNode     `json:"node"`
	Workspace                    WorkspacePolicy  `json:"workspace"`
	Provider                     ProviderPolicy   `json:"provider"`
	Request                      NodeRequest      `json:"request"`
	Budgets                      ExecutionBudgets `json:"budgets"`
	Approval                     ApprovalPolicy   `json:"approval"`
	Result                       ResultPolicy     `json:"result"`
	Failure                      FailurePolicy    `json:"failure"`
	ExecutionContractPresent     bool             `json:"execution_contract_present"`
	DispatchAuthorityReleased    bool             `json:"dispatch_authority_released"`
	ContractID                   string           `json:"contract_id"`
	ContractSHA256               string           `json:"contract_sha256"`
}

// ContractNode freezes the scheduler selection and its project lane.
type ContractNode struct {
	NodeID            string `json:"node_id"`
	AuthoredNodeIndex uint16 `json:"authored_node_index"`
	TopologyWaveIndex uint16 `json:"topology_wave_index"`
	Attempt           uint16 `json:"attempt"`
	ProjectID         string `json:"project_id"`
	MemberRole        string `json:"member_role"`
	AgentProfile      string `json:"agent_profile"`
	ProjectLaneSHA256 string `json:"project_lane_sha256"`
	SameProjectPolicy string `json:"same_project_policy"`
}

// WorkspacePolicy deliberately grants no workspace capability.
type WorkspacePolicy struct {
	Mode             string   `json:"mode"`
	RootIdentity     *string  `json:"root_identity"`
	IsolationID      *string  `json:"isolation_id"`
	AllowedReadPaths []string `json:"allowed_read_paths"`
}

// ProviderPolicy freezes destination and request transport without a credential.
type ProviderPolicy struct {
	Kind     string `json:"kind"`
	Endpoint string `json:"endpoint"`
	Model    string `json:"model"`
	Store    bool   `json:"store"`
	Stream   bool   `json:"stream"`
}

// NodeRequest freezes exact Prompt bytes and prohibits tools/dataflow.
type NodeRequest struct {
	SystemPrompt              string   `json:"system_prompt"`
	SystemPromptBytes         uint64   `json:"system_prompt_bytes"`
	SystemPromptSHA256        string   `json:"system_prompt_sha256"`
	UserPrompt                string   `json:"user_prompt"`
	UserPromptBytes           uint64   `json:"user_prompt_bytes"`
	UserPromptSHA256          string   `json:"user_prompt_sha256"`
	PredecessorResultReceipts []string `json:"predecessor_result_receipts"`
	Tools                     []string `json:"tools"`
	RequestSHA256             string   `json:"request_sha256"`
}

// ExecutionBudgets are all explicit; no protocol defaults are applied.
type ExecutionBudgets struct {
	MaxTurns              uint16 `json:"max_turns"`
	MaxToolCalls          uint16 `json:"max_tool_calls"`
	MaxOutputTokens       uint64 `json:"max_output_tokens"`
	MaxModelOutputBytes   uint64 `json:"max_model_output_bytes"`
	MaxModelEvents        uint64 `json:"max_model_events"`
	TimeoutMilliseconds   uint64 `json:"timeout_ms"`
	MaxCostUSDMicros      uint64 `json:"max_cost_usd_micros"`
	PricingSnapshotSHA256 string `json:"pricing_snapshot_sha256"`
}

// ApprovalPolicy keeps every future effect behind explicit consent or forbids it.
type ApprovalPolicy struct {
	ProviderDispatch string `json:"provider_dispatch"`
	Workspace        string `json:"workspace"`
	Tools            string `json:"tools"`
	Writeback        string `json:"writeback"`
}

// ResultPolicy fixes a local, bounded, content-addressable future result.
type ResultPolicy struct {
	ArtifactKind          string `json:"artifact_kind"`
	MaxResultBytes        uint64 `json:"max_result_bytes"`
	PredecessorDataflow   string `json:"predecessor_dataflow"`
	ConversationWriteback string `json:"conversation_writeback"`
	PromptWriteback       string `json:"prompt_writeback"`
	MemoryWriteback       string `json:"memory_writeback"`
}

// FailurePolicy makes unknown dispatch terminal for automatic retry purposes.
type FailurePolicy struct {
	AutomaticRetry          bool   `json:"automatic_retry"`
	LeaseRetry              bool   `json:"lease_retry"`
	PostClaimUncertainty    string `json:"post_claim_uncertainty"`
	FailurePropagationOwner string `json:"failure_propagation_owner"`
}
