package graphscheduledcontract

import (
	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/scheduledterminal"
)

// successorContractScope marks a candidate that follows verified predecessor
// terminal receipts under the topological-ready rule (ADR-0035).
const successorContractScope = "schedule_successor_only"

// BuildSuccessor freezes the contract candidate for a topologically-ready
// successor node: every direct predecessor of the selected node carries a
// consumed, verified receipt (any order; unrelated same-wave siblings do not
// block it). It grants no lifecycle, execution, dispatch, lane, or successor
// authority. With an empty targetNodeID the first ready node in serial order
// is selected; a non-empty target requires that exact node to be ready.
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

// filterDirectPredecessors keeps only the receipts of the candidate node's
// direct predecessors, in the schedule's canonical predecessor order. Input
// receipts may arrive in any order, but candidate content binds the first
// scheduled direct predecessor deterministically.
func filterDirectPredecessors(
	node graphschedule.ScheduledNode,
	predecessors []PredecessorTerminalReceipt,
) []PredecessorTerminalReceipt {
	byNode := make(map[string]PredecessorTerminalReceipt, len(predecessors))
	for _, receipt := range predecessors {
		byNode[receipt.PredecessorNodeID] = receipt
	}
	filtered := make([]PredecessorTerminalReceipt, 0, len(node.DirectPredecessorNodeIDs))
	for _, predecessorID := range node.DirectPredecessorNodeIDs {
		if receipt, ok := byNode[predecessorID]; ok {
			filtered = append(filtered, receipt)
		}
	}
	return filtered
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
	// A fully-consumed graph legitimately has an empty next wave; report it
	// as an empty plan (nil, nil), not as a planning fault.
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

// verifyReceipts requires every consumed receipt to bind one exact scheduled
// node by identity, attempt, Graph Run, and project lane, with no duplicates.
// Input order is immaterial. The scheduled sidecar does not consume the Graph
// Run journal, so reserved event fields remain zero/empty.
func verifyReceipts(
	snapshot graphdispatch.ControlSnapshot,
	schedule graphschedule.ExecutionSchedule,
	receipts []scheduledterminal.Receipt,
) ([]PredecessorTerminalReceipt, error) {
	if len(receipts) > len(schedule.Nodes) {
		return nil, errInvalidCandidate
	}
	predecessors := make([]PredecessorTerminalReceipt, 0, len(receipts))
	seen := make(map[string]bool, len(receipts))
	for _, receipt := range receipts {
		node, ok := scheduledNodeFor(schedule, receipt.NodeID)
		if !ok || !validConsumedTerminalReceipt(receipt) ||
			receipt.GraphRunID != snapshot.GraphRunID || receipt.GraphID != snapshot.GraphID ||
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

func validConsumedTerminalReceipt(receipt scheduledterminal.Receipt) bool {
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		return false
	}
	validated, err := scheduledterminal.DecodeReceipt(encoded)
	return err == nil && validated == receipt && receipt.NodeOutcome == "completed" &&
		receipt.ArtifactKind == "result" && !receipt.RetryAuthorized &&
		receipt.LaneReleaseAuthorized && !receipt.SuccessorAdvanceAuthorized
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

func buildSuccessorRequest(
	graphRunID string,
	schedule graphschedule.ExecutionSchedule,
	node graphschedule.ScheduledNode,
	source graphplan.Node,
	base graphdispatch.NodeRequest,
	predecessors []PredecessorTerminalReceipt,
	predecessorContent string,
) (ScheduledNodeRequest, error) {
	// ADR-0035: the candidate carries exactly its direct predecessors'
	// receipts in schedule order. Same-wave siblings are evidence of progress
	// but are not carried by this candidate's request (a wave-sibling with an
	// empty direct-predecessor set carries zero receipts). If content is
	// present, it is therefore bound to the first direct predecessor.
	predecessors = filterDirectPredecessors(node, predecessors)
	if predecessorContent != "" && len(predecessors) == 0 {
		// Disclosed output is authenticated through one of the selected
		// node's direct predecessor receipts. A same-wave sibling has no
		// such receipt, so it cannot carry predecessor plaintext.
		return ScheduledNodeRequest{}, errInvalidCandidate
	}
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
