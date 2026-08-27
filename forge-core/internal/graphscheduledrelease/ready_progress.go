package graphscheduledrelease

import (
	"bytes"

	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledreconcile"
)

func validateReadyProgress(
	value ReadyReleaseControl,
) (graphscheduledreconcile.ProgressNode, error) {
	snapshot := value.ProgressSnapshot
	expected, err := graphscheduledreconcile.Reconcile(snapshot)
	if err != nil || !exactReadyDecision(expected, value.ReconcileDecision) ||
		!readyProgressSourceMatches(value) {
		return graphscheduledreconcile.ProgressNode{}, errInvalidControl
	}
	ordinal := *expected.NextExecutionOrdinal
	if int(ordinal) >= len(snapshot.Nodes) {
		return graphscheduledreconcile.ProgressNode{}, errInvalidControl
	}
	selected := snapshot.Nodes[ordinal]
	if !readySelectedNodeMatches(value, selected, ordinal, *expected.NextNodeID) {
		return graphscheduledreconcile.ProgressNode{}, errInvalidControl
	}
	return selected, nil
}

func exactReadyDecision(
	expected, supplied graphscheduledreconcile.Decision,
) bool {
	if expected.Disposition != "ready" || expected.NextExecutionOrdinal == nil ||
		expected.NextNodeID == nil {
		return false
	}
	expectedJSON, err := graphscheduledreconcile.MarshalDecision(expected)
	if err != nil {
		return false
	}
	suppliedJSON, err := graphscheduledreconcile.MarshalDecision(supplied)
	return err == nil && bytes.Equal(expectedJSON, suppliedJSON)
}

func readyProgressSourceMatches(value ReadyReleaseControl) bool {
	progress, schedule := value.ProgressSnapshot, value.Schedule
	if progress.GraphRunID != value.GraphRun.GraphRunID || progress.GraphID != value.GraphRun.GraphID ||
		progress.ScheduleID != schedule.ScheduleID || progress.ScheduleSHA256 != schedule.ScheduleSHA256 ||
		progress.NodeCount != schedule.NodeCount || len(progress.Nodes) != len(schedule.Nodes) ||
		!readyProgressPoliciesMatch(progress, schedule) {
		return false
	}
	for index := range progress.Nodes {
		left, right := progress.Nodes[index], schedule.Nodes[index]
		if left.ExecutionOrdinal != right.ExecutionOrdinal || left.NodeID != right.NodeID ||
			left.Attempt != right.Attempt {
			return false
		}
	}
	return true
}

func readyProgressPoliciesMatch(
	progress graphscheduledreconcile.ProgressSnapshot,
	schedule graphschedule.ExecutionSchedule,
) bool {
	return progress.ExecutionMode == schedule.ExecutionMode &&
		progress.MaxInFlightNodes == schedule.MaxInFlightNodes &&
		progress.ProgressionPolicy == schedule.ProgressionPolicy &&
		progress.AttemptPolicy == schedule.AttemptPolicy &&
		progress.FailurePolicy == schedule.FailurePolicy
}

func readySelectedNodeMatches(
	value ReadyReleaseControl,
	node graphscheduledreconcile.ProgressNode,
	ordinal uint16,
	nodeID string,
) bool {
	contract, request := value.ScheduledContract, value.ProviderRequest
	return node.ExecutionOrdinal == ordinal && node.NodeID == nodeID &&
		contract.Node.ExecutionOrdinal == ordinal && contract.Node.NodeID == nodeID &&
		node.CandidateID != nil && *node.CandidateID == contract.ContractID &&
		node.CandidateSHA256 != nil && *node.CandidateSHA256 == contract.ContractSHA256 &&
		node.ProviderRequestID != nil && *node.ProviderRequestID == request.ProviderRequestID &&
		node.PreparedRequestSHA256 != nil && *node.PreparedRequestSHA256 == request.PreparedRequestSHA256 &&
		node.LifecycleStatus == nil && node.TerminalOutcome == nil && node.TerminalReceiptSHA256 == nil
}
