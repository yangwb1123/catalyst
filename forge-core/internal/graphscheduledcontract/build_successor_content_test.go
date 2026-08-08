package graphscheduledcontract

import (
	"bytes"
	"strings"
	"testing"

	"forgeos/forge-core/internal/scheduledterminal"
)

func buildContentSuccessor(
	t *testing.T,
	content string,
) (ScheduledNodeContractCandidate, error) {
	t.Helper()
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	if len(schedule.Nodes) < 3 {
		t.Fatalf("content successor test needs >= 3 nodes, got %d", len(schedule.Nodes))
	}
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipts := []scheduledterminal.Receipt{
		successorReceiptForNode(t, 0),
		successorReceiptForNode(t, 1),
	}
	return BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options, receipts, content, schedule.Nodes[2].NodeID,
	)
}

func TestBuildSuccessorEmbedsPredecessorContent(t *testing.T) {
	content := "frontend produced: login flow verified, token refresh works"
	candidate, err := buildContentSuccessor(t, content)
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	if !candidate.Request.PredecessorContentIncluded {
		t.Fatal("predecessor content flag must be true when content is embedded")
	}
	if !strings.Contains(candidate.Request.UserPrompt, content) {
		t.Fatalf("user prompt must embed the predecessor output, got %q", candidate.Request.UserPrompt)
	}
	if !strings.Contains(candidate.Request.UserPrompt, "predecessor_output") {
		t.Fatal("user prompt must carry the predecessor_output field")
	}

	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	plain, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options,
		[]scheduledterminal.Receipt{successorReceipt(t)}, "", "",
	)
	if err != nil {
		t.Fatalf("BuildSuccessor plain: %v", err)
	}
	if plain.Request.PredecessorContentIncluded ||
		strings.Contains(plain.Request.UserPrompt, "predecessor_output") {
		t.Fatal("plain successor must omit predecessor content and its field")
	}
}

func TestBuildSuccessorRejectsContentWithoutDirectPredecessorReceipt(t *testing.T) {
	snapshot := fixtureSnapshot(t)
	schedule := mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	receipts := []scheduledterminal.Receipt{successorReceipt(t)}
	if _, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options,
		receipts, "unbound sibling output", "",
	); err == nil {
		t.Fatal("BuildSuccessor accepted content for a wave sibling with no direct predecessor receipt")
	}

	plain, err := BuildSuccessor(snapshot, schedule.ScheduleSHA256, options, receipts, "", "")
	if err != nil {
		t.Fatalf("BuildSuccessor plain wave sibling: %v", err)
	}
	prompt, err := decodeExact[userPrompt]([]byte(plain.Request.UserPrompt))
	if err != nil {
		t.Fatalf("decode plain user Prompt: %v", err)
	}
	prompt.PredecessorOutput = "unbound sibling output"
	encoded, err := canonicalBytes(prompt)
	if err != nil {
		t.Fatalf("encode content-bearing user Prompt: %v", err)
	}
	plain.Request.UserPrompt = string(encoded)
	plain.Request.UserPromptBytes = uint64(len(encoded))
	plain.Request.UserPromptSHA256 = byteDigest(string(encoded))
	plain.Request.PredecessorContentIncluded = true
	resignCandidate(t, &plain)
	if validateCandidate(plain) == nil {
		t.Fatal("intrinsic validation accepted content without an authenticating receipt")
	}
}

func TestCandidateRejectsReceiptUnrelatedToRequiredPredecessors(t *testing.T) {
	candidate, err := buildContentSuccessor(t, "authenticated output")
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	unrelated := candidate.Request.PredecessorTerminalReceipts[0]
	unrelated.PredecessorNodeID = "unrelated-node"
	candidate.Request.PredecessorTerminalReceipts = append(
		candidate.Request.PredecessorTerminalReceipts,
		unrelated,
	)
	resignCandidate(t, &candidate)
	if validateCandidate(candidate) == nil {
		t.Fatal("intrinsic validation accepted an unrelated extra receipt")
	}
}

func TestCandidateRejectsDuplicateRequiredIDHidingUnrelatedReceipt(t *testing.T) {
	candidate, err := buildContentSuccessor(t, "authenticated output")
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	direct := candidate.Request.RequiredPredecessorNodeIDs[0]
	candidate.Request.RequiredPredecessorNodeIDs = []string{direct, direct}
	unrelated := candidate.Request.PredecessorTerminalReceipts[1]
	unrelated.PredecessorNodeID = "unrelated-node"
	candidate.Request.PredecessorTerminalReceipts[1] = unrelated
	resignCandidate(t, &candidate)
	if validateCandidate(candidate) == nil {
		t.Fatal("intrinsic validation accepted duplicate required IDs hiding an unrelated receipt")
	}
}

func TestBuildSuccessorAcceptsPredecessorContentAtOneMiB(t *testing.T) {
	candidate, err := buildContentSuccessor(t, strings.Repeat("x", MaxPredecessorOutputBytes))
	if err != nil {
		t.Fatalf("BuildSuccessor at one-MiB content limit: %v", err)
	}
	if _, err := MarshalCandidate(candidate); err != nil {
		t.Fatalf("MarshalCandidate at one-MiB content limit: %v", err)
	}
}

func TestBuildSuccessorRejectsPredecessorContentAboveOneMiB(t *testing.T) {
	if _, err := buildContentSuccessor(
		t, strings.Repeat("x", MaxPredecessorOutputBytes+1),
	); err == nil {
		t.Fatal("BuildSuccessor accepted predecessor content above one MiB")
	}
}

func TestMaximumEscapeExpandedPromptAndCandidateRemainReachable(t *testing.T) {
	candidate, err := buildContentSuccessor(t, "seed")
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}

	candidate.Node.NodeID = strings.Repeat("\"", maxIdentifierBytes)
	candidate.Request.NodeID = candidate.Node.NodeID
	prompt, err := canonicalBytes(userPrompt{
		V: RequestVersion, NodeID: candidate.Node.NodeID,
		Task:              strings.Repeat("\"", maxProseBytes),
		Acceptance:        strings.Repeat("\"", maxProseBytes),
		PredecessorOutput: strings.Repeat("\"", MaxPredecessorOutputBytes),
	})
	if err != nil {
		t.Fatalf("canonical user Prompt: %v", err)
	}
	if len(prompt) != MaxUserPromptBytes {
		t.Fatalf("user Prompt bytes = %d, want %d", len(prompt), MaxUserPromptBytes)
	}
	candidate.Request.UserPrompt = string(prompt)
	candidate.Request.UserPromptBytes = uint64(len(prompt))
	candidate.Request.UserPromptSHA256 = byteDigest(string(prompt))
	candidate.Request.PredecessorContentIncluded = true
	resignCandidate(t, &candidate)

	encoded, err := MarshalCandidate(candidate)
	if err != nil {
		t.Fatalf("MarshalCandidate with maximally escaped bounded fields: %v", err)
	}
	if len(encoded) <= 4*1024*1024 || len(encoded) > MaxCandidateBytes {
		t.Fatalf("escape-expanded candidate bytes = %d, want (4 MiB, %d]", len(encoded), MaxCandidateBytes)
	}
	if _, err := DecodeCandidate(bytes.NewReader(encoded)); err != nil {
		t.Fatalf("DecodeCandidate with maximally escaped bounded fields: %v", err)
	}
}
