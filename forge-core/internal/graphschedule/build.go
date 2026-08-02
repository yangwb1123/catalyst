package graphschedule

import (
	"forgeos/forge-core/internal/graphdispatch"
	"forgeos/forge-core/internal/graphplan"
)

// Build validates an exact v1 control and emits a passive multi-node schedule.
func Build(snapshot graphdispatch.ControlSnapshot) (ExecutionSchedule, error) {
	if len(snapshot.Plan.AuthoredNodeIDs) < 2 ||
		graphdispatch.ValidateControlSnapshot(snapshot) != nil {
		return ExecutionSchedule{}, errInvalidControl
	}
	nodes, err := buildNodes(snapshot)
	if err != nil {
		return ExecutionSchedule{}, errInvalidControl
	}
	value, err := scheduleFrom(snapshot, nodes)
	if err != nil {
		return ExecutionSchedule{}, errInvalidControl
	}
	digest, err := scheduleDigest(value)
	if err != nil {
		return ExecutionSchedule{}, errInvalidControl
	}
	value.ScheduleID = "graph-execution-schedule-" + digest
	value.ScheduleSHA256 = digest
	if validateSchedule(value) != nil {
		return ExecutionSchedule{}, errInvalidControl
	}
	return value, nil
}

func buildNodes(snapshot graphdispatch.ControlSnapshot) ([]ScheduledNode, error) {
	positions := authoredPositions(snapshot.Plan.AuthoredNodeIDs)
	projects := manifestProjects(snapshot)
	nodes := make([]ScheduledNode, 0, len(snapshot.Plan.AuthoredNodeIDs))
	for waveIndex, wave := range snapshot.Plan.Waves {
		for _, nodeID := range wave {
			node, err := scheduledNode(snapshot.Plan, positions, projects, nodeID, waveIndex, len(nodes))
			if err != nil {
				return nil, err
			}
			nodes = append(nodes, node)
		}
	}
	return nodes, nil
}

func scheduledNode(
	plan graphplan.Plan,
	positions map[string]int,
	projects map[string]string,
	nodeID string,
	waveIndex int,
	ordinal int,
) (ScheduledNode, error) {
	authoredIndex, exists := positions[nodeID]
	projectID, projectExists := projects[nodeID]
	if !exists || !projectExists {
		return ScheduledNode{}, errInvalidControl
	}
	indices := []int{authoredIndex, waveIndex, ordinal}
	for _, index := range indices {
		if index < 0 || index > int(^uint16(0)) {
			return ScheduledNode{}, errInvalidControl
		}
	}
	return ScheduledNode{
		ExecutionOrdinal: uint16(ordinal), NodeID: nodeID,
		AuthoredNodeIndex: uint16(authoredIndex), TopologyWaveIndex: uint16(waveIndex),
		ProjectLaneSHA256: rawDomainDigest(projectLaneDomain, []byte(projectID)), Attempt: 1,
		DirectPredecessorNodeIDs: directPredecessors(plan, nodeID),
	}, nil
}

func authoredPositions(identifiers []string) map[string]int {
	positions := make(map[string]int, len(identifiers))
	for index, identifier := range identifiers {
		positions[identifier] = index
	}
	return positions
}

func manifestProjects(snapshot graphdispatch.ControlSnapshot) map[string]string {
	projects := make(map[string]string, len(snapshot.Manifest.Nodes))
	for _, node := range snapshot.Manifest.Nodes {
		projects[node.NodeID] = node.ProjectID
	}
	return projects
}

func directPredecessors(plan graphplan.Plan, target string) []string {
	incoming := make(map[string]struct{})
	for _, edge := range plan.Edges {
		if edge.ToNodeID == target {
			incoming[edge.FromNodeID] = struct{}{}
		}
	}
	result := make([]string, 0, len(incoming))
	for _, identifier := range plan.AuthoredNodeIDs {
		if _, exists := incoming[identifier]; exists {
			result = append(result, identifier)
		}
	}
	return result
}

func scheduleFrom(
	snapshot graphdispatch.ControlSnapshot,
	nodes []ScheduledNode,
) (ExecutionSchedule, error) {
	nodeCount, waveCount := len(snapshot.Plan.AuthoredNodeIDs), len(snapshot.Plan.Waves)
	if nodeCount > int(^uint16(0)) || waveCount > int(^uint16(0)) || len(nodes) == 0 {
		return ExecutionSchedule{}, errInvalidControl
	}
	return ExecutionSchedule{
		V: ExecutionScheduleVersion, SchedulerProtocolVersion: snapshot.SchedulerProtocolVersion,
		ExecutionScheduleProtocolVersion: ExecutionScheduleProtocolVersion,
		ControlSnapshotSHA256:            snapshot.SnapshotSHA256,
		ExpectedLastEventSeq:             snapshot.LastEventSeq, ExpectedLastEventSHA256: snapshot.LastEventSHA256,
		GraphRunID: snapshot.GraphRunID, GraphID: snapshot.GraphID,
		SourceSnapshotSHA256: snapshot.SourceSnapshotSHA256,
		GraphManifestSHA256:  snapshot.GraphManifestSHA256, CorePlanSHA256: snapshot.CorePlanSHA256,
		NodeCount: uint16(nodeCount), WaveCount: uint16(waveCount), ExecutionMode: "serial",
		MaxInFlightNodes: 1, SelectionPolicy: "topology_wave_then_authored_order",
		ProgressionPolicy: "completed_contiguous_prefix",
		AttemptPolicy:     "exactly_one", FailurePolicy: "fail_fast_no_retry",
		OutcomePolicy: fixedOutcomePolicy(), PredecessorSemantics: "ordering_only",
		PredecessorDataflow: "none", PartialOutputDataflow: false,
		ReceiptHandling: "future_verified_identity_slots", Nodes: nodes,
		InitialFrontier: append([]string(nil), snapshot.Plan.Waves[0]...), InitialNode: nodes[0].NodeID,
		ExecutionContractPresent: false, DispatchAuthorityReleased: false,
		ProgressObserved: false, SuccessorAdvanced: false,
	}, nil
}

func fixedOutcomePolicy() OutcomePolicy {
	return OutcomePolicy{
		Completed: "advance_or_complete", Length: "fail_graph",
		Uncertainty: "fail_graph_uncertain", DispatchUnknown: "quarantine_no_advance",
	}
}

func scheduleDigest(value ExecutionSchedule) (string, error) {
	encoded, err := canonicalBytes(schedulePayloadFrom(value))
	if err != nil {
		return "", err
	}
	return rawDomainDigest(scheduleDigestDomain, encoded), nil
}
