package evolverepolocatorevidencecontract

import (
	"bytes"
	"strings"
	"testing"

	"forgeos/forge-core/internal/governancecontract"
)

func TestAdaptProducesExactShadowEvidenceMapping(t *testing.T) {
	request := validRequest()
	adaptation := mustAdapt(t, request)
	evidence := adaptation.Evidence.Evidence
	if adaptation.Result != AdaptedShadow || evidence == nil {
		t.Fatalf("adaptation envelope = %#v", adaptation)
	}
	assertExactEvidenceMetadata(t, adaptation, request)
	assertExactEvidenceSpec(t, adaptation, request)
	assertExactEvidenceStatus(t, adaptation, request)
	if _, err := governancecontract.DecodeRecord(adaptation.EvidenceJSON()); err != nil {
		t.Fatalf("governance revalidation: %v", err)
	}
}

func assertExactEvidenceMetadata(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	metadata := adaptation.Evidence.Evidence.Metadata
	wantPrincipal := governancecontract.Principal{
		AuthorityDomain: "shadow", PrincipalID: adapterPrincipalID,
		PrincipalType: "tool", Role: adapterRole,
		RunID: "evolve-locator-adaptation-" + adaptation.RequestSHA256,
	}
	if metadata.AggregateID != request.Binding.AggregateID ||
		metadata.ContextSHA256 != request.Binding.ContextSHA256 ||
		metadata.CreatedAtUnixMS != request.Observation.ObservedAtUnixMS ||
		metadata.CreatedBy != wantPrincipal ||
		metadata.PolicySHA256 != request.Binding.PolicySHA256 ||
		metadata.ProjectID != request.Binding.ProjectID ||
		metadata.RecordID != "evolve-locator-evidence-"+adaptation.RequestSHA256 ||
		metadata.Scope != request.Binding.Scope || metadata.Sequence != request.Binding.Sequence ||
		metadata.SourceRevision != request.Observation.Source.SourceRevision ||
		metadata.SourceTreeSHA256 != request.Observation.Source.SourceTreeSHA256 ||
		!equalStrings(metadata.SupersedesRecordIDs, request.Binding.SupersedesRecordIDs) {
		t.Fatalf("metadata mapping drifted: %#v", metadata)
	}
}

func assertExactEvidenceSpec(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	spec := adaptation.Evidence.Evidence.Spec
	if spec.ArtifactSHA256 == nil || *spec.ArtifactSHA256 != request.Observation.Content.SHA256 ||
		spec.EvidenceType != "repo_locator" || spec.Directness != "direct" ||
		spec.SourceTrust != "observed" || spec.ContentRole != "untrusted_data" ||
		spec.ObservedAtUnixMS != request.Observation.ObservedAtUnixMS ||
		spec.Sensitivity != request.Binding.Sensitivity ||
		!equalStrings(spec.Subjects, request.Binding.Subjects) {
		t.Fatalf("spec mapping drifted: %#v", spec)
	}
	wantCollector := governancecontract.Collector{
		CollectorID:      request.Observation.Producer.ProducerID,
		CollectorType:    request.Observation.Producer.ProducerType,
		CollectorVersion: request.Observation.Producer.ProducerVersion,
		ParametersSHA256: request.Observation.Producer.ParametersSHA256,
		RunID:            request.Observation.Producer.RunID,
	}
	if spec.Collector != wantCollector {
		t.Fatalf("collector = %#v, want %#v", spec.Collector, wantCollector)
	}
	if spec.Locator.ContentSHA256 != request.Observation.Content.SHA256 ||
		spec.Locator.ExitCode != nil || spec.Locator.LineStart == nil || spec.Locator.LineEnd == nil ||
		*spec.Locator.LineStart != request.Observation.Locator.Line ||
		*spec.Locator.LineEnd != request.Observation.Locator.Line ||
		spec.Locator.LocatorRef != request.Observation.Locator.Path || spec.Locator.LocatorType != "repo" {
		t.Fatalf("locator mapping drifted: %#v", spec.Locator)
	}
	wantSnapshot := governancecontract.SourceSnapshot{
		SnapshotID:     "evolve-locator-" + adaptation.SourceSnapshotSHA256,
		SnapshotSHA256: adaptation.SourceSnapshotSHA256, SnapshotType: "repository",
	}
	if spec.SourceSnapshot != wantSnapshot {
		t.Fatalf("source snapshot = %#v, want %#v", spec.SourceSnapshot, wantSnapshot)
	}
}

func assertExactEvidenceStatus(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	status := adaptation.Evidence.Evidence.Status
	if status.State != "valid" || len(status.ReasonCodes) != 0 ||
		status.ValidUntilUnixMS != nil || status.ValidFromUnixMS != request.Observation.ObservedAtUnixMS {
		t.Fatalf("status mapping drifted: %#v", status)
	}
}

func TestZeroLocatorLineMapsToNullLineRange(t *testing.T) {
	request := validRequest()
	request.Observation.Locator.Line = 0
	locator := mustAdapt(t, request).Evidence.Evidence.Spec.Locator
	if locator.LineStart != nil || locator.LineEnd != nil {
		t.Fatalf("line zero mapped to non-null range: %#v", locator)
	}
}

func TestAdaptCanonicalRequestMatchesTypedAdapt(t *testing.T) {
	typed := mustAdapt(t, validRequest())
	decoded, err := AdaptCanonicalRequest(typed.RequestJSON())
	if err != nil {
		t.Fatal(err)
	}
	if typed.LocatorSHA256 != decoded.LocatorSHA256 ||
		typed.SourceSnapshotSHA256 != decoded.SourceSnapshotSHA256 ||
		typed.RequestSHA256 != decoded.RequestSHA256 ||
		!bytes.Equal(typed.EvidenceJSON(), decoded.EvidenceJSON()) {
		t.Fatal("typed and canonical-byte adaptation differ")
	}
}

func TestAdaptationDefendsOutputFromTypedInputAliases(t *testing.T) {
	request := validRequest()
	adaptation := mustAdapt(t, request)
	wantEvidence := string(adaptation.EvidenceJSON())
	request.Binding.Subjects[0] = "mutated"
	request.Binding.SupersedesRecordIDs = append(request.Binding.SupersedesRecordIDs, "mutated")
	*request.Observation.ScanContext.OpportunityID = "mutated"
	for _, output := range [][]byte{adaptation.RequestJSON(), adaptation.ObservationJSON(), adaptation.LocatorJSON()} {
		output[0] = '['
	}
	if got := string(adaptation.EvidenceJSON()); got != wantEvidence ||
		adaptation.RequestJSON()[0] != '{' || adaptation.ObservationJSON()[0] != '{' ||
		adaptation.LocatorJSON()[0] != '{' {
		t.Fatal("adaptation aliases typed input or returned byte slices")
	}
}

func TestEvidenceCarriesOnlyLocatorMappingCapability(t *testing.T) {
	adaptation := mustAdapt(t, validRequest())
	evidenceJSON := string(adaptation.EvidenceJSON())
	for _, forbidden := range []string{
		"KnowledgeClaim", "CognitiveAtom", "authority_ref", "effect_attestation",
		"architecture-budget-0050", "report_sha256", "scan_context",
	} {
		if strings.Contains(evidenceJSON, forbidden) {
			t.Fatalf("evidence contains forbidden or nonprojected capability %q", forbidden)
		}
	}
	for _, denied := range []string{"file/report verification", "scan judgment", "completion", "truth", "authority", "claim", "atom", "persistence", "effect"} {
		if !strings.Contains(AdaptedShadow, denied) {
			t.Fatalf("positive result omits non-capability %q", denied)
		}
	}
}

func equalStrings(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}
