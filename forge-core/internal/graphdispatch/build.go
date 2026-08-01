package graphdispatch

import "forgeos/forge-core/internal/graphplan"

const systemPromptPrefix = "Execute exactly one frozen Group Agent Graph node. " +
	"Follow the manager instruction, complete only the assigned task, and return a text result " +
	"that can be checked against the acceptance criteria. Tools, network, workspace access, " +
	"memory, and writeback are unavailable.\n\nManager instruction:\n"

type userPrompt struct {
	V                         uint16   `json:"v"`
	NodeID                    string   `json:"node_id"`
	Task                      string   `json:"task"`
	Acceptance                string   `json:"acceptance"`
	PredecessorResultReceipts []string `json:"predecessor_result_receipts"`
}

// Build validates the exact scheduler snapshot and emits its sole first-node
// execution contract. The resulting contract is complete but releases no effect.
func Build(
	snapshot ControlSnapshot,
	options ExecutionOptions,
) (NodeExecutionContract, error) {
	if validateControl(snapshot) != nil || validateOptions(options) != nil {
		return NodeExecutionContract{}, errInvalidControl
	}
	node, authoredIndex, err := selectFirstNode(snapshot)
	if err != nil {
		return NodeExecutionContract{}, errInvalidControl
	}
	request, err := buildRequest(snapshot.Manifest.Manager.Instruction, node)
	if err != nil {
		return NodeExecutionContract{}, errInvalidControl
	}
	contract := baseContract(snapshot, options, node, authoredIndex, request)
	digest, err := domainDigest(contractDigestDomain, contractPayloadFrom(contract))
	if err != nil {
		return NodeExecutionContract{}, errInvalidControl
	}
	contract.ContractID = "node-contract-" + digest
	contract.ContractSHA256 = digest
	if validateContract(contract) != nil {
		return NodeExecutionContract{}, errInvalidControl
	}
	return contract, nil
}

func selectFirstNode(
	snapshot ControlSnapshot,
) (graphplan.Node, uint16, error) {
	if len(snapshot.Plan.Waves) == 0 || len(snapshot.Plan.Waves[0]) == 0 {
		return graphplan.Node{}, 0, errInvalidControl
	}
	selected := snapshot.Plan.Waves[0][0]
	for index, node := range snapshot.Manifest.Nodes {
		if node.NodeID == selected {
			position, err := checkedNodeIndex(index)
			return node, position, err
		}
	}
	return graphplan.Node{}, 0, errInvalidControl
}

func checkedNodeIndex(index int) (uint16, error) {
	position := uint16(index)
	if int(position) != index || index >= 32 {
		return 0, errInvalidControl
	}
	return position, nil
}

func buildRequest(managerInstruction string, node graphplan.Node) (NodeRequest, error) {
	system := systemPromptPrefix + managerInstruction
	userBytes, err := canonicalBytes(userPrompt{
		V: 1, NodeID: node.NodeID, Task: node.Task, Acceptance: node.Acceptance,
		PredecessorResultReceipts: []string{},
	})
	if err != nil {
		return NodeRequest{}, err
	}
	request := NodeRequest{
		SystemPrompt: system, SystemPromptBytes: uint64(len(system)),
		SystemPromptSHA256: byteDigest(system), UserPrompt: string(userBytes),
		UserPromptBytes: uint64(len(userBytes)), UserPromptSHA256: byteDigest(string(userBytes)),
		PredecessorResultReceipts: []string{}, Tools: []string{},
	}
	digest, err := domainDigest(requestDigestDomain, requestPayloadFrom(request))
	if err != nil {
		return NodeRequest{}, err
	}
	request.RequestSHA256 = digest
	return request, nil
}

func baseContract(
	snapshot ControlSnapshot,
	options ExecutionOptions,
	node graphplan.Node,
	authoredIndex uint16,
	request NodeRequest,
) NodeExecutionContract {
	return NodeExecutionContract{
		V: 1, SchedulerProtocolVersion: graphplan.SchedulerProtocolVersion,
		NodeExecutionProtocolVersion: NodeExecutionProtocolVersion,
		GraphRunID:                   snapshot.GraphRunID, GraphID: snapshot.GraphID,
		SourceSnapshotSHA256:    snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256:     snapshot.GraphManifestSHA256,
		CorePlanSHA256:          snapshot.CorePlanSHA256,
		ControlSnapshotSHA256:   snapshot.SnapshotSHA256,
		ExpectedLastEventSeq:    snapshot.LastEventSeq,
		ExpectedLastEventSHA256: snapshot.LastEventSHA256,
		Node:                    contractNode(node, authoredIndex), Workspace: emptyWorkspace(),
		Provider: providerPolicy(options), Request: request,
		Budgets: executionBudgets(options), Approval: approvalPolicy(),
		Result: resultPolicy(options.MaxResultBytes), Failure: failurePolicy(),
		ExecutionContractPresent: true, DispatchAuthorityReleased: false,
	}
}

func contractNode(node graphplan.Node, authoredIndex uint16) ContractNode {
	return ContractNode{
		NodeID: node.NodeID, AuthoredNodeIndex: authoredIndex,
		TopologyWaveIndex: 0, Attempt: 1, ProjectID: node.ProjectID,
		MemberRole: node.MemberRole, AgentProfile: node.AgentProfile,
		ProjectLaneSHA256: rawDomainDigest(projectLaneDomain, node.ProjectID),
		SameProjectPolicy: "exclusive_until_terminal",
	}
}

func emptyWorkspace() WorkspacePolicy {
	return WorkspacePolicy{
		Mode: "none", RootIdentity: nil, IsolationID: nil, AllowedReadPaths: []string{},
	}
}

func providerPolicy(options ExecutionOptions) ProviderPolicy {
	return ProviderPolicy{
		Kind: "openai_responses", Endpoint: options.Endpoint,
		Model: options.Model, Store: false, Stream: true,
	}
}

func executionBudgets(options ExecutionOptions) ExecutionBudgets {
	return ExecutionBudgets{
		MaxTurns: 1, MaxToolCalls: 0, MaxOutputTokens: options.MaxOutputTokens,
		MaxModelOutputBytes: options.MaxModelOutputBytes,
		MaxModelEvents:      options.MaxModelEvents, TimeoutMilliseconds: options.TimeoutMilliseconds,
		MaxCostUSDMicros:      options.MaxCostUSDMicros,
		PricingSnapshotSHA256: options.PricingSnapshotSHA256,
	}
}

func approvalPolicy() ApprovalPolicy {
	return ApprovalPolicy{
		ProviderDispatch: "fresh_off_machine_consent", Workspace: "forbidden",
		Tools: "forbidden", Writeback: "forbidden",
	}
}

func resultPolicy(maximum uint64) ResultPolicy {
	return ResultPolicy{
		ArtifactKind: "local_graph_node_artifact", MaxResultBytes: maximum,
		PredecessorDataflow: "none", ConversationWriteback: "none",
		PromptWriteback: "none", MemoryWriteback: "none",
	}
}

func failurePolicy() FailurePolicy {
	return FailurePolicy{
		AutomaticRetry: false, LeaseRetry: false,
		PostClaimUncertainty: "dispatch_unknown", FailurePropagationOwner: "forge_core",
	}
}
