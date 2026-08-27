package scheduledterminal

import (
	"strings"
	"testing"
)

func TestDecodeControlRejectsTrailingJSON(t *testing.T) {
	_, err := decodeControl([]byte(`{"v":1}{"v":1}`))
	if err == nil {
		t.Fatal("expected trailing JSON to be rejected")
	}
}

func TestDecodeControlRejectsNonCanonicalWhitespace(t *testing.T) {
	data := []byte(`{"v":1}`)
	if _, err := decodeControl(data); err == nil {
		t.Fatal("expected incomplete control to be rejected")
	}
	if _, err := decodeControl([]byte(strings.TrimSpace(string(data)) + "\n")); err == nil {
		t.Fatal("expected noncanonical control bytes to be rejected")
	}
}

func TestDigestWithoutFieldRecursivelySortsAndPreservesUint64(t *testing.T) {
	type nested struct {
		Z uint64 `json:"z"`
		A uint64 `json:"a"`
	}
	type envelope struct {
		SnapshotSHA256 string `json:"snapshot_sha256"`
		Nested         nested `json:"nested"`
	}
	value := envelope{
		SnapshotSHA256: strings.Repeat("f", 64),
		Nested:         nested{Z: ^uint64(0), A: 1},
	}

	got, err := digestWithoutField(value, "snapshot_sha256", "fixture\x00")
	if err != nil {
		t.Fatalf("digestWithoutField: %v", err)
	}
	preimage := []byte(`{"nested":{"a":1,"z":18446744073709551615}}`)
	want := digestBytes("fixture\x00", preimage)
	if got != want {
		t.Fatalf("digest = %s, want %s from %s", got, want, preimage)
	}
}

func TestReceiptRejectsOutcomeArtifactAndLaneDrift(t *testing.T) {
	cases := []struct {
		name   string
		mutate func(*Receipt)
	}{
		{"completed uncertainty", func(value *Receipt) { value.ArtifactKind = "uncertainty" }},
		{"failed result", func(value *Receipt) { value.NodeOutcome = "failed" }},
		{"lane retained", func(value *Receipt) { value.LaneReleaseAuthorized = false }},
		{"missing artifact identity", func(value *Receipt) { value.ArtifactID = "" }},
		{"drifted artifact identity", func(value *Receipt) { value.ArtifactID = "other-artifact" }},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			value := validReceiptFixture()
			test.mutate(&value)
			if _, err := MarshalReceipt(value); err == nil {
				t.Fatal("MarshalReceipt accepted inconsistent terminal facts")
			}
			resignReceiptFixture(t, &value)
			encoded, err := marshalCanonical(value)
			if err != nil {
				t.Fatalf("encode forged receipt: %v", err)
			}
			if _, err := DecodeReceipt(encoded); err == nil {
				t.Fatal("DecodeReceipt accepted inconsistent terminal facts")
			}
		})
	}
}

func validReceiptFixture() Receipt {
	return Receipt{
		V: 1, SchedulerProtocolVersion: 1, TerminalReceiptProtocol: terminalProtocol,
		TerminalControlSHA256: strings.Repeat("a", 64), GraphRunID: "graph-run",
		GraphID: "graph", NodeID: "frontend", Attempt: 1, DispatchID: "dispatch-frontend",
		ProviderRequestID: "scheduled-node-provider-request-frontend",
		ProjectLaneSHA256: strings.Repeat("b", 64), ArtifactKind: "result",
		ArtifactID:     "scheduled-node-terminal-artifact-" + strings.Repeat("c", 64),
		ArtifactSHA256: strings.Repeat("c", 64),
		NodeOutcome:    "completed", LaneReleaseAuthorized: true,
	}
}

func resignReceiptFixture(t *testing.T, value *Receipt) {
	t.Helper()
	digest, err := digestWithoutField(*value, []string{"receipt_id", "receipt_sha256"}, receiptDomain)
	if err != nil {
		t.Fatalf("receipt digest: %v", err)
	}
	value.ReceiptID = "scheduled-node-terminal-receipt-" + digest
	value.ReceiptSHA256 = digest
}
