package graphdispatch

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"

	"forgeos/forge-core/internal/graphplan"
)

type requestPayload struct {
	SystemPrompt              string   `json:"system_prompt"`
	SystemPromptBytes         uint64   `json:"system_prompt_bytes"`
	SystemPromptSHA256        string   `json:"system_prompt_sha256"`
	UserPrompt                string   `json:"user_prompt"`
	UserPromptBytes           uint64   `json:"user_prompt_bytes"`
	UserPromptSHA256          string   `json:"user_prompt_sha256"`
	PredecessorResultReceipts []string `json:"predecessor_result_receipts"`
	Tools                     []string `json:"tools"`
}

type contractPayload struct {
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
}

type manifestDigestView struct {
	Edges   []graphplan.Edge     `json:"edges"`
	Manager graphplan.Manager    `json:"manager"`
	Nodes   []manifestNodeDigest `json:"nodes"`
	Source  manifestSourceDigest `json:"source"`
	V       uint16               `json:"v"`
	Waves   [][]string           `json:"waves"`
}

type manifestNodeDigest struct {
	Acceptance   string `json:"acceptance"`
	AgentProfile string `json:"agent_profile"`
	MemberRole   string `json:"member_role"`
	NodeID       string `json:"node_id"`
	ProjectID    string `json:"project_id"`
	Task         string `json:"task"`
}

type manifestSourceDigest struct {
	ContextSliceSHA256 string `json:"context_slice_sha256"`
	ContextVersion     uint16 `json:"context_version"`
	GroupID            string `json:"group_id"`
	GroupRunID         string `json:"group_run_id"`
	GroupRunVersion    uint16 `json:"group_run_version"`
	SnapshotBytes      uint64 `json:"snapshot_bytes"`
	SnapshotSHA256     string `json:"snapshot_sha256"`
}

func canonicalBytes(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, errInvalidControl
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, errInvalidControl
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func domainDigest(domain string, value any) (string, error) {
	encoded, err := canonicalBytes(value)
	if err != nil {
		return "", err
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

func snapshotPayload(snapshot ControlSnapshot) controlSnapshotPayload {
	return controlSnapshotPayload{
		V: snapshot.V, SchedulerProtocolVersion: snapshot.SchedulerProtocolVersion,
		GraphRunVersion: snapshot.GraphRunVersion, GraphRunID: snapshot.GraphRunID,
		GraphID: snapshot.GraphID, SourceSnapshotSHA256: snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256: snapshot.GraphManifestSHA256,
		CorePlanSHA256:      snapshot.CorePlanSHA256, LastEventSeq: snapshot.LastEventSeq,
		LastEventSHA256:           snapshot.LastEventSHA256,
		ExecutionContractPresent:  snapshot.ExecutionContractPresent,
		DispatchAuthorityReleased: snapshot.DispatchAuthorityReleased,
		Plan:                      snapshot.Plan, Manifest: snapshot.Manifest,
	}
}

func manifestDigest(manifest GraphManifest) (string, error) {
	view := manifestDigestView{
		Edges: manifest.Edges, Manager: manifest.Manager,
		Nodes:  manifestDigestNodes(manifest.Nodes),
		Source: manifestDigestSource(manifest.Source),
		V:      manifest.V, Waves: manifest.Waves,
	}
	return domainDigest(manifestDigestDomain, view)
}

func manifestDigestNodes(nodes []graphplan.Node) []manifestNodeDigest {
	result := make([]manifestNodeDigest, len(nodes))
	for index, node := range nodes {
		result[index] = manifestNodeDigest{
			Acceptance: node.Acceptance, AgentProfile: node.AgentProfile,
			MemberRole: node.MemberRole, NodeID: node.NodeID,
			ProjectID: node.ProjectID, Task: node.Task,
		}
	}
	return result
}

func manifestDigestSource(source GraphSource) manifestSourceDigest {
	return manifestSourceDigest{
		ContextSliceSHA256: source.ContextSliceSHA256,
		ContextVersion:     source.ContextVersion, GroupID: source.GroupID,
		GroupRunID: source.GroupRunID, GroupRunVersion: source.GroupRunVersion,
		SnapshotBytes: source.SnapshotBytes, SnapshotSHA256: source.SnapshotSHA256,
	}
}

func requestPayloadFrom(request NodeRequest) requestPayload {
	return requestPayload{
		SystemPrompt: request.SystemPrompt, SystemPromptBytes: request.SystemPromptBytes,
		SystemPromptSHA256: request.SystemPromptSHA256, UserPrompt: request.UserPrompt,
		UserPromptBytes: request.UserPromptBytes, UserPromptSHA256: request.UserPromptSHA256,
		PredecessorResultReceipts: request.PredecessorResultReceipts,
		Tools:                     request.Tools,
	}
}

func contractPayloadFrom(contract NodeExecutionContract) contractPayload {
	return contractPayload{
		V: contract.V, SchedulerProtocolVersion: contract.SchedulerProtocolVersion,
		NodeExecutionProtocolVersion: contract.NodeExecutionProtocolVersion,
		GraphRunID:                   contract.GraphRunID, GraphID: contract.GraphID,
		SourceSnapshotSHA256:    contract.SourceSnapshotSHA256,
		GraphManifestSHA256:     contract.GraphManifestSHA256,
		CorePlanSHA256:          contract.CorePlanSHA256,
		ControlSnapshotSHA256:   contract.ControlSnapshotSHA256,
		ExpectedLastEventSeq:    contract.ExpectedLastEventSeq,
		ExpectedLastEventSHA256: contract.ExpectedLastEventSHA256,
		Node:                    contract.Node, Workspace: contract.Workspace, Provider: contract.Provider,
		Request: contract.Request, Budgets: contract.Budgets, Approval: contract.Approval,
		Result: contract.Result, Failure: contract.Failure,
		ExecutionContractPresent:  contract.ExecutionContractPresent,
		DispatchAuthorityReleased: contract.DispatchAuthorityReleased,
	}
}

// MarshalContract returns exact canonical contract JSON without a trailing LF.
func MarshalContract(contract NodeExecutionContract) ([]byte, error) {
	if err := validateContract(contract); err != nil {
		return nil, err
	}
	return canonicalBytes(contract)
}
