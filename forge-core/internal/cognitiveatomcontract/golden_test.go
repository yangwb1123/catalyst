package cognitiveatomcontract

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

type goldenFixture struct {
	APIVersion       string            `json:"api_version"`
	Canonicalization string            `json:"canonicalization"`
	DigestDomains    map[string]string `json:"digest_domains"`
	Expected         goldenExpected    `json:"expected"`
	TaskID           string            `json:"task_id"`
}

type goldenExpected struct {
	AtomID                     string `json:"atom_id"`
	AtomSetSHA256              string `json:"atom_set_sha256"`
	CanonicalAtomJSON          string `json:"canonical_atom_json"`
	CanonicalAtomPayloadJSON   string `json:"canonical_atom_payload_json"`
	CanonicalAtomSetJSON       string `json:"canonical_atom_set_json"`
	CanonicalAtomSHA256        string `json:"canonical_atom_sha256"`
	CanonicalSourceClosureJSON string `json:"canonical_source_closure_json"`
	SourceClosureSHA256        string `json:"source_closure_sha256"`
}

func TestGoldenProjection(t *testing.T) {
	fixture := loadGoldenFixture(t)
	projection, err := ProjectRecordSet(fixture.TaskID, []byte(fixture.Expected.CanonicalSourceClosureJSON))
	if err != nil {
		t.Fatalf("ProjectRecordSet: %v", err)
	}
	if len(projection.Atoms) != 1 {
		t.Fatalf("atom count = %d, want 1", len(projection.Atoms))
	}
	atom := projection.Atoms[0]
	assertEqual(t, "atom id", atom.ID(), fixture.Expected.AtomID)
	assertEqual(t, "atom digest", atom.Digest(), fixture.Expected.CanonicalAtomSHA256)
	assertEqual(t, "atom payload", string(atom.PayloadJSON()), fixture.Expected.CanonicalAtomPayloadJSON)
	assertEqual(t, "atom JSON", string(atom.AtomJSON()), fixture.Expected.CanonicalAtomJSON)
	assertEqual(t, "atom set JSON", string(projection.CanonicalAtomSetJSON), fixture.Expected.CanonicalAtomSetJSON)
	assertEqual(t, "atom set digest", projection.AtomSetSHA256, fixture.Expected.AtomSetSHA256)
	closure := projection.SourceClosures[atom.Source.ClaimRecordID]
	assertEqual(t, "closure JSON", string(closure.CanonicalJSON), fixture.Expected.CanonicalSourceClosureJSON)
	assertEqual(t, "closure digest", closure.SHA256, fixture.Expected.SourceClosureSHA256)
	if closure.ByteCount != int64(len([]byte(fixture.Expected.CanonicalSourceClosureJSON))) || closure.RecordCount != 2 {
		t.Fatalf("closure counts = (%d, %d)", closure.ByteCount, closure.RecordCount)
	}
	decoded, err := DecodeAtom([]byte(fixture.Expected.CanonicalAtomJSON))
	if err != nil {
		t.Fatalf("DecodeAtom: %v", err)
	}
	if !bytes.Equal(decoded.AtomJSON(), atom.AtomJSON()) {
		t.Fatal("typed decode did not preserve exact golden bytes")
	}
	if _, err := DecodeAtomSet([]byte(fixture.Expected.CanonicalAtomSetJSON)); err != nil {
		t.Fatalf("DecodeAtomSet: %v", err)
	}
	if err := CompareProjection(fixture.TaskID, []byte(fixture.Expected.CanonicalSourceClosureJSON), []byte(fixture.Expected.CanonicalAtomSetJSON)); err != nil {
		t.Fatalf("CompareProjection: %v", err)
	}
	if fixture.DigestDomains["atom"] != atomDigestDomain || fixture.DigestDomains["atom_id"] != atomIDDomain ||
		fixture.DigestDomains["atom_set"] != atomSetDigestDomain || fixture.DigestDomains["source_closure"] != closureDigestDomain {
		t.Fatalf("fixture digest domains drifted: %#v", fixture.DigestDomains)
	}
}

func TestProjectsEverySupportedShadowState(t *testing.T) {
	fixture := loadGoldenFixture(t)
	tests := []struct {
		claimType string
		state     string
	}{
		{"fact", "candidate"}, {"fact", "contested"},
		{"constraint", "candidate"}, {"decision", "proposed"}, {"inference", "candidate"},
		{"assumption", "open"}, {"assumption", "testing"},
		{"hypothesis", "open"}, {"hypothesis", "testing"},
		{"unknown", "open"}, {"unknown", "investigating"},
	}
	for _, test := range tests {
		t.Run(test.claimType+"/"+test.state, func(t *testing.T) {
			source := sourceVariant(t, fixture, test.claimType, test.state)
			projection, err := ProjectRecordSet(fixture.TaskID, source)
			if err != nil {
				t.Fatalf("ProjectRecordSet: %v", err)
			}
			got := projection.Atoms[0].Spec
			if got.AtomType != test.claimType || got.EpistemicState != test.state {
				t.Fatalf("projection state = %s/%s", got.AtomType, got.EpistemicState)
			}
			needsConfidence := test.claimType == "assumption" || test.claimType == "hypothesis" || test.claimType == "inference"
			if needsConfidence != (got.ProjectionConfidenceMicros != nil) {
				t.Fatalf("confidence presence mismatch for %s", test.claimType)
			}
		})
	}
}

func TestLessonAndProposalAreClosureOnly(t *testing.T) {
	fixture := loadGoldenFixture(t)
	for _, test := range []struct{ claimType, state string }{{"lesson", "candidate"}, {"proposal", "draft"}} {
		t.Run(test.claimType, func(t *testing.T) {
			source := sourceVariant(t, fixture, test.claimType, test.state)
			_, err := ProjectRecordSet(fixture.TaskID, source)
			if err == nil || !strings.Contains(err.Error(), "no projectable KnowledgeClaim") {
				t.Fatalf("ProjectRecordSet error = %v", err)
			}
		})
	}
}

func TestDecodeRejectsAdversarialAtomJSON(t *testing.T) {
	fixture := loadGoldenFixture(t)
	raw := fixture.Expected.CanonicalAtomJSON
	tests := []struct{ name, input, want string }{
		{"duplicate", strings.Replace(raw, `{"api_version":`, `{"api_version":"forgeos.aadm.cognitive-atom/v1","api_version":`, 1), "duplicate JSON key"},
		{"whitespace", " " + raw, "not exact compact canonical"},
		{"float", strings.Replace(raw, `"claim_sequence":1`, `"claim_sequence":1.0`, 1), "signed int64"},
		{"overflow", strings.Replace(raw, `"claim_sequence":1`, `"claim_sequence":9223372036854775808`, 1), "signed int64"},
		{"wrong digest", strings.Replace(raw, fixture.Expected.CanonicalAtomSHA256, strings.Repeat("a", 64), 1), "canonical_sha256 mismatch"},
		{"oversize", raw + strings.Repeat(" ", maxAtomBytes), "JSON byte length"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := DecodeAtom([]byte(test.input))
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("DecodeAtom error = %v, want %q", err, test.want)
			}
		})
	}

	unknown := mutateAndResignAtom(t, raw, func(root map[string]any) { root["alien"] = nil })
	if _, err := DecodeAtom(unknown); err == nil || !strings.Contains(err.Error(), "expected exactly") {
		t.Fatalf("unknown field error = %v", err)
	}
	bidi := mutateAndResignAtom(t, raw, func(root map[string]any) {
		root["spec"].(map[string]any)["proposition"].(map[string]any)["object_value"] = "unsafe\u202etext"
	})
	if _, err := DecodeAtom(bidi); err == nil || !strings.Contains(err.Error(), "forbidden Unicode") {
		t.Fatalf("bidi error = %v", err)
	}
}

func TestProjectionComparisonRejectsStructurallyValidFieldDrift(t *testing.T) {
	fixture := loadGoldenFixture(t)
	drifted := mutateAndResignAtom(t, fixture.Expected.CanonicalAtomJSON, func(root map[string]any) {
		source := root["source"].(map[string]any)
		source["closure_byte_count"] = source["closure_byte_count"].(int64) + 1
	})
	set := append(append([]byte{'['}, drifted...), ']')
	if _, err := DecodeAtomSet(set); err != nil {
		t.Fatalf("drifted atom should remain structurally valid: %v", err)
	}
	err := CompareProjection(fixture.TaskID, []byte(fixture.Expected.CanonicalSourceClosureJSON), set)
	if err == nil || !strings.Contains(err.Error(), "differs") {
		t.Fatalf("CompareProjection error = %v", err)
	}
}

func TestDecodeRejectsResignedSupportingContradictingOverlap(t *testing.T) {
	fixture := loadGoldenFixture(t)
	drifted := mutateAndResignAtom(t, fixture.Expected.CanonicalAtomJSON, func(root map[string]any) {
		spec := root["spec"].(map[string]any)
		supporting := spec["supporting_evidence_record_ids"].([]any)
		spec["contradicting_evidence_record_ids"] = []any{supporting[0]}
	})
	set := append(append([]byte{'['}, drifted...), ']')
	_, err := DecodeAtomSet(set)
	if err == nil || !strings.Contains(err.Error(), "must be disjoint") {
		t.Fatalf("DecodeAtomSet overlap error = %v", err)
	}
}

func TestAtomGettersRejectPostDecodeMutation(t *testing.T) {
	fixture := loadGoldenFixture(t)
	atom, err := DecodeAtom([]byte(fixture.Expected.CanonicalAtomJSON))
	if err != nil {
		t.Fatal(err)
	}
	atom.Spec.ProjectionMode = "changed"
	if atom.AtomJSON() != nil || atom.PayloadJSON() != nil || atom.Digest() != "" || atom.ID() != "" {
		t.Fatal("mutated atom returned stale verified values")
	}
}

func TestAtomSetRejectsEmptyAndDuplicate(t *testing.T) {
	fixture := loadGoldenFixture(t)
	if _, err := DecodeAtomSet([]byte("[]")); err == nil {
		t.Fatal("empty atom set accepted")
	}
	raw := fixture.Expected.CanonicalAtomJSON
	if _, err := DecodeAtomSet([]byte("[" + raw + "," + raw + "]")); err == nil || !strings.Contains(err.Error(), "unique atom_id") {
		t.Fatalf("duplicate atom set error = %v", err)
	}
}

func loadGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "cognitive-atom-projection-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	var fixture goldenFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatalf("decode fixture: %v", err)
	}
	return fixture
}

func sourceVariant(t *testing.T, fixture goldenFixture, claimType, state string) []byte {
	t.Helper()
	node, err := parseStrictJSONBounded([]byte(fixture.Expected.CanonicalSourceClosureJSON), maxSetBytes)
	if err != nil {
		t.Fatal(err)
	}
	array := node.([]any)
	claim := array[1].(map[string]any)
	spec := claim["spec"].(map[string]any)
	spec["claim_type"] = claimType
	spec["confidence_micros"] = nil
	spec["queue_ref"] = nil
	spec["validation_plan"] = nil
	if claimType == "assumption" || claimType == "hypothesis" || claimType == "inference" {
		spec["confidence_micros"] = int64(700000)
	}
	if claimType == "assumption" || claimType == "hypothesis" {
		spec["validation_plan"] = map[string]any{
			"due_at_unix_ms": int64(1700100000000), "impact_if_false": "reassess task",
			"method": "rerun structural test", "owner_id": "governance-review",
			"required_evidence_types": []any{"test_run"},
		}
	}
	if claimType == "unknown" {
		spec["queue_ref"] = "queue:unknown"
	}
	claim["status"].(map[string]any)["state"] = state
	resignGovernanceRecord(t, claim, "forgeos.governance.knowledge-claim.v1")
	encoded, err := canonicalJSON(array)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func resignGovernanceRecord(t *testing.T, root map[string]any, domain string) {
	t.Helper()
	integrity := root["integrity"].(map[string]any)
	integrity["canonical_sha256"] = ""
	payload, err := canonicalJSON(root)
	if err != nil {
		t.Fatal(err)
	}
	integrity["canonical_sha256"] = domainDigest(domain, payload)
}

func mutateAndResignAtom(t *testing.T, raw string, mutate func(map[string]any)) []byte {
	t.Helper()
	node, err := parseStrictJSONBounded([]byte(raw), maxAtomBytes)
	if err != nil {
		t.Fatal(err)
	}
	root := node.(map[string]any)
	mutate(root)
	integrity, ok := root["integrity"].(map[string]any)
	if ok {
		integrity["canonical_sha256"] = ""
		payload, err := canonicalJSON(root)
		if err != nil {
			t.Fatal(err)
		}
		integrity["canonical_sha256"] = domainDigest(atomDigestDomain, payload)
	}
	encoded, err := canonicalJSON(root)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func assertEqual(t *testing.T, name, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("%s mismatch\ngot:  %s\nwant: %s", name, got, want)
	}
}
