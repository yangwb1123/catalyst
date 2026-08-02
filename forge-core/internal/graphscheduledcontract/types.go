// Package graphscheduledcontract builds and validates one passive, schedule-
// bound initial-node contract candidate. It grants no lifecycle or dispatch
// authority and cannot select a successor.
package graphscheduledcontract

import (
	"errors"

	"forgeos/forge-core/internal/graphdispatch"
)

const (
	CandidateVersion             uint16 = 2
	NodeExecutionProtocolVersion uint16 = 2
	RequestVersion               uint16 = 2
	MaxCandidateBytes                   = 4 * 1024 * 1024
)

const (
	contractScope        = "schedule_initial_node_only"
	projectLaneDomain    = "forge.group-agent-project-lane.v1\x00"
	requestDigestDomain  = "forge.group-agent-scheduled-node-request.v2\x00"
	contractDigestDomain = "forge.group-agent-scheduled-node-contract.v2\x00"
	requestIDPrefix      = "scheduled-node-request-"
	contractIDPrefix     = "scheduled-node-contract-"
)

const maxProseBytes = 64 * 1024

var errInvalidCandidate = errors.New("invalid scheduled node contract candidate")

// CandidateNode freezes Core's ordinal-zero schedule selection and private
// manifest labels without granting its Project lane.
type CandidateNode struct {
	ExecutionOrdinal  uint16 `json:"execution_ordinal"`
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

// PredecessorTerminalReceipt reserves the evidence shape a later protocol
// must verify. Candidate v2 rejects every non-empty instance.
type PredecessorTerminalReceipt struct {
	PredecessorNodeID     string `json:"predecessor_node_id"`
	PredecessorAttempt    uint16 `json:"predecessor_attempt"`
	TerminalEventSeq      uint64 `json:"terminal_event_seq"`
	TerminalEventSHA256   string `json:"terminal_event_sha256"`
	TerminalReceiptID     string `json:"terminal_receipt_id"`
	TerminalReceiptSHA256 string `json:"terminal_receipt_sha256"`
	NodeOutcome           string `json:"node_outcome"`
}

// ScheduledNodeRequest is local logical-request evidence. Predecessor receipt
// identities are never copied into its provider-facing user Prompt.
type ScheduledNodeRequest struct {
	V                           uint16                       `json:"v"`
	GraphRunID                  string                       `json:"graph_run_id"`
	ScheduleID                  string                       `json:"schedule_id"`
	ScheduleSHA256              string                       `json:"schedule_sha256"`
	ExecutionOrdinal            uint16                       `json:"execution_ordinal"`
	NodeID                      string                       `json:"node_id"`
	Attempt                     uint16                       `json:"attempt"`
	SystemPrompt                string                       `json:"system_prompt"`
	SystemPromptBytes           uint64                       `json:"system_prompt_bytes"`
	SystemPromptSHA256          string                       `json:"system_prompt_sha256"`
	UserPrompt                  string                       `json:"user_prompt"`
	UserPromptBytes             uint64                       `json:"user_prompt_bytes"`
	UserPromptSHA256            string                       `json:"user_prompt_sha256"`
	RequiredPredecessorNodeIDs  []string                     `json:"required_predecessor_node_ids"`
	PredecessorTerminalReceipts []PredecessorTerminalReceipt `json:"predecessor_terminal_receipts"`
	PredecessorContentIncluded  bool                         `json:"predecessor_content_included"`
	Tools                       []string                     `json:"tools"`
	RequestID                   string                       `json:"request_id"`
	RequestSHA256               string                       `json:"request_sha256"`
}

// ScheduledNodeContractCandidate is a passive initial-node candidate, not an
// admitted lifecycle contract or a provider-dispatch authorization.
type ScheduledNodeContractCandidate struct {
	V                                uint16                         `json:"v"`
	SchedulerProtocolVersion         uint16                         `json:"scheduler_protocol_version"`
	NodeExecutionProtocolVersion     uint16                         `json:"node_execution_protocol_version"`
	ExecutionScheduleProtocolVersion uint16                         `json:"execution_schedule_protocol_version"`
	ContractScope                    string                         `json:"contract_scope"`
	GraphRunID                       string                         `json:"graph_run_id"`
	GraphID                          string                         `json:"graph_id"`
	SourceSnapshotSHA256             string                         `json:"source_snapshot_sha256"`
	GraphManifestSHA256              string                         `json:"graph_manifest_sha256"`
	CorePlanSHA256                   string                         `json:"core_plan_sha256"`
	ControlSnapshotSHA256            string                         `json:"control_snapshot_sha256"`
	ScheduleID                       string                         `json:"schedule_id"`
	ScheduleSHA256                   string                         `json:"schedule_sha256"`
	ExpectedLastEventSeq             uint64                         `json:"expected_last_event_seq"`
	ExpectedLastEventSHA256          string                         `json:"expected_last_event_sha256"`
	Node                             CandidateNode                  `json:"node"`
	Request                          ScheduledNodeRequest           `json:"request"`
	Workspace                        graphdispatch.WorkspacePolicy  `json:"workspace"`
	Provider                         graphdispatch.ProviderPolicy   `json:"provider"`
	Budgets                          graphdispatch.ExecutionBudgets `json:"budgets"`
	Approval                         graphdispatch.ApprovalPolicy   `json:"approval"`
	Result                           graphdispatch.ResultPolicy     `json:"result"`
	Failure                          graphdispatch.FailurePolicy    `json:"failure"`
	LifecycleContractAdmitted        bool                           `json:"lifecycle_contract_admitted"`
	ProviderRequestPresent           bool                           `json:"provider_request_present"`
	ExecutionAuthorityReleased       bool                           `json:"execution_authority_released"`
	DispatchAuthorityReleased        bool                           `json:"dispatch_authority_released"`
	ProgressObserved                 bool                           `json:"progress_observed"`
	SuccessorAdvanceAuthorized       bool                           `json:"successor_advance_authorized"`
	ContractID                       string                         `json:"contract_id"`
	ContractSHA256                   string                         `json:"contract_sha256"`
}
