package graphscheduledcontract

import (
	"reflect"

	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphschedule"
)

// BuildInitial rebuilds Core's exact schedule and emits only its ordinal-zero
// passive candidate. The caller can supply neither a node nor receipt evidence.
func BuildInitial(
	snapshot graphdispatch.ControlSnapshot,
	scheduleSHA256 string,
	options graphdispatch.ExecutionOptions,
) (ScheduledNodeContractCandidate, error) {
	base, err := graphdispatch.Build(snapshot, options)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	schedule, err := graphschedule.Build(snapshot)
	if err != nil || schedule.ScheduleSHA256 != scheduleSHA256 {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	scheduled, err := initialScheduledNode(schedule, base.Node)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	source, err := manifestNode(snapshot, scheduled.NodeID)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	request, err := buildRequest(snapshot.GraphRunID, schedule, scheduled, source, base.Request)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	value := candidateFrom(snapshot, schedule, scheduled, base, request)
	digest, err := domainDigest(contractDigestDomain, candidatePayloadFrom(value))
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	value.ContractID, value.ContractSHA256 = contractIDPrefix+digest, digest
	if validateCandidate(value) != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	return value, nil
}

func initialScheduledNode(
	schedule graphschedule.ExecutionSchedule,
	base graphdispatch.ContractNode,
) (graphschedule.ScheduledNode, error) {
	if len(schedule.Nodes) == 0 || len(schedule.InitialFrontier) == 0 {
		return graphschedule.ScheduledNode{}, errInvalidCandidate
	}
	node := schedule.Nodes[0]
	valid := node.ExecutionOrdinal == 0 && node.TopologyWaveIndex == 0 && node.Attempt == 1 &&
		node.NodeID == schedule.InitialNode && node.NodeID == schedule.InitialFrontier[0] &&
		node.DirectPredecessorNodeIDs != nil && len(node.DirectPredecessorNodeIDs) == 0 &&
		node.NodeID == base.NodeID && node.AuthoredNodeIndex == base.AuthoredNodeIndex &&
		node.TopologyWaveIndex == base.TopologyWaveIndex && node.Attempt == base.Attempt &&
		node.ProjectLaneSHA256 == base.ProjectLaneSHA256
	if !valid {
		return graphschedule.ScheduledNode{}, errInvalidCandidate
	}
	return node, nil
}

func manifestNode(
	snapshot graphdispatch.ControlSnapshot,
	nodeID string,
) (graphplan.Node, error) {
	for _, node := range snapshot.Manifest.Nodes {
		if node.NodeID == nodeID {
			return node, nil
		}
	}
	return graphplan.Node{}, errInvalidCandidate
}

func buildRequest(
	graphRunID string,
	schedule graphschedule.ExecutionSchedule,
	node graphschedule.ScheduledNode,
	source graphplan.Node,
	base graphdispatch.NodeRequest,
) (ScheduledNodeRequest, error) {
	user, err := canonicalBytes(userPrompt{
		V: RequestVersion, NodeID: source.NodeID, Task: source.Task, Acceptance: source.Acceptance,
	})
	if err != nil {
		return ScheduledNodeRequest{}, errInvalidCandidate
	}
	value := ScheduledNodeRequest{
		V: RequestVersion, GraphRunID: graphRunID, ScheduleID: schedule.ScheduleID,
		ScheduleSHA256: schedule.ScheduleSHA256, ExecutionOrdinal: node.ExecutionOrdinal,
		NodeID: node.NodeID, Attempt: node.Attempt, SystemPrompt: base.SystemPrompt,
		SystemPromptBytes: uint64(len(base.SystemPrompt)), SystemPromptSHA256: byteDigest(base.SystemPrompt),
		UserPrompt: string(user), UserPromptBytes: uint64(len(user)), UserPromptSHA256: byteDigest(string(user)),
		RequiredPredecessorNodeIDs: []string{}, PredecessorTerminalReceipts: []PredecessorTerminalReceipt{},
		PredecessorContentIncluded: false, Tools: []string{},
	}
	digest, err := domainDigest(requestDigestDomain, requestPayloadFrom(value))
	if err != nil {
		return ScheduledNodeRequest{}, errInvalidCandidate
	}
	value.RequestID, value.RequestSHA256 = requestIDPrefix+digest, digest
	return value, nil
}

func candidateFrom(
	snapshot graphdispatch.ControlSnapshot,
	schedule graphschedule.ExecutionSchedule,
	scheduled graphschedule.ScheduledNode,
	base graphdispatch.NodeExecutionContract,
	request ScheduledNodeRequest,
) ScheduledNodeContractCandidate {
	return ScheduledNodeContractCandidate{
		V: CandidateVersion, SchedulerProtocolVersion: snapshot.SchedulerProtocolVersion,
		NodeExecutionProtocolVersion:     NodeExecutionProtocolVersion,
		ExecutionScheduleProtocolVersion: schedule.ExecutionScheduleProtocolVersion,
		ContractScope:                    contractScope, GraphRunID: snapshot.GraphRunID, GraphID: snapshot.GraphID,
		SourceSnapshotSHA256: snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256:  snapshot.GraphManifestSHA256, CorePlanSHA256: snapshot.CorePlanSHA256,
		ControlSnapshotSHA256: snapshot.SnapshotSHA256, ScheduleID: schedule.ScheduleID,
		ScheduleSHA256: schedule.ScheduleSHA256, ExpectedLastEventSeq: snapshot.LastEventSeq,
		ExpectedLastEventSHA256: snapshot.LastEventSHA256, Node: candidateNode(scheduled, base.Node),
		Request: request, Workspace: base.Workspace, Provider: base.Provider, Budgets: base.Budgets,
		Approval: base.Approval, Result: base.Result, Failure: base.Failure,
	}
}

func candidateNode(
	scheduled graphschedule.ScheduledNode,
	base graphdispatch.ContractNode,
) CandidateNode {
	return CandidateNode{
		ExecutionOrdinal: scheduled.ExecutionOrdinal, NodeID: base.NodeID,
		AuthoredNodeIndex: base.AuthoredNodeIndex, TopologyWaveIndex: base.TopologyWaveIndex,
		Attempt: base.Attempt, ProjectID: base.ProjectID, MemberRole: base.MemberRole,
		AgentProfile: base.AgentProfile, ProjectLaneSHA256: base.ProjectLaneSHA256,
		SameProjectPolicy: base.SameProjectPolicy,
	}
}

// ValidateCandidateSource reconstructs the candidate from its bound private
// control and embedded options. It is required in addition to DecodeCandidate.
func ValidateCandidateSource(
	value ScheduledNodeContractCandidate,
	snapshot graphdispatch.ControlSnapshot,
) error {
	if validateCandidate(value) != nil {
		return errInvalidCandidate
	}
	expected, err := BuildInitial(snapshot, value.ScheduleSHA256, optionsFrom(value))
	if err != nil || !reflect.DeepEqual(expected, value) {
		return errInvalidCandidate
	}
	return nil
}

func optionsFrom(value ScheduledNodeContractCandidate) graphdispatch.ExecutionOptions {
	return graphdispatch.ExecutionOptions{
		Endpoint: value.Provider.Endpoint, Model: value.Provider.Model,
		MaxOutputTokens:       value.Budgets.MaxOutputTokens,
		MaxModelOutputBytes:   value.Budgets.MaxModelOutputBytes,
		MaxModelEvents:        value.Budgets.MaxModelEvents,
		TimeoutMilliseconds:   value.Budgets.TimeoutMilliseconds,
		MaxCostUSDMicros:      value.Budgets.MaxCostUSDMicros,
		PricingSnapshotSHA256: value.Budgets.PricingSnapshotSHA256,
		MaxResultBytes:        value.Result.MaxResultBytes,
	}
}
