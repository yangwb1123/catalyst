package graphscheduledrelease

import (
	"fmt"
	"strings"
	"testing"

	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/graphscheduledcontract"
	"forgeos/forge-core/internal/graphscheduledreconcile"
	"forgeos/forge-core/internal/scheduledterminal"
)

type readyProgressPayloadTest struct {
	V                       uint16                                 `json:"v"`
	ProgressProtocolVersion uint16                                 `json:"progress_protocol_version"`
	GraphRunID              string                                 `json:"graph_run_id"`
	GraphID                 string                                 `json:"graph_id"`
	ScheduleID              string                                 `json:"schedule_id"`
	ScheduleSHA256          string                                 `json:"schedule_sha256"`
	NodeCount               uint16                                 `json:"node_count"`
	ExecutionMode           string                                 `json:"execution_mode"`
	MaxInFlightNodes        uint16                                 `json:"max_in_flight_nodes"`
	ProgressionPolicy       string                                 `json:"progression_policy"`
	AttemptPolicy           string                                 `json:"attempt_policy"`
	FailurePolicy           string                                 `json:"failure_policy"`
	Nodes                   []graphscheduledreconcile.ProgressNode `json:"nodes"`
}

func readyProgressTest(
	t *testing.T,
	schedule graphschedule.ExecutionSchedule,
	contract graphscheduledcontract.ScheduledNodeContractCandidate,
	request ScheduledNodeProviderRequestRecord,
	completed []scheduledterminal.Receipt,
) (graphscheduledreconcile.ProgressSnapshot, graphscheduledreconcile.Decision) {
	t.Helper()
	nodes := readyProgressNodesTest(schedule)
	for ordinal, receipt := range completed {
		completeReadyProgressNodeTest(&nodes[ordinal], receipt, ordinal)
	}
	selected := &nodes[contract.Node.ExecutionOrdinal]
	selected.CandidateID = readyStringPointerTest(contract.ContractID)
	selected.CandidateSHA256 = readyStringPointerTest(contract.ContractSHA256)
	selected.ProviderRequestID = readyStringPointerTest(request.ProviderRequestID)
	selected.PreparedRequestSHA256 = readyStringPointerTest(request.PreparedRequestSHA256)
	value := graphscheduledreconcile.ProgressSnapshot{
		V: 1, ProgressProtocolVersion: 1, GraphRunID: schedule.GraphRunID,
		GraphID: schedule.GraphID, ScheduleID: schedule.ScheduleID,
		ScheduleSHA256: schedule.ScheduleSHA256, NodeCount: schedule.NodeCount,
		ExecutionMode: schedule.ExecutionMode, MaxInFlightNodes: schedule.MaxInFlightNodes,
		ProgressionPolicy: schedule.ProgressionPolicy, AttemptPolicy: schedule.AttemptPolicy,
		FailurePolicy: schedule.FailurePolicy, Nodes: nodes,
	}
	value = resignReadyProgressTest(t, value)
	decision, err := graphscheduledreconcile.Reconcile(value)
	if err != nil {
		t.Fatalf("Reconcile ready fixture: %v", err)
	}
	return value, decision
}

func readyProgressNodesTest(
	schedule graphschedule.ExecutionSchedule,
) []graphscheduledreconcile.ProgressNode {
	nodes := make([]graphscheduledreconcile.ProgressNode, len(schedule.Nodes))
	for index, node := range schedule.Nodes {
		nodes[index] = graphscheduledreconcile.ProgressNode{
			ExecutionOrdinal: node.ExecutionOrdinal, NodeID: node.NodeID, Attempt: node.Attempt,
		}
	}
	return nodes
}

func completeReadyProgressNodeTest(
	node *graphscheduledreconcile.ProgressNode,
	receipt scheduledterminal.Receipt,
	ordinal int,
) {
	candidateSHA := readyIndexedDigestTest(1_000 + ordinal)
	node.CandidateID = readyStringPointerTest("scheduled-node-contract-" + candidateSHA)
	node.CandidateSHA256 = readyStringPointerTest(candidateSHA)
	node.ProviderRequestID = readyStringPointerTest(receipt.ProviderRequestID)
	node.PreparedRequestSHA256 = readyStringPointerTest(readyRequestDigestTest(ordinal))
	node.LifecycleStatus = readyStringPointerTest("terminalized")
	node.TerminalOutcome = readyStringPointerTest("completed")
	node.TerminalReceiptSHA256 = readyStringPointerTest(receipt.ReceiptSHA256)
}

func resignReadyProgressTest(
	t *testing.T,
	value graphscheduledreconcile.ProgressSnapshot,
) graphscheduledreconcile.ProgressSnapshot {
	t.Helper()
	payload := readyProgressPayloadTest{
		V: value.V, ProgressProtocolVersion: value.ProgressProtocolVersion,
		GraphRunID: value.GraphRunID, GraphID: value.GraphID,
		ScheduleID: value.ScheduleID, ScheduleSHA256: value.ScheduleSHA256,
		NodeCount: value.NodeCount, ExecutionMode: value.ExecutionMode,
		MaxInFlightNodes: value.MaxInFlightNodes, ProgressionPolicy: value.ProgressionPolicy,
		AttemptPolicy: value.AttemptPolicy, FailurePolicy: value.FailurePolicy, Nodes: value.Nodes,
	}
	value.SnapshotSHA256 = mustDomainDigestTest(
		t, "forge.scheduled-graph-progress-snapshot.v1\x00", payload,
	)
	return value
}

func readyStringPointerTest(value string) *string { return &value }

func readyTestDigest(character byte) string { return strings.Repeat(string(character), 64) }

func readyRequestDigestTest(ordinal int) string {
	return readyIndexedDigestTest(2_000 + ordinal)
}

func readyIndexedDigestTest(value int) string { return fmt.Sprintf("%064x", value) }
