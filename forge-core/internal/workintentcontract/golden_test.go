package workintentcontract

import (
	"bytes"
	"encoding/json"
	"os"
	"testing"
)

func TestGoldenPhysicalPinsAndExactFraming(t *testing.T) {
	physical := readGoldenPhysical(t)
	if got := physicalSHA256(t, repositoryPath("docs", "contracts", "fixtures",
		"work-intent-v1.json")); got != GoldenPhysicalSHA256 {
		t.Fatalf("golden physical SHA-256 = %s", got)
	}
	if got := physicalSHA256(t, repositoryPath("docs", "contracts",
		"work-intent-v1.schema.json")); got != SchemaPhysicalSHA256 {
		t.Fatalf("schema physical SHA-256 = %s", got)
	}
	if _, err := DecodeCanonicalWorkIntent(physical); err == nil {
		t.Fatal("physical fixture with LF was accepted as an instance")
	}
}

func TestGoldenCanonicalBytesAndIdentity(t *testing.T) {
	document, instance := loadGolden(t)
	if document.WorkIntentSHA256 != GoldenRecordSHA256 {
		t.Fatalf("record digest = %s", document.WorkIntentSHA256)
	}
	if document.WorkIntentID != workIntentIDPrefix+GoldenRecordSHA256 {
		t.Fatalf("record ID = %s", document.WorkIntentID)
	}
	encoded, err := CanonicalWorkIntentJSON(document)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(encoded, instance) {
		t.Fatal("Go canonical bytes differ from the Python golden")
	}
}

func TestGoldenSealAndDigestDoNotMutateCandidate(t *testing.T) {
	candidate := blankGolden(t)
	sealed := mustSeal(t, candidate)
	if candidate.WorkIntentID != "" || candidate.WorkIntentSHA256 != "" {
		t.Fatal("SealWorkIntent mutated its caller")
	}
	if sealed.WorkIntentSHA256 != GoldenRecordSHA256 {
		t.Fatalf("sealed digest = %s", sealed.WorkIntentSHA256)
	}
	digest, err := WorkIntentSHA256(sealed)
	if err != nil || digest != GoldenRecordSHA256 {
		t.Fatalf("WorkIntentSHA256 = %q, %v", digest, err)
	}
}

func TestGoldenEarlyDeadlineIsAllowed(t *testing.T) {
	document, _ := loadGolden(t)
	if document.Intent.DeadlineUnixMS == nil ||
		*document.Intent.DeadlineUnixMS >= document.DeclaredAtUnixMS {
		t.Fatal("golden does not exercise an early deadline")
	}
	if err := ValidateWorkIntent(document); err != nil {
		t.Fatal(err)
	}
}

func TestSuccessMarkerMatchesFrozenSchemaExactly(t *testing.T) {
	expected := "STRUCTURALLY_VALID_DECLARED_WORK_INTENT_V1 (exact caller-supplied " +
		"declaration only; no origin authentication, reference resolution, G0, routing, " +
		"Run or RunJournal existence, lifecycle, approval, authentication, authority, " +
		"completion, effect, execution, freshness, materiality, ownership, permission, " +
		"persistence, scope, or truth attestation)"
	if SUCCESS_MARKER != expected {
		t.Fatalf("SUCCESS_MARKER = %q", SUCCESS_MARKER)
	}
	raw, err := os.ReadFile(repositoryPath("docs", "contracts", "work-intent-v1.schema.json"))
	if err != nil {
		t.Fatal(err)
	}
	var schema map[string]any
	if err := json.Unmarshal(raw, &schema); err != nil {
		t.Fatal(err)
	}
	authority := schema["x-forgeos-authority-semantics"].(map[string]any)
	if authority["positive_result"] != SUCCESS_MARKER {
		t.Fatalf("schema positive_result = %#v", authority["positive_result"])
	}
}
