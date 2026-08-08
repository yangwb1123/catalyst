package graphscheduledcontract

import (
	"bytes"
	"strings"
	"testing"

	"forgeos/forge-core/internal/scheduledterminal"
)

func TestCandidateRejectsForgedPredecessorReceiptMetadata(t *testing.T) {
	cases := []struct {
		name   string
		mutate func(*PredecessorTerminalReceipt)
	}{
		{"attempt", func(r *PredecessorTerminalReceipt) { r.PredecessorAttempt = 2 }},
		{"event sequence", func(r *PredecessorTerminalReceipt) { r.TerminalEventSeq = 1 }},
		{"event digest", func(r *PredecessorTerminalReceipt) { r.TerminalEventSHA256 = strings.Repeat("0", 64) }},
		{"receipt ID", func(r *PredecessorTerminalReceipt) {
			r.TerminalReceiptID = terminalReceiptIDPrefix + strings.Repeat("0", 64)
		}},
		{"receipt digest", func(r *PredecessorTerminalReceipt) { r.TerminalReceiptSHA256 = "not-a-digest" }},
		{"provider request", func(r *PredecessorTerminalReceipt) { r.ProviderRequestID = "" }},
		{"dispatch", func(r *PredecessorTerminalReceipt) { r.DispatchID = "" }},
		{"failed outcome", func(r *PredecessorTerminalReceipt) { r.NodeOutcome = "failed" }},
		{"uncertain outcome", func(r *PredecessorTerminalReceipt) { r.NodeOutcome = "failed_uncertain" }},
		{"unknown outcome", func(r *PredecessorTerminalReceipt) { r.NodeOutcome = "unknown" }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			candidate, err := buildContentSuccessor(t, "authenticated output")
			if err != nil {
				t.Fatalf("BuildSuccessor: %v", err)
			}
			test.mutate(&candidate.Request.PredecessorTerminalReceipts[0])
			assertForgedCandidateRejected(t, candidate)
		})
	}
}

func TestBuildSuccessorCanonicalizesDirectPredecessorOrder(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	target := schedule.Nodes[2]
	receipts := []scheduledterminal.Receipt{
		successorReceiptForNode(t, 1),
		successorReceiptForNode(t, 0),
	}
	candidate, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, readSourceFixture(t).Input.ExecutionOptions.options(),
		receipts, "", target.NodeID,
	)
	if err != nil {
		t.Fatalf("BuildSuccessor with reverse-order input receipts: %v", err)
	}
	for index, predecessorID := range target.DirectPredecessorNodeIDs {
		if candidate.Request.RequiredPredecessorNodeIDs[index] != predecessorID ||
			candidate.Request.PredecessorTerminalReceipts[index].PredecessorNodeID != predecessorID {
			t.Fatalf("candidate predecessor %d is not canonical: %#v", index, candidate.Request)
		}
	}
}

func TestCandidateRejectsShuffledPredecessorReceipts(t *testing.T) {
	candidate, err := buildContentSuccessor(t, "first direct predecessor output")
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	receipts := candidate.Request.PredecessorTerminalReceipts
	receipts[0], receipts[1] = receipts[1], receipts[0]
	assertForgedCandidateRejected(t, candidate)
}

func TestCandidateRejectsSuccessorOrdinalOutsideBound(t *testing.T) {
	candidate, err := BuildSuccessor(
		fixtureSnapshot(t), mustSchedule(t).ScheduleSHA256,
		readSourceFixture(t).Input.ExecutionOptions.options(), nil, "", mustSchedule(t).Nodes[1].NodeID,
	)
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	candidate.Node.ExecutionOrdinal = 32
	candidate.Request.ExecutionOrdinal = 32
	assertForgedCandidateRejected(t, candidate)
}

func assertForgedCandidateRejected(t *testing.T, candidate ScheduledNodeContractCandidate) {
	t.Helper()
	resignCandidate(t, &candidate)
	encoded, err := canonicalBytes(candidate)
	if err != nil {
		t.Fatalf("canonical forged candidate: %v", err)
	}
	if validateCandidate(candidate) == nil {
		t.Fatal("intrinsic validation accepted a forged predecessor receipt")
	}
	if _, err := DecodeCandidate(bytes.NewReader(encoded)); err == nil {
		t.Fatal("DecodeCandidate accepted a canonical resigned predecessor forgery")
	}
}
