package graphscheduledreconcile

import (
	"strings"
	"testing"
)

func stringPointer(value string) *string {
	return &value
}

func validUnsignedSnapshot() ProgressSnapshot {
	scheduleDigest := strings.Repeat("a", 64)
	return ProgressSnapshot{
		V: 1, ProgressProtocolVersion: 1, GraphRunID: "graph-run-test", GraphID: "graph-test",
		ScheduleID: scheduleIDPrefix + scheduleDigest, ScheduleSHA256: scheduleDigest,
		NodeCount: 2, ExecutionMode: "serial", MaxInFlightNodes: 1,
		ProgressionPolicy: "completed_contiguous_prefix", AttemptPolicy: "exactly_one",
		FailurePolicy: "fail_fast_no_retry",
		Nodes: []ProgressNode{
			{ExecutionOrdinal: 0, NodeID: "build", Attempt: 1},
			{ExecutionOrdinal: 1, NodeID: "verify", Attempt: 1},
		},
	}
}

func signedSnapshotBytes(t *testing.T, value ProgressSnapshot) []byte {
	t.Helper()
	value.SnapshotSHA256 = ""
	digest, err := domainDigest(snapshotDigestDomain, payloadFromSnapshot(value))
	if err != nil {
		t.Fatalf("snapshot digest: %v", err)
	}
	value.SnapshotSHA256 = digest
	encoded, err := canonicalBytes(value)
	if err != nil {
		t.Fatalf("canonical snapshot: %v", err)
	}
	return encoded
}

func decodeSignedSnapshot(t *testing.T, value ProgressSnapshot) ProgressSnapshot {
	t.Helper()
	snapshot, err := DecodeSnapshot(strings.NewReader(string(signedSnapshotBytes(t, value))))
	if err != nil {
		t.Fatalf("DecodeSnapshot: %v", err)
	}
	return snapshot
}

func materialize(node *ProgressNode, candidateByte, requestByte string) {
	candidateDigest := strings.Repeat(candidateByte, 64)
	requestDigest := strings.Repeat(requestByte, 64)
	node.CandidateSHA256 = stringPointer(candidateDigest)
	node.CandidateID = stringPointer(candidateIDPrefix + candidateDigest)
	node.PreparedRequestSHA256 = stringPointer(requestDigest)
	node.ProviderRequestID = stringPointer(providerIDPrefix + requestDigest)
}

func setLifecycle(node *ProgressNode, status, outcome string) {
	materialize(node, "b", "c")
	node.LifecycleStatus = stringPointer(status)
	if status == "terminalized" {
		node.TerminalOutcome = stringPointer(outcome)
		node.TerminalReceiptSHA256 = stringPointer(strings.Repeat("d", 64))
	}
}
