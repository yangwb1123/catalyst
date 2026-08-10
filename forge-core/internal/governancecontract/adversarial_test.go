package governancecontract

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"strings"
	"testing"
)

func TestDecodeRecordRejectsAdversarialJSON(t *testing.T) {
	evidence, claim := goldenRecordStrings(t)
	tests := []struct {
		name  string
		input string
		want  string
	}{
		{"duplicate key", strings.Replace(evidence, `{"api_version":`, `{"api_version":"forgeos.governance/v1","api_version":`, 1), "duplicate JSON key"},
		{"float", strings.Replace(evidence, `1700000000000`, `1700000000000.0`, 1), "signed int64"},
		{"noncanonical whitespace", " " + evidence, "not exact compact canonical"},
		{"wrong digest", strings.Replace(evidence, `dc6963537f59e0594e6d5d1651e16070b81365ff379acc5ec09956b18e4b17b4`, strings.Repeat("a", 64), 1), "canonical_sha256 mismatch"},
		{"forbidden bidi", strings.Replace(claim, "构建通过", "构建\u202e通过", 1), "forbidden Unicode"},
		{"authority state", strings.Replace(claim, `"state":"candidate"`, `"state":"confirmed"`, 1), "unavailable authority"},
		{"unknown key", strings.Replace(evidence, `{"api_version":`, `{"alien":null,"api_version":`, 1), "expected exactly"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := DecodeRecord([]byte(test.input))
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("DecodeRecord error = %v, want substring %q", err, test.want)
			}
		})
	}
}

func TestDecodeRecordRejectsNullForRequiredIntegers(t *testing.T) {
	evidence, _ := goldenRecordStrings(t)
	tests := [][2]string{
		{"metadata", "created_at_unix_ms"},
		{"spec", "observed_at_unix_ms"},
		{"status", "valid_from_unix_ms"},
	}
	for _, test := range tests {
		t.Run(test[0]+"."+test[1], func(t *testing.T) {
			raw := signedNullIntegerMutation(t, evidence, test)
			if _, err := DecodeRecord(raw); err == nil {
				t.Fatalf("DecodeRecord accepted null required integer")
			}
		})
	}
	raw := signedNullIntegerMutation(t, evidence, tests...)
	if _, err := DecodeRecord(raw); err == nil || !strings.Contains(err.Error(), "does not preserve") {
		t.Fatalf("DecodeRecord all-null error = %v, want typed wire mismatch", err)
	}
}

func signedNullIntegerMutation(t *testing.T, evidence string, fields ...[2]string) []byte {
	t.Helper()
	node, err := parseStrictJSON([]byte(evidence))
	if err != nil {
		t.Fatalf("parse golden evidence: %v", err)
	}
	root := node.(map[string]any)
	for _, field := range fields {
		root[field[0]].(map[string]any)[field[1]] = nil
	}
	state, err := canonicalStateFromNode(root, EvidenceKind)
	if err != nil {
		t.Fatalf("digest null mutation: %v", err)
	}
	root["integrity"].(map[string]any)["canonical_sha256"] = state.digest
	raw, err := canonicalJSON(root)
	if err != nil {
		t.Fatalf("encode null mutation: %v", err)
	}
	return raw
}

func TestValidateRecordSetRejectsDanglingReference(t *testing.T) {
	records := goldenRecords(t)
	records[1].Claim.Spec.SupportingEvidenceRecordIDs = []string{"evr-missing"}
	resignRecord(t, records[1])
	err := ValidateRecordSet(records)
	if err == nil || !strings.Contains(err.Error(), "missing EvidenceRecord") {
		t.Fatalf("ValidateRecordSet error = %v, want dangling evidence reference", err)
	}
}

func TestValidateRecordSetRejectsSelfDerivedClaim(t *testing.T) {
	records := goldenRecords(t)
	records[1].Claim.Spec.DerivedFromClaimRecordIDs = []string{"kcr-0001"}
	resignRecord(t, records[1])
	err := ValidateRecordSet(records)
	if err == nil || !strings.Contains(err.Error(), "cannot derive from itself") {
		t.Fatalf("ValidateRecordSet error = %v, want self-reference rejection", err)
	}
}

func TestValidateRecordSetRejectsClaimDerivationCycle(t *testing.T) {
	records := goldenRecords(t)
	derived := cloneClaimRecord(records[1], "kcr-other", "claim-other")
	records[1].Claim.Spec.DerivedFromClaimRecordIDs = []string{"kcr-other"}
	derived.Claim.Spec.DerivedFromClaimRecordIDs = []string{"kcr-0001"}
	resignRecord(t, records[1])
	resignRecord(t, derived)
	records = append(records, derived)
	err := ValidateRecordSet(records)
	if err == nil || !strings.Contains(err.Error(), "claim derivation cycle") {
		t.Fatalf("ValidateRecordSet error = %v, want derivation cycle", err)
	}
}

func TestValidateRecordSetAllowsCrossSubjectDerivation(t *testing.T) {
	records := goldenRecords(t)
	otherEvidence := cloneEvidenceRecord(records[0], "evr-other", 1, nil)
	otherEvidence.Evidence.Spec = records[0].Evidence.Spec
	otherEvidence.Evidence.Spec.Subjects = []string{"module:other"}
	otherClaim := cloneClaimRecord(records[1], "kcr-other", "claim-other")
	otherClaim.Claim.Spec.Subject = "module:other"
	otherClaim.Claim.Spec.SupportingEvidenceRecordIDs = []string{"evr-other"}
	records[1].Claim.Spec.DerivedFromClaimRecordIDs = []string{"kcr-other"}
	resignRecord(t, otherEvidence)
	resignRecord(t, records[1])
	resignRecord(t, otherClaim)
	records = []*Record{records[0], otherEvidence, records[1], otherClaim}
	if err := ValidateRecordSet(records); err != nil {
		t.Fatalf("ValidateRecordSet rejected cross-subject derivation: %v", err)
	}
}

func TestValidateRecordSetRejectsSupersessionCycle(t *testing.T) {
	base := goldenRecords(t)[0]
	first := cloneEvidenceRecord(base, "evr-cycle-a", 2, []string{"evr-cycle-b"})
	second := cloneEvidenceRecord(base, "evr-cycle-b", 3, []string{"evr-cycle-a"})
	resignRecord(t, first)
	resignRecord(t, second)
	err := ValidateRecordSet([]*Record{first, second})
	if err == nil || !strings.Contains(err.Error(), "supersession cycle") {
		t.Fatalf("ValidateRecordSet error = %v, want cycle", err)
	}
}

func TestDecodeRecordSetRejectsNoncanonicalArray(t *testing.T) {
	evidence, claim := goldenRecordStrings(t)
	_, err := DecodeRecordSet([]byte("[" + evidence + ", " + claim + "]"))
	if err == nil || !strings.Contains(err.Error(), "not exact compact canonical") {
		t.Fatalf("DecodeRecordSet error = %v, want noncanonical rejection", err)
	}
}

func TestRepositoryLocatorRejectsWindowsDrivePaths(t *testing.T) {
	for _, path := range []string{"C:/Windows/system.ini", "C:system.ini"} {
		if safeRepositoryPath(path) {
			t.Fatalf("safeRepositoryPath(%q) = true, want false", path)
		}
	}
}

func TestCanonicalStateReservesTheSealedDigestBytes(t *testing.T) {
	_, claim := goldenRecordStrings(t)
	node, err := parseStrictJSONBounded([]byte(claim), len(claim))
	if err != nil {
		t.Fatalf("parse golden claim: %v", err)
	}
	root := node.(map[string]any)
	root["integrity"].(map[string]any)["canonical_sha256"] = ""
	spec := root["spec"].(map[string]any)
	for index, field := range []string{"supporting_evidence_record_ids", "contradicting_evidence_record_ids", "derived_from_claim_record_ids"} {
		spec[field] = boundaryRecordIDs(byte('a' + index))
	}
	spec["object_value"], spec["reasoning"] = strings.Repeat("x", maxStringBytes), ""
	base, err := canonicalJSON(root)
	if err != nil {
		t.Fatalf("canonicalize boundary base: %v", err)
	}
	target := maxRecordBytes - sha256.Size*2 + 1
	padding := target - len(base)
	if padding <= 0 || padding > maxStringBytes {
		t.Fatalf("boundary padding = %d, want 1..%d", padding, maxStringBytes)
	}
	spec["reasoning"] = strings.Repeat("r", padding)
	if state, err := canonicalStateFromNode(root, ClaimKind); err == nil {
		t.Fatalf("sealed-overflow state unexpectedly succeeded: %d bytes", len(state.payload))
	}
}

func boundaryRecordIDs(prefix byte) []any {
	values := make([]any, maxArrayItems)
	for index := range values {
		values[index] = fmt.Sprintf("%c:%03d:%s", prefix, index, strings.Repeat("x", 134))
	}
	return values
}

func TestStructuralValidityDoesNotClaimAuthority(t *testing.T) {
	if !strings.Contains(structuralValidity, "no truth or authority attestation") {
		t.Fatalf("positive result overclaims: %q", structuralValidity)
	}
}

func TestValidateRecordSetRevalidatesMutableRecords(t *testing.T) {
	tests := []struct {
		name   string
		mutate func([]*Record)
		want   string
	}{
		{"field mutation", func(records []*Record) { records[1].Claim.Spec.Reasoning += "changed" }, "canonical_sha256 mismatch"},
		{"oversized array", mutateOversizedSubjects, "exceeds 256 items"},
		{"oversized string", func(records []*Record) {
			records[1].Claim.Spec.ObjectValue.String = strings.Repeat("x", maxStringBytes+1)
		}, "string byte length exceeds"},
		{"kind mismatch", func(records []*Record) { records[0].Evidence.Kind = ClaimKind }, `EvidenceRecord kind must be "EvidenceRecord"`},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			records := goldenRecords(t)
			test.mutate(records)
			err := ValidateRecordSet(records)
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("ValidateRecordSet error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestValidateRecordSetRejectsMoreThan256Records(t *testing.T) {
	base := goldenRecords(t)[0]
	records := make([]*Record, maxArrayItems+1)
	for index := range records {
		records[index] = base
	}
	err := ValidateRecordSet(records)
	if err == nil || !strings.Contains(err.Error(), "1..256 records") {
		t.Fatalf("ValidateRecordSet error = %v, want top-level record limit", err)
	}
}

func TestProgrammaticRecordRejectsInvalidUTF8(t *testing.T) {
	records := goldenRecords(t)
	records[1].Claim.Spec.ObjectValue.String = string([]byte{0xff, 0xfe})
	err := ValidateRecordSet(records)
	if err == nil || !strings.Contains(err.Error(), "not valid UTF-8") {
		t.Fatalf("ValidateRecordSet error = %v, want invalid UTF-8 rejection", err)
	}
}

func TestRecordGettersDoNotReturnStaleCacheAfterMutation(t *testing.T) {
	record := goldenRecords(t)[0]
	record.Evidence.Metadata.Scope = "module:mutated"
	if got := record.RecordJSON(); got != nil {
		t.Fatalf("RecordJSON returned %d stale bytes", len(got))
	}
	if got := record.PayloadJSON(); got != nil {
		t.Fatalf("PayloadJSON returned %d stale bytes", len(got))
	}
	if got := record.Digest(); got != "" {
		t.Fatalf("Digest returned stale value %q", got)
	}
}

func TestHTMLCharactersNearRecordSetBoundary(t *testing.T) {
	base := goldenRecords(t)[1]
	valid, overflow := boundaryClaimSets(t, base)
	if len(valid) == 0 || len(overflow) != len(valid)+1 {
		t.Fatalf("failed to construct boundary record sets")
	}
	encoded := valid[0].RecordJSON()
	if bytes.Contains(encoded, []byte(`\u003c`)) || !bytes.Contains(encoded, []byte("<>&")) {
		t.Fatalf("canonical JSON HTML escaping is incorrect")
	}
	if err := ValidateRecordSet(valid); err != nil {
		t.Fatalf("near-boundary record set rejected: %v", err)
	}
	err := ValidateRecordSet(overflow)
	if err == nil || !strings.Contains(err.Error(), "record set exceeds") {
		t.Fatalf("overflow record set error = %v, want final canonical byte limit", err)
	}
}

func goldenRecordStrings(t *testing.T) (string, string) {
	t.Helper()
	fixture := loadGoldenFixture(t)
	return fixture.Records[0].Expected.CanonicalRecordJSON, fixture.Records[1].Expected.CanonicalRecordJSON
}

func goldenRecords(t *testing.T) []*Record {
	t.Helper()
	fixture := loadGoldenFixture(t)
	records := make([]*Record, 0, len(fixture.Records))
	for _, entry := range fixture.Records {
		records = append(records, assertGoldenEntry(t, entry))
	}
	return records
}

func cloneEvidenceRecord(base *Record, recordID string, sequence int64, supersedes []string) *Record {
	evidence := *base.Evidence
	evidence.Metadata = base.Evidence.Metadata
	evidence.Metadata.RecordID = recordID
	evidence.Metadata.AggregateID = "evidence-cycle"
	evidence.Metadata.Sequence = sequence
	evidence.Metadata.SupersedesRecordIDs = append([]string{}, supersedes...)
	return &Record{Evidence: &evidence, canonicalRecord: base.RecordJSON()}
}

func cloneClaimRecord(base *Record, recordID, aggregateID string) *Record {
	claim := *base.Claim
	claim.Metadata = base.Claim.Metadata
	claim.Metadata.RecordID = recordID
	claim.Metadata.AggregateID = aggregateID
	claim.Spec = base.Claim.Spec
	return &Record{Claim: &claim, canonicalRecord: base.RecordJSON()}
}

func resignRecord(t *testing.T, record *Record) {
	t.Helper()
	state, err := canonicalStateFromTyped(record)
	if err != nil {
		t.Fatalf("canonicalize mutated record: %v", err)
	}
	if record.Evidence != nil {
		record.Evidence.Integrity.CanonicalSHA256 = state.digest
	} else {
		record.Claim.Integrity.CanonicalSHA256 = state.digest
	}
	state, err = canonicalStateFromTyped(record)
	if err != nil {
		t.Fatalf("canonicalize resigned record: %v", err)
	}
	applyCanonicalState(record, state)
}

func mutateOversizedSubjects(records []*Record) {
	subjects := make([]string, maxArrayItems+1)
	for index := range subjects {
		subjects[index] = fmt.Sprintf("subject:%03d", index)
	}
	records[0].Evidence.Spec.Subjects = subjects
}

func boundaryClaimSets(t *testing.T, base *Record) ([]*Record, []*Record) {
	t.Helper()
	records, total := make([]*Record, 0), 2
	for index := 0; index < maxArrayItems; index++ {
		record := boundaryClaim(t, base, index)
		addition := len(record.RecordJSON())
		if len(records) > 0 {
			addition++
		}
		if total+addition > maxSetBytes {
			return records, append(append([]*Record{}, records...), record)
		}
		records, total = append(records, record), total+addition
	}
	t.Fatalf("boundary claims did not reach the record-set limit")
	return nil, nil
}

func boundaryClaim(t *testing.T, base *Record, index int) *Record {
	t.Helper()
	id := fmt.Sprintf("kcr-edge-%03d", index)
	record := cloneClaimRecord(base, id, "claim-edge-"+fmt.Sprintf("%03d", index))
	record.Claim.Spec.ClaimType = "proposal"
	record.Claim.Spec.SupportingEvidenceRecordIDs = []string{}
	record.Claim.Spec.ObjectValue.String = strings.Repeat("<>&", 5400)
	record.Claim.Spec.Reasoning = "record-set canonical HTML boundary"
	record.Claim.Status.State = "draft"
	resignRecord(t, record)
	return record
}
