package commandobservationevidencecontract

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/governancecontract"
)

func TestCompareEvidenceAcceptsExactReadaptation(t *testing.T) {
	adaptation := mustAdapt(t, validRequest())
	if err := CompareEvidence(adaptation.RequestJSON(), adaptation.EvidenceJSON()); err != nil {
		t.Fatalf("CompareEvidence: %v", err)
	}
}

func TestCompareEvidenceRejectsStructurallyValidOutputDrift(t *testing.T) {
	base := mustAdapt(t, validRequest())
	tests := []struct {
		name string
		edit func(*governancecontract.EvidenceRecord)
	}{
		{"source trust", func(e *governancecontract.EvidenceRecord) { e.Spec.SourceTrust = "untrusted" }},
		{"collector parameters", func(e *governancecontract.EvidenceRecord) {
			e.Spec.Collector.ParametersSHA256 = strings.Repeat("0", 64)
		}},
		{"synthetic run", func(e *governancecontract.EvidenceRecord) { e.Metadata.CreatedBy.RunID = "command-adaptation-other" }},
		{"locator content", func(e *governancecontract.EvidenceRecord) { e.Spec.Locator.ContentSHA256 = strings.Repeat("1", 64) }},
		{"record id", func(e *governancecontract.EvidenceRecord) { e.Metadata.RecordID = "command-evidence-other" }},
		{"state", func(e *governancecontract.EvidenceRecord) {
			e.Status.State = "invalid"
			e.Status.ReasonCodes = []string{"drift"}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			evidence := *base.Evidence.Evidence
			evidence.Integrity.CanonicalSHA256 = ""
			test.edit(&evidence)
			candidate, err := sealAndRevalidateEvidence(evidence)
			if err != nil {
				t.Fatalf("drift must remain structurally valid: %v", err)
			}
			err = CompareEvidence(base.RequestJSON(), candidate.RecordJSON())
			assertErrorContains(t, err, "differs")
		})
	}
}

func TestCompareEvidenceRejectsWrongSelfDigest(t *testing.T) {
	adaptation := mustAdapt(t, validRequest())
	wrong := strings.Replace(
		string(adaptation.EvidenceJSON()), adaptation.Evidence.Digest(), strings.Repeat("0", 64), 1,
	)
	err := CompareEvidence(adaptation.RequestJSON(), []byte(wrong))
	assertErrorContains(t, err, "canonical_sha256 mismatch")
}
