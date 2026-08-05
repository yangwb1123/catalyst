package graphscheduledcontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"

	"forgeos/forge-core/internal/graphdispatch"
)

type requestPayload struct {
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
}

type candidatePayload struct {
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
}

type userPrompt struct {
	V                 uint16 `json:"v"`
	NodeID            string `json:"node_id"`
	Task              string `json:"task"`
	Acceptance        string `json:"acceptance"`
	PredecessorOutput string `json:"predecessor_output,omitempty"`
}

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidCandidate
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidCandidate
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func domainDigest(domain string, value any) (string, error) {
	encoded, err := canonicalBytes(value)
	if err != nil {
		return "", errInvalidCandidate
	}
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write(encoded)
	return hex.EncodeToString(digest.Sum(nil)), nil
}

func byteDigest(value string) string {
	digest := sha256.Sum256([]byte(value))
	return hex.EncodeToString(digest[:])
}

func rawDomainDigest(domain, value string) string {
	digest := sha256.New()
	_, _ = digest.Write([]byte(domain))
	_, _ = digest.Write([]byte(value))
	return hex.EncodeToString(digest.Sum(nil))
}

func requestPayloadFrom(value ScheduledNodeRequest) requestPayload {
	return requestPayload{
		V: value.V, GraphRunID: value.GraphRunID, ScheduleID: value.ScheduleID,
		ScheduleSHA256: value.ScheduleSHA256, ExecutionOrdinal: value.ExecutionOrdinal,
		NodeID: value.NodeID, Attempt: value.Attempt, SystemPrompt: value.SystemPrompt,
		SystemPromptBytes: value.SystemPromptBytes, SystemPromptSHA256: value.SystemPromptSHA256,
		UserPrompt: value.UserPrompt, UserPromptBytes: value.UserPromptBytes,
		UserPromptSHA256:            value.UserPromptSHA256,
		RequiredPredecessorNodeIDs:  value.RequiredPredecessorNodeIDs,
		PredecessorTerminalReceipts: value.PredecessorTerminalReceipts,
		PredecessorContentIncluded:  value.PredecessorContentIncluded, Tools: value.Tools,
	}
}

func candidatePayloadFrom(value ScheduledNodeContractCandidate) candidatePayload {
	return candidatePayload{
		V: value.V, SchedulerProtocolVersion: value.SchedulerProtocolVersion,
		NodeExecutionProtocolVersion:     value.NodeExecutionProtocolVersion,
		ExecutionScheduleProtocolVersion: value.ExecutionScheduleProtocolVersion,
		ContractScope:                    value.ContractScope, GraphRunID: value.GraphRunID, GraphID: value.GraphID,
		SourceSnapshotSHA256: value.SourceSnapshotSHA256,
		GraphManifestSHA256:  value.GraphManifestSHA256, CorePlanSHA256: value.CorePlanSHA256,
		ControlSnapshotSHA256: value.ControlSnapshotSHA256, ScheduleID: value.ScheduleID,
		ScheduleSHA256: value.ScheduleSHA256, ExpectedLastEventSeq: value.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: value.ExpectedLastEventSHA256, Node: value.Node,
		Request: value.Request, Workspace: value.Workspace, Provider: value.Provider,
		Budgets: value.Budgets, Approval: value.Approval, Result: value.Result,
		Failure: value.Failure, LifecycleContractAdmitted: value.LifecycleContractAdmitted,
		ProviderRequestPresent:     value.ProviderRequestPresent,
		ExecutionAuthorityReleased: value.ExecutionAuthorityReleased,
		DispatchAuthorityReleased:  value.DispatchAuthorityReleased,
		ProgressObserved:           value.ProgressObserved,
		SuccessorAdvanceAuthorized: value.SuccessorAdvanceAuthorized,
	}
}

// MarshalCandidate returns exact compact canonical JSON without a trailing LF.
func MarshalCandidate(value ScheduledNodeContractCandidate) ([]byte, error) {
	if validateCandidate(value) != nil {
		return nil, errInvalidCandidate
	}
	return canonicalBytes(value)
}
