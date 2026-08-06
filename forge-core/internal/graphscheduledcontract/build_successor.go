package graphscheduledcontract

import (
	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/scheduledterminal"
)

// successorContractScope marks a candidate that follows a verified contiguous
// prefix of predecessor terminal receipts.
const successorContractScope = "schedule_successor_only"

// BuildSuccessor freezes the contract candidate for the next serial node after
// a verified contiguous prefix of predecessor terminal receipts. It grants no
// lifecycle, execution, dispatch, lane, or successor authority.
//
// receipts must be ordered by execution ordinal: receipt i is the terminal
// evidence for schedule.Nodes[i]. The successor is schedule.Nodes[N] where N
// is the number of receipts, and every direct predecessor of that node must
// be covered by the consumed prefix.
func BuildSuccessor(
	snapshot graphdispatch.ControlSnapshot,
	scheduleSHA256 string,
	options graphdispatch.ExecutionOptions,
	receipts []scheduledterminal.Receipt,
	predecessorContent string,
	targetNodeID string,
) (ScheduledNodeContractCandidate, error) {
	base, err := graphdispatch.Build(snapshot, options)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	schedule, err := graphschedule.Build(snapshot)
	if err != nil || schedule.ScheduleSHA256 != scheduleSHA256 {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	predecessors, err := verifyReceipts(snapshot, schedule, receipts)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	scheduled, err := selectReadyNode(schedule, predecessors, targetNodeID)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	value, err := successorCandidate(
		snapshot, schedule, scheduled, base, predecessors, predecessorContent,
	)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	return value, nil
}

// selectReadyNode picks a topologically-ready successor node. With an empty
// target it is the first node in the schedule's serial (wave-then-authored)
// order whose direct predecessors are all consumed and which itself is not
// yet consumed; with a non-empty target it requires that exact node to be
// ready. This is the wave-parallel rule: unrelated earlier ordinals
// (same-wave siblings) do not block a node whose own predecessor set is
// satisfied.
func selectReadyNode(
	schedule graphschedule.ExecutionSchedule,
	predecessors []PredecessorTerminalReceipt,
	targetNodeID string,
) (graphschedule.ScheduledNode, error) {
	consumed := make(map[string]bool, len(predecessors))
	for _, receipt := range predecessors {
		consumed[receipt.PredecessorNodeID] = true
	}
	for _, node := range schedule.Nodes {
		if node.ExecutionOrdinal == 0 {
			// The initial node belongs to the initial-candidate flow; a
			// successor selection never re-derives it.
			continue
		}
		if targetNodeID != "" && node.NodeID != targetNodeID {
			continue
		}
		if consumed[node.NodeID] {
			continue
		}
		ready := true
		for _, predecessorID := range node.DirectPredecessorNodeIDs {
			if !consumed[predecessorID] {
				ready = false
				break
			}
		}
		if ready {
			return node, nil
		}
	}
	return graphschedule.ScheduledNode{}, errInvalidCandidate
}

// ReadySuccessorNodes lists every topologically-ready successor node for a
// consumed receipt set, in serial order — the wave-parallel planning view.
func ReadySuccessorNodes(
	snapshot graphdispatch.ControlSnapshot,
	scheduleSHA256 string,
	receipts []scheduledterminal.Receipt,
) ([]string, error) {
	schedule, err := graphschedule.Build(snapshot)
	if err != nil || schedule.ScheduleSHA256 != scheduleSHA256 {
		return nil, errInvalidCandidate
	}
	if _, err := verifyReceipts(snapshot, schedule, receipts); err != nil {
		return nil, errInvalidCandidate
	}
	consumed := make(map[string]bool, len(receipts))
	for _, receipt := range receipts {
		consumed[receipt.NodeID] = true
	}
	ready := make([]string, 0, len(schedule.Nodes))
	for _, node := range schedule.Nodes {
		if node.ExecutionOrdinal == 0 || consumed[node.NodeID] {
			continue
		}
		all := true
		for _, predecessorID := range node.DirectPredecessorNodeIDs {
			if !consumed[predecessorID] {
				all = false
				break
			}
		}
		if all {
			ready = append(ready, node.NodeID)
		}
	}
	if len(ready) == 0 {
		return nil, errInvalidCandidate
	}
	return ready, nil
}

// successorCandidate rebuilds the node profile, the request, and the signed
// candidate for the selected successor node.
func successorCandidate(
	snapshot graphdispatch.ControlSnapshot,
	schedule graphschedule.ExecutionSchedule,
	scheduled graphschedule.ScheduledNode,
	base graphdispatch.NodeExecutionContract,
	predecessors []PredecessorTerminalReceipt,
	predecessorContent string,
) (ScheduledNodeContractCandidate, error) {
	source, err := manifestNode(snapshot, scheduled.NodeID)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	successorBase := base
	successorBase.Node = successorContractNode(scheduled, source, base.Node.SameProjectPolicy)
	request, err := buildSuccessorRequest(
		snapshot.GraphRunID, schedule, scheduled, source, base.Request, predecessors,
		predecessorContent,
	)
	if err != nil {
		return ScheduledNodeContractCandidate{}, errInvalidCandidate
	}
	value := candidateFrom(snapshot, schedule, scheduled, successorBase, request)
	value.ContractScope = successorContractScope
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

// verifyPredecessorPrefix requires the receipts to match the exact serial
// prefix of the schedule: receipt i must bind schedule.Nodes[i] by node,
// attempt, Graph Run, and Project lane, and no receipt may be duplicated. It
// returns the candidate's predecessor-evidence shape for each receipt. The
// scheduled sidecar does not consume the Graph Run journal, so the reserved
// event-seq fields stay zero/empty rather than inventing journal evidence.
func verifyReceipts(
	snapshot graphdispatch.ControlSnapshot,
	schedule graphschedule.ExecutionSchedule,
	receipts []scheduledterminal.Receipt,
) ([]PredecessorTerminalReceipt, error) {
	if len(receipts) == 0 || len(receipts) >= len(schedule.Nodes) {
		return nil, errInvalidCandidate
	}
	predecessors := make([]PredecessorTerminalReceipt, 0, len(receipts))
	seen := make(map[string]bool, len(receipts))
	for _, receipt := range receipts {
		node, ok := scheduledNodeFor(schedule, receipt.NodeID)
		if !ok || receipt.GraphRunID != snapshot.GraphRunID ||
			receipt.Attempt != node.Attempt || receipt.ProjectLaneSHA256 != node.ProjectLaneSHA256 {
			return nil, errInvalidCandidate
		}
		if seen[receipt.NodeID] {
			return nil, errInvalidCandidate // at most one receipt per node
		}
		seen[receipt.NodeID] = true
		predecessors = append(predecessors, PredecessorTerminalReceipt{
			PredecessorNodeID:     receipt.NodeID,
			PredecessorAttempt:    receipt.Attempt,
			TerminalEventSeq:      0,
			TerminalEventSHA256:   "",
			TerminalReceiptID:     receipt.ReceiptID,
			TerminalReceiptSHA256: receipt.ReceiptSHA256,
			NodeOutcome:           receipt.NodeOutcome,
			ProviderRequestID:     receipt.ProviderRequestID,
			DispatchID:            receipt.DispatchID,
		})
	}
	return predecessors, nil
}

func scheduledNodeFor(
	schedule graphschedule.ExecutionSchedule,
	nodeID string,
) (graphschedule.ScheduledNode, bool) {
	for _, node := range schedule.Nodes {
		if node.NodeID == nodeID {
			return node, true
		}
	}
	return graphschedule.ScheduledNode{}, false
}

// successorPredecessorsCovered checks that every direct predecessor of the
// successor node is present in the consumed receipt prefix. Topology order
// guarantees direct predecessors carry smaller ordinals, so prefix coverage
// is both necessary and sufficient.
func successorPredecessorsCovered(
	successor graphschedule.ScheduledNode,
	predecessors []PredecessorTerminalReceipt,
) bool {
	consumed := make(map[string]bool, len(predecessors))
	for _, receipt := range predecessors {
		consumed[receipt.PredecessorNodeID] = true
	}
	for _, predecessorID := range successor.DirectPredecessorNodeIDs {
		if !consumed[predecessorID] {
			return false
		}
	}
	return true
}

func buildSuccessorRequest(
	graphRunID string,
	schedule graphschedule.ExecutionSchedule,
	node graphschedule.ScheduledNode,
	source graphplan.Node,
	base graphdispatch.NodeRequest,
	predecessors []PredecessorTerminalReceipt,
	predecessorContent string,
) (ScheduledNodeRequest, error) {
	user, err := canonicalBytes(userPrompt{
		V: RequestVersion, NodeID: source.NodeID, Task: source.Task, Acceptance: source.Acceptance,
		PredecessorOutput: predecessorContent,
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
		RequiredPredecessorNodeIDs:  nonNilStrings(node.DirectPredecessorNodeIDs),
		PredecessorTerminalReceipts: nonNilReceipts(predecessors),
		PredecessorContentIncluded:  predecessorContent != "", Tools: []string{},
	}
	digest, err := domainDigest(requestDigestDomain, requestPayloadFrom(value))
	if err != nil {
		return ScheduledNodeRequest{}, errInvalidCandidate
	}
	value.RequestID, value.RequestSHA256 = requestIDPrefix+digest, digest
	return value, nil
}

// nonNilStrings returns a non-nil copy of the input so the canonical JSON
// always encodes an empty predecessor list as [] and never as null.
func nonNilStrings(values []string) []string {
	result := make([]string, 0, len(values))
	return append(result, values...)
}

// nonNilReceipts returns a non-nil copy of the predecessor evidence list.
func nonNilReceipts(values []PredecessorTerminalReceipt) []PredecessorTerminalReceipt {
	result := make([]PredecessorTerminalReceipt, 0, len(values))
	return append(result, values...)
}

// successorContractNode rebuilds the candidate node for the selected successor
// from the manifest profile and the schedule's serial/wave/lane identity.
// The base contract node always describes the ordinal-zero node, so it cannot
// be reused verbatim for a successor.
func successorContractNode(
	scheduled graphschedule.ScheduledNode,
	source graphplan.Node,
	sameProjectPolicy string,
) graphdispatch.ContractNode {
	return graphdispatch.ContractNode{
		NodeID:            scheduled.NodeID,
		AuthoredNodeIndex: scheduled.AuthoredNodeIndex,
		TopologyWaveIndex: scheduled.TopologyWaveIndex,
		Attempt:           scheduled.Attempt,
		ProjectID:         source.ProjectID,
		MemberRole:        source.MemberRole,
		AgentProfile:      source.AgentProfile,
		ProjectLaneSHA256: scheduled.ProjectLaneSHA256,
		SameProjectPolicy: sameProjectPolicy,
	}
}
