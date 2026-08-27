package graphscheduledrelease

import (
	"fmt"
	"testing"

	"forgeos/forge-core/internal/graphschedule"
	"forgeos/forge-core/internal/scheduledterminal"
)

const readyTerminalOutputDomainTest = "forge.group-agent-scheduled-node-terminal-output.v1\x00"

func readyCompletedReceiptsTest(
	t *testing.T,
	schedule graphschedule.ExecutionSchedule,
	count uint16,
) []scheduledterminal.Receipt {
	t.Helper()
	receipts := make([]scheduledterminal.Receipt, count)
	for ordinal := range count {
		receipts[ordinal] = readyReceiptTest(t, schedule, ordinal)
	}
	return receipts
}

func readyReceiptTest(
	t *testing.T,
	schedule graphschedule.ExecutionSchedule,
	ordinal uint16,
) scheduledterminal.Receipt {
	t.Helper()
	node := schedule.Nodes[ordinal]
	artifactSHA := readyIndexedDigestTest(3_000 + int(ordinal))
	value := scheduledterminal.Receipt{
		V: 1, SchedulerProtocolVersion: 1, TerminalReceiptProtocol: 1,
		TerminalControlSHA256: readyTestDigest('1'), GraphRunID: schedule.GraphRunID,
		GraphID: schedule.GraphID, NodeID: node.NodeID, Attempt: node.Attempt,
		DispatchID:        fmt.Sprintf("dispatch-%s", node.NodeID),
		ProviderRequestID: "scheduled-node-provider-request-" + readyRequestDigestTest(int(ordinal)),
		ProjectLaneSHA256: node.ProjectLaneSHA256, ArtifactKind: "result",
		ArtifactID:     "scheduled-node-terminal-artifact-" + artifactSHA,
		ArtifactSHA256: artifactSHA, NodeOutcome: "completed", LaneReleaseAuthorized: true,
	}
	return resignReadyReceiptTest(t, value)
}

func bindReadyContentArtifactTest(
	t *testing.T,
	receipt scheduledterminal.Receipt,
	content string,
) (scheduledterminal.Receipt, scheduledterminal.Artifact) {
	t.Helper()
	value := scheduledterminal.Artifact{
		V: 1, TerminalArtifactProtocol: 1, ArtifactKind: "result",
		GraphRunID: receipt.GraphRunID, NodeID: receipt.NodeID, Attempt: receipt.Attempt,
		DispatchID: receipt.DispatchID, ProviderRequestID: receipt.ProviderRequestID,
		ClaimEventSHA256: readyTestDigest('1'), AuthorizationSHA256: readyTestDigest('2'),
		ProviderRequestSHA256: readyTestDigest('3'), RequestBodySHA256: readyTestDigest('4'),
		PricingSnapshotSHA256: readyTestDigest('5'), LaneOwnershipID: "lane-owner-" + receipt.NodeID,
		ProjectLaneSHA256: receipt.ProjectLaneSHA256, ProviderPollStarted: true,
		TerminalSeen: true, StreamEOFSeen: true, Classification: "completed",
		OutputText: content, OutputBytes: len([]byte(content)),
		OutputSHA256:  rawDomainDigest(readyTerminalOutputDomainTest, []byte(content)),
		UsageObserved: true, InputTokens: 1, OutputTokens: 1, CreatedAtMS: 77,
	}
	encoded, err := scheduledterminal.MarshalArtifact(value)
	if err != nil {
		t.Fatalf("MarshalArtifact: %v", err)
	}
	artifact, err := scheduledterminal.DecodeArtifact(encoded)
	if err != nil {
		t.Fatalf("DecodeArtifact: %v", err)
	}
	receipt.ArtifactID, receipt.ArtifactSHA256 = artifact.ArtifactID, artifact.ArtifactSHA256
	return resignReadyReceiptTest(t, receipt), artifact
}

func resignReadyReceiptTest(
	t *testing.T,
	value scheduledterminal.Receipt,
) scheduledterminal.Receipt {
	t.Helper()
	encoded, err := scheduledterminal.MarshalReceipt(value)
	if err != nil {
		t.Fatalf("MarshalReceipt: %v", err)
	}
	decoded, err := scheduledterminal.DecodeReceipt(encoded)
	if err != nil {
		t.Fatalf("DecodeReceipt: %v", err)
	}
	return decoded
}

func readyDirectReceiptsTest(
	schedule graphschedule.ExecutionSchedule,
	target uint16,
	completed []scheduledterminal.Receipt,
) []scheduledterminal.Receipt {
	byNode := make(map[string]scheduledterminal.Receipt, len(completed))
	for _, receipt := range completed {
		byNode[receipt.NodeID] = receipt
	}
	direct := make([]scheduledterminal.Receipt, 0, len(schedule.Nodes[target].DirectPredecessorNodeIDs))
	for _, nodeID := range schedule.Nodes[target].DirectPredecessorNodeIDs {
		direct = append(direct, byNode[nodeID])
	}
	return direct
}
