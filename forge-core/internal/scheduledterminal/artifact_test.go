package scheduledterminal

import (
	"bytes"
	"strings"
	"testing"
)

const terminalOutputDomain = "forge.group-agent-scheduled-node-terminal-output.v1\x00"

func TestArtifactDecodeIsStrictAndExact(t *testing.T) {
	artifact, encoded := validArtifactFixture(t, validReceiptFixture(), "verified output")
	decoded, err := DecodeArtifact(encoded)
	if err != nil || decoded != artifact {
		t.Fatalf("DecodeArtifact() = %#v, %v", decoded, err)
	}
	cases := map[string][]byte{
		"trailing newline": append(append([]byte(nil), encoded...), '\n'),
		"unknown field": bytes.Replace(
			encoded, []byte(`{"v":1,`), []byte(`{"v":1,"unknown":0,`), 1,
		),
		"duplicate field": bytes.Replace(
			encoded, []byte(`{"v":1,`), []byte(`{"v":1,"v":1,`), 1,
		),
	}
	for name, data := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeArtifact(data); err == nil {
				t.Fatal("DecodeArtifact accepted non-exact input")
			}
		})
	}
	artifact.ArtifactSHA256 = strings.Repeat("0", 64)
	stale, err := marshalCanonical(artifact)
	if err != nil {
		t.Fatalf("marshal stale artifact: %v", err)
	}
	if _, err := DecodeArtifact(stale); err == nil {
		t.Fatal("DecodeArtifact accepted a stale artifact digest")
	}
}

func TestArtifactCanonicalJSONUsesRawUTF8LineSeparators(t *testing.T) {
	artifact, encoded := validArtifactFixture(t, validReceiptFixture(), "\u2028\u2029")
	if !bytes.Contains(encoded, []byte(artifact.OutputText)) || bytes.Contains(encoded, []byte(`\u2028`)) {
		t.Fatalf("artifact does not preserve raw line separators: %q", encoded)
	}
	if decoded, err := DecodeArtifact(encoded); err != nil || decoded != artifact {
		t.Fatalf("DecodeArtifact() = %#v, %v", decoded, err)
	}
}

func TestValidatePredecessorContentBindsExactReceiptAndOutput(t *testing.T) {
	receipt := validReceiptFixture()
	artifact, _ := validArtifactFixture(t, receipt, "verified output")
	receipt = bindReceiptArtifact(t, receipt, artifact)
	if err := ValidatePredecessorContent(receipt, artifact, "verified output"); err != nil {
		t.Fatalf("ValidatePredecessorContent: %v", err)
	}
	if err := ValidatePredecessorContent(receipt, artifact, "different output"); err == nil {
		t.Fatal("accepted different disclosed output")
	}

	staleReceipt := receipt
	staleReceipt.ReceiptSHA256 = strings.Repeat("0", 64)
	if err := ValidatePredecessorContent(staleReceipt, artifact, artifact.OutputText); err == nil {
		t.Fatal("accepted a typed receipt with a stale digest")
	}
	staleArtifact := artifact
	staleArtifact.ArtifactSHA256 = strings.Repeat("0", 64)
	if err := ValidatePredecessorContent(receipt, staleArtifact, artifact.OutputText); err == nil {
		t.Fatal("accepted a typed artifact with a stale digest")
	}
}

func TestValidatePredecessorContentRejectsSelfConsistentDrift(t *testing.T) {
	receipt := validReceiptFixture()
	artifact, _ := validArtifactFixture(t, receipt, "verified output")
	receipt = bindReceiptArtifact(t, receipt, artifact)

	driftedArtifact := artifact
	driftedArtifact.DispatchID = "different-dispatch"
	driftedArtifact, _ = resignArtifactFixture(t, driftedArtifact)
	if err := ValidatePredecessorContent(receipt, driftedArtifact, artifact.OutputText); err == nil {
		t.Fatal("accepted a self-consistent artifact bound to another dispatch")
	}
	driftedReceipt := receipt
	driftedReceipt.ProviderRequestID = "different-provider-request"
	driftedReceipt = resignReceiptValue(t, driftedReceipt)
	if err := ValidatePredecessorContent(driftedReceipt, artifact, artifact.OutputText); err == nil {
		t.Fatal("accepted a self-consistent receipt bound to another request")
	}
	failed := receipt
	failed.NodeOutcome, failed.ArtifactKind = "failed", "uncertainty"
	failed = resignReceiptValue(t, failed)
	if err := ValidatePredecessorContent(failed, artifact, artifact.OutputText); err == nil {
		t.Fatal("accepted a non-completed predecessor receipt")
	}
}

func validArtifactFixture(
	t *testing.T,
	receipt Receipt,
	content string,
) (Artifact, []byte) {
	t.Helper()
	value := Artifact{
		V: 1, TerminalArtifactProtocol: terminalProtocol, ArtifactKind: "result",
		GraphRunID: receipt.GraphRunID, NodeID: receipt.NodeID, Attempt: receipt.Attempt,
		DispatchID: receipt.DispatchID, ProviderRequestID: receipt.ProviderRequestID,
		ClaimEventSHA256: strings.Repeat("1", 64), AuthorizationSHA256: strings.Repeat("2", 64),
		ProviderRequestSHA256: strings.Repeat("3", 64), RequestBodySHA256: strings.Repeat("4", 64),
		PricingSnapshotSHA256: strings.Repeat("5", 64), LaneOwnershipID: "lane-owner",
		ProjectLaneSHA256: receipt.ProjectLaneSHA256, ProviderPollStarted: true,
		TerminalSeen: true, StreamEOFSeen: true, Classification: "completed",
		OutputText: content, UsageObserved: true, InputTokens: 1, OutputTokens: 1, CreatedAtMS: 1,
	}
	return resignArtifactFixture(t, value)
}

func resignArtifactFixture(t *testing.T, value Artifact) (Artifact, []byte) {
	t.Helper()
	value.OutputBytes = len([]byte(value.OutputText))
	value.OutputSHA256 = digestBytes(terminalOutputDomain, []byte(value.OutputText))
	encoded, err := MarshalArtifact(value)
	if err != nil {
		t.Fatalf("MarshalArtifact: %v", err)
	}
	decoded, err := DecodeArtifact(encoded)
	if err != nil {
		t.Fatalf("DecodeArtifact: %v", err)
	}
	return decoded, encoded
}

func bindReceiptArtifact(t *testing.T, receipt Receipt, artifact Artifact) Receipt {
	t.Helper()
	receipt.ArtifactID, receipt.ArtifactSHA256 = artifact.ArtifactID, artifact.ArtifactSHA256
	return resignReceiptValue(t, receipt)
}

func resignReceiptValue(t *testing.T, value Receipt) Receipt {
	t.Helper()
	encoded, err := MarshalReceipt(value)
	if err != nil {
		t.Fatalf("MarshalReceipt: %v", err)
	}
	decoded, err := DecodeReceipt(encoded)
	if err != nil {
		t.Fatalf("DecodeReceipt: %v", err)
	}
	return decoded
}
