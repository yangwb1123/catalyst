package graphscheduledcontract

import (
	"crypto/sha256"
	"encoding/hex"
	"strings"
	"testing"

	"forgeos/forge-core/internal/scheduledterminal"
)

func TestValidateSelectedCandidateSourceAcceptsInitialAndSuccessors(t *testing.T) {
	initial := mustCandidate(t)
	if err := ValidateSelectedCandidateSource(
		initial, fixtureSnapshot(t), []scheduledterminal.Receipt{}, nil,
	); err != nil {
		t.Fatalf("validate initial: %v", err)
	}

	snapshot, schedule := fixtureSnapshot(t), mustSchedule(t)
	options := readSourceFixture(t).Input.ExecutionOptions.options()
	zeroClosure, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, options,
		[]scheduledterminal.Receipt{}, "", schedule.Nodes[1].NodeID,
	)
	if err != nil {
		t.Fatalf("build zero-closure successor: %v", err)
	}
	if err := ValidateSelectedCandidateSource(
		zeroClosure, snapshot, []scheduledterminal.Receipt{}, nil,
	); err != nil {
		t.Fatalf("validate zero-closure successor: %v", err)
	}

	candidate, receipts, artifact := selectedContentFixture(t, "verified predecessor output")
	if err := ValidateSelectedCandidateSource(candidate, snapshot, receipts, &artifact); err != nil {
		t.Fatalf("validate content successor: %v", err)
	}
}

func TestValidateSelectedCandidateSourceRequiresExplicitEmptyClosure(t *testing.T) {
	if err := ValidateSelectedCandidateSource(
		mustCandidate(t), fixtureSnapshot(t), nil, nil,
	); err == nil {
		t.Fatal("accepted a nil initial closure instead of explicit []")
	}
	snapshot, schedule := fixtureSnapshot(t), mustSchedule(t)
	candidate, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, readSourceFixture(t).Input.ExecutionOptions.options(),
		[]scheduledterminal.Receipt{}, "", schedule.Nodes[1].NodeID,
	)
	if err != nil {
		t.Fatalf("build zero-closure successor: %v", err)
	}
	if err := ValidateSelectedCandidateSource(candidate, snapshot, nil, nil); err == nil {
		t.Fatal("accepted a nil successor closure instead of explicit []")
	}
}

func TestValidateSelectedCandidateSourceRejectsShuffledFullClosure(t *testing.T) {
	candidate, receipts, _ := selectedContentFixture(t, "verified predecessor output")
	plain, err := BuildSuccessor(
		fixtureSnapshot(t), candidate.ScheduleSHA256, optionsFrom(candidate),
		receipts, "", candidate.Node.NodeID,
	)
	if err != nil {
		t.Fatalf("build plain successor: %v", err)
	}
	if err := ValidateSelectedCandidateSource(plain, fixtureSnapshot(t), receipts, nil); err != nil {
		t.Fatalf("validate ordered closure: %v", err)
	}
	receipts[0], receipts[1] = receipts[1], receipts[0]
	if err := ValidateSelectedCandidateSource(plain, fixtureSnapshot(t), receipts, nil); err == nil {
		t.Fatal("accepted a shuffled direct-predecessor closure")
	}
	if err := ValidateSelectedCandidateSource(
		plain, fixtureSnapshot(t), receipts[:1], nil,
	); err == nil {
		t.Fatal("accepted a missing direct-predecessor receipt")
	}
}

func TestValidateSelectedCandidateSourceEnforcesContentArtifactRules(t *testing.T) {
	candidate, receipts, artifact := selectedContentFixture(t, "verified predecessor output")
	if err := ValidateSelectedCandidateSource(candidate, fixtureSnapshot(t), receipts, nil); err == nil {
		t.Fatal("accepted disclosed content without an artifact")
	}
	wrong := selectedArtifactFixture(t, receipts[1], "verified predecessor output")
	if err := ValidateSelectedCandidateSource(
		candidate, fixtureSnapshot(t), receipts, &wrong,
	); err == nil {
		t.Fatal("accepted content from a non-first predecessor artifact")
	}
	stale := artifact
	stale.ArtifactSHA256 = strings.Repeat("0", 64)
	if err := ValidateSelectedCandidateSource(
		candidate, fixtureSnapshot(t), receipts, &stale,
	); err == nil {
		t.Fatal("accepted a typed artifact with stale identity")
	}

	plain, err := BuildSuccessor(
		fixtureSnapshot(t), candidate.ScheduleSHA256, optionsFrom(candidate),
		receipts, "", candidate.Node.NodeID,
	)
	if err != nil {
		t.Fatalf("build plain successor: %v", err)
	}
	if err := ValidateSelectedCandidateSource(
		plain, fixtureSnapshot(t), receipts, &artifact,
	); err == nil {
		t.Fatal("accepted an artifact when predecessor content is absent")
	}
}

func TestValidateSelectedCandidateSourceRejectsSuccessorSourceDrift(t *testing.T) {
	candidate, receipts, artifact := selectedContentFixture(t, "verified predecessor output")
	candidate.Node.AgentProfile = "different-profile"
	resignCandidate(t, &candidate)
	if validateCandidate(candidate) != nil {
		t.Fatal("source drift fixture should remain intrinsically valid")
	}
	if err := ValidateSelectedCandidateSource(
		candidate, fixtureSnapshot(t), receipts, &artifact,
	); err == nil {
		t.Fatal("accepted a self-consistent successor source substitution")
	}
}

func selectedContentFixture(
	t *testing.T,
	content string,
) (ScheduledNodeContractCandidate, []scheduledterminal.Receipt, scheduledterminal.Artifact) {
	t.Helper()
	receipts := []scheduledterminal.Receipt{
		successorReceiptForNode(t, 0), successorReceiptForNode(t, 1),
	}
	artifact := selectedArtifactFixture(t, receipts[0], content)
	receipts[0] = bindSelectedReceipt(t, receipts[0], artifact)
	snapshot, schedule := fixtureSnapshot(t), mustSchedule(t)
	candidate, err := BuildSuccessor(
		snapshot, schedule.ScheduleSHA256, readSourceFixture(t).Input.ExecutionOptions.options(),
		receipts, content, schedule.Nodes[2].NodeID,
	)
	if err != nil {
		t.Fatalf("BuildSuccessor: %v", err)
	}
	return candidate, receipts, artifact
}

func selectedArtifactFixture(
	t *testing.T,
	receipt scheduledterminal.Receipt,
	content string,
) scheduledterminal.Artifact {
	t.Helper()
	value := scheduledterminal.Artifact{
		V: 1, TerminalArtifactProtocol: 1, ArtifactKind: "result",
		GraphRunID: receipt.GraphRunID, NodeID: receipt.NodeID, Attempt: receipt.Attempt,
		DispatchID: receipt.DispatchID, ProviderRequestID: receipt.ProviderRequestID,
		ClaimEventSHA256: strings.Repeat("1", 64), AuthorizationSHA256: strings.Repeat("2", 64),
		ProviderRequestSHA256: strings.Repeat("3", 64), RequestBodySHA256: strings.Repeat("4", 64),
		PricingSnapshotSHA256: strings.Repeat("5", 64), LaneOwnershipID: "lane-owner",
		ProjectLaneSHA256: receipt.ProjectLaneSHA256, ProviderPollStarted: true,
		TerminalSeen: true, StreamEOFSeen: true, Classification: "completed",
		OutputText: content, OutputBytes: len([]byte(content)), OutputSHA256: selectedOutputDigest(content),
		UsageObserved: true, InputTokens: 1, OutputTokens: 1, CreatedAtMS: 1,
	}
	encoded, err := scheduledterminal.MarshalArtifact(value)
	if err != nil {
		t.Fatalf("MarshalArtifact: %v", err)
	}
	decoded, err := scheduledterminal.DecodeArtifact(encoded)
	if err != nil {
		t.Fatalf("DecodeArtifact: %v", err)
	}
	return decoded
}

func bindSelectedReceipt(
	t *testing.T,
	receipt scheduledterminal.Receipt,
	artifact scheduledterminal.Artifact,
) scheduledterminal.Receipt {
	t.Helper()
	receipt.ArtifactID, receipt.ArtifactSHA256 = artifact.ArtifactID, artifact.ArtifactSHA256
	encoded, err := scheduledterminal.MarshalReceipt(receipt)
	if err != nil {
		t.Fatalf("MarshalReceipt: %v", err)
	}
	decoded, err := scheduledterminal.DecodeReceipt(encoded)
	if err != nil {
		t.Fatalf("DecodeReceipt: %v", err)
	}
	return decoded
}

func selectedOutputDigest(content string) string {
	hash := sha256.New()
	_, _ = hash.Write([]byte("forge.group-agent-scheduled-node-terminal-output.v1\x00"))
	_, _ = hash.Write([]byte(content))
	return hex.EncodeToString(hash.Sum(nil))
}
