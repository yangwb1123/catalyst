package evolverepolocatorevidencecontract

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

func TestCompareEvidenceRejectsStructurallyValidProjectionDrift(t *testing.T) {
	base := mustAdapt(t, validRequest())
	tests := []struct {
		name string
		edit func(*governancecontract.EvidenceRecord)
	}{
		{"source trust", func(e *governancecontract.EvidenceRecord) { e.Spec.SourceTrust = "untrusted" }},
		{"collector parameters", func(e *governancecontract.EvidenceRecord) {
			e.Spec.Collector.ParametersSHA256 = strings.Repeat("0", 64)
		}},
		{"collector identity", func(e *governancecontract.EvidenceRecord) {
			e.Spec.Collector.CollectorID = "forgeos.other-evolve-scanner"
		}},
		{"synthetic run", func(e *governancecontract.EvidenceRecord) {
			e.Metadata.CreatedBy.RunID = "evolve-locator-adaptation-other"
		}},
		{"locator content", func(e *governancecontract.EvidenceRecord) {
			e.Spec.Locator.ContentSHA256 = strings.Repeat("1", 64)
		}},
		{"locator path", func(e *governancecontract.EvidenceRecord) { e.Spec.Locator.LocatorRef = "other/file" }},
		{"locator line", func(e *governancecontract.EvidenceRecord) {
			line := int64(115)
			e.Spec.Locator.LineStart = &line
			e.Spec.Locator.LineEnd = &line
		}},
		{"record id", func(e *governancecontract.EvidenceRecord) {
			e.Metadata.RecordID = "evolve-locator-evidence-other"
		}},
		{"state", func(e *governancecontract.EvidenceRecord) {
			e.Status.State = "invalid"
			e.Status.ReasonCodes = []string{"projection_drift"}
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

func TestCompareEvidenceRejectsNoncanonicalCandidate(t *testing.T) {
	adaptation := mustAdapt(t, validRequest())
	err := CompareEvidence(adaptation.RequestJSON(), append([]byte(" "), adaptation.EvidenceJSON()...))
	assertErrorContains(t, err, "not exact compact canonical")
}
