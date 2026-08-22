package approvalcontext

import (
	"bytes"
	"strings"
	"testing"
)

func contextFixture() Context {
	digest := strings.Repeat("a", 64)
	return Context{
		Format: ContextFormat, AgentOutputReceiptSHA256: digest,
		ArtifactInputsSHA256: digest, ArtifactOutputsSHA256: digest,
		CreatedAtUnixMS: 1, LocalRuntimePolicySHA256: digest,
		PromptContextSHA256: digest, RunID: "run-1", SourceAfterSHA256: digest,
		Stage: "design", Workflow: "design", WorkflowSHA256: digest,
	}
}

func TestContextAndPositiveMarkerCanonicalRoundTrip(t *testing.T) {
	context := contextFixture()
	data, err := CanonicalContextJSON(context)
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeCanonicalContext(data)
	if err != nil || decoded != context {
		t.Fatalf("context roundtrip = %#v, %v", decoded, err)
	}
	digest, err := ContextSHA256(context)
	if err != nil || !validDigest(digest) {
		t.Fatalf("context digest = %q, %v", digest, err)
	}
	marker := PositiveMarkerFromContext(context, digest, "local-operator", 2)
	markerData, err := CanonicalMarkerJSON(marker)
	if err != nil {
		t.Fatal(err)
	}
	decodedMarker, err := DecodeCanonicalMarker(markerData)
	if err != nil || decodedMarker != marker {
		t.Fatalf("marker roundtrip = %#v, %v", decodedMarker, err)
	}
	if err := ValidateMarkerContext(marker, context); err != nil {
		t.Fatal(err)
	}
}

func TestCanonicalDecodersRejectAmbiguousOrDriftedWire(t *testing.T) {
	data, err := CanonicalContextJSON(contextFixture())
	if err != nil {
		t.Fatal(err)
	}
	for _, candidate := range [][]byte{
		append(append([]byte(nil), data...), '\n'),
		bytes.Replace(data, []byte(`"run_id":"run-1"`), []byte(`"run_id":"run-1","run_id":"run-1"`), 1),
		bytes.Replace(data, []byte(`"run_id":"run-1"`), []byte(`"unknown":1,"run_id":"run-1"`), 1),
		bytes.Replace(data, []byte(`"created_at_unix_ms":1`), []byte(`"created_at_unix_ms":1.0`), 1),
	} {
		if _, err := DecodeCanonicalContext(candidate); err == nil {
			t.Fatalf("ambiguous context accepted: %s", candidate)
		}
	}
}

func TestPositiveMarkerRejectsTamperedContextReference(t *testing.T) {
	context := contextFixture()
	digest, _ := ContextSHA256(context)
	marker := PositiveMarkerFromContext(context, digest, "operator", 2)
	marker.SourceAfterSHA256 = strings.Repeat("b", 64)
	if err := ValidateMarkerContext(marker, context); err == nil {
		t.Fatal("tampered marker reference accepted")
	}
	marker = PositiveMarkerFromContext(context, digest, "operator", 2)
	marker.Decision = "rejected"
	if _, err := CanonicalMarkerJSON(marker); err == nil {
		t.Fatal("negative decision accepted as a positive marker")
	}
}
