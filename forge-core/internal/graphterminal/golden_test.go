package graphterminal

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

type sharedTerminalFixture struct {
	V                                uint16 `json:"v"`
	CanonicalClaimEventJSON          string `json:"canonical_claim_event_json"`
	ClaimEventSHA256                 string `json:"claim_event_sha256"`
	CanonicalTerminalArtifactJSON    string `json:"canonical_terminal_artifact_json"`
	ArtifactSHA256                   string `json:"artifact_sha256"`
	CanonicalUncertaintyArtifactJSON string `json:"canonical_uncertainty_artifact_json"`
	UncertaintyArtifactSHA256        string `json:"uncertainty_artifact_sha256"`
	CanonicalTerminalControlJSON     string `json:"canonical_terminal_control_json"`
	TerminalControlSHA256            string `json:"terminal_control_sha256"`
	CanonicalTerminalReceiptJSON     string `json:"canonical_terminal_receipt_json"`
	TerminalReceiptSHA256            string `json:"terminal_receipt_sha256"`
	CanonicalTerminalEventJSON       string `json:"canonical_terminal_event_json"`
	TerminalEventSHA256              string `json:"terminal_event_sha256"`
}

func TestSharedTerminalGolden(t *testing.T) {
	actual := terminalSharedFixture(t)
	expected := readSharedTerminalFixture(t)
	if expected != actual {
		t.Fatal("shared terminal fixture differs from rebuilt Go golden")
	}
	assertFixtureStringsNoLF(t, expected)
}

func TestSharedUncertaintyArtifactIsStrictlyValid(t *testing.T) {
	expected := readSharedTerminalFixture(t)
	artifact, err := decodeExact[TerminalArtifact](
		[]byte(expected.CanonicalUncertaintyArtifactJSON),
	)
	if err != nil || artifact.ArtifactSHA256 != expected.UncertaintyArtifactSHA256 {
		t.Fatal("shared uncertainty artifact is not exact canonical JSON")
	}
	fixture := validTerminalFixture(t)
	fixture.Control.Artifact = artifact
	control := identifyControlFixture(t, fixture.Control)
	encoded := mustCanonicalFixture(t, control)
	decoded, err := DecodeControl(bytes.NewReader(encoded))
	if err != nil {
		t.Fatalf("strictly decode uncertainty control: %v", err)
	}
	receipt, err := BuildReceipt(decoded)
	if err != nil || receipt.GraphStatus != "failed_uncertain" {
		t.Fatalf("uncertainty receipt = %#v / %v", receipt, err)
	}
}

func TestEmitSharedTerminalGolden(t *testing.T) {
	if os.Getenv("FORGE_EMIT_TERMINAL_FIXTURE") != "1" {
		t.Skip("fixture emission is opt-in")
	}
	encoded, err := json.MarshalIndent(terminalSharedFixture(t), "", "  ")
	if err != nil {
		t.Fatalf("encode fixture: %v", err)
	}
	fmt.Printf("FIXTURE_BEGIN\n%s\nFIXTURE_END\n", encoded)
}

func terminalSharedFixture(t *testing.T) sharedTerminalFixture {
	t.Helper()
	fixture := validTerminalFixture(t)
	uncertainty := buildUncertaintyArtifactFixture(t, fixture.Control.Artifact)
	terminalEventJSON := mustCanonicalFixture(t, terminalEventFixture(fixture))
	return sharedTerminalFixture{
		V: 1, CanonicalClaimEventJSON: string(fixture.ClaimJSON),
		ClaimEventSHA256:                 fixture.Control.Claim.ClaimEventSHA256,
		CanonicalTerminalArtifactJSON:    string(fixture.ArtifactJSON),
		ArtifactSHA256:                   fixture.Control.Artifact.ArtifactSHA256,
		CanonicalUncertaintyArtifactJSON: string(mustCanonicalFixture(t, uncertainty)),
		UncertaintyArtifactSHA256:        uncertainty.ArtifactSHA256,
		CanonicalTerminalControlJSON:     string(fixture.ControlJSON),
		TerminalControlSHA256:            fixture.Control.SnapshotSHA256,
		CanonicalTerminalReceiptJSON:     string(fixture.ReceiptJSON),
		TerminalReceiptSHA256:            fixture.Receipt.ReceiptSHA256,
		CanonicalTerminalEventJSON:       string(terminalEventJSON),
		TerminalEventSHA256:              rawDomainDigest(controlEventDomain, terminalEventJSON),
	}
}

func readSharedTerminalFixture(t *testing.T) sharedTerminalFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"group-agent-node-terminal-lifecycle-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read shared terminal fixture: %v", err)
	}
	var fixture sharedTerminalFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode shared terminal fixture: %v", err)
	}
	return fixture
}

func terminalEventFixture(fixture terminalFixture) nodeLifecycleTerminalizedEvent {
	receipt, claim := fixture.Receipt, fixture.Control.Claim
	return nodeLifecycleTerminalizedEvent{
		V: 5, GraphRunID: receipt.GraphRunID, Seq: 5, Type: "node_lifecycle_terminalized",
		PreviousEventSHA256: claim.ClaimEventSHA256, DispatchID: receipt.DispatchID,
		LaneOwnershipID: receipt.LaneOwnershipID, ProjectLaneSHA256: receipt.ProjectLaneSHA256,
		ArtifactID: receipt.ArtifactID, ArtifactSHA256: receipt.ArtifactSHA256,
		TerminalReceiptID: receipt.ReceiptID, TerminalReceiptSHA256: receipt.ReceiptSHA256,
		NodeID: receipt.NodeID, Attempt: receipt.Attempt, NodeOutcome: receipt.NodeOutcome,
		WaveIndex: receipt.WaveIndex, WaveOutcome: receipt.WaveOutcome,
		GraphStatus: receipt.GraphStatus, RetryAuthorized: false,
		LaneReleased: true, TerminalizedAtMS: 120,
	}
}

func assertFixtureStringsNoLF(t *testing.T, value sharedTerminalFixture) {
	t.Helper()
	for name, text := range map[string]string{
		"claim": value.CanonicalClaimEventJSON, "artifact": value.CanonicalTerminalArtifactJSON,
		"uncertainty_artifact": value.CanonicalUncertaintyArtifactJSON,
		"control":              value.CanonicalTerminalControlJSON, "receipt": value.CanonicalTerminalReceiptJSON,
		"terminal_event": value.CanonicalTerminalEventJSON,
	} {
		if text == "" || text[len(text)-1] == '\n' {
			t.Fatalf("%s fixture string is empty or has trailing LF", name)
		}
	}
}
