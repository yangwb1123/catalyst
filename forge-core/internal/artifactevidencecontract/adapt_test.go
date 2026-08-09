package artifactevidencecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/governancecontract"
)

func TestAdaptProducesExactShadowEvidenceMapping(t *testing.T) {
	request := validRequest()
	adaptation, err := Adapt(request)
	if err != nil {
		t.Fatalf("Adapt: %v", err)
	}
	assertAdaptationEnvelope(t, adaptation)
	assertEvidenceMetadata(t, adaptation, request)
	assertEvidenceSpec(t, adaptation, request)
	assertEvidenceStatus(t, adaptation)
	if _, err := governancecontract.DecodeRecord(adaptation.EvidenceJSON()); err != nil {
		t.Fatalf("governance revalidation: %v", err)
	}
}

func assertAdaptationEnvelope(t *testing.T, adaptation *Adaptation) {
	t.Helper()
	if adaptation.Result != AdaptedShadow {
		t.Fatalf("result = %q", adaptation.Result)
	}
	wantRequest := manualDomainDigest(requestDigestDomain, adaptation.RequestJSON())
	if adaptation.RequestSHA256 != wantRequest {
		t.Fatalf("request digest = %q, want %q", adaptation.RequestSHA256, wantRequest)
	}
	request, err := DecodeRequest(adaptation.RequestJSON())
	if err != nil {
		t.Fatalf("DecodeRequest(adaptation): %v", err)
	}
	sourceJSON, err := canonicalJSON(artifactNode(request.Artifact))
	if err != nil {
		t.Fatal(err)
	}
	wantSource := manualDomainDigest(sourceDigestDomain, sourceJSON)
	if adaptation.SourceSHA256 != wantSource {
		t.Fatalf("source digest = %q, want %q", adaptation.SourceSHA256, wantSource)
	}
}

func assertEvidenceMetadata(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	evidence := adaptation.Evidence.Evidence
	wantTime := mustUnixMilliseconds(t, request.Artifact.CreatedAt)
	metadata := evidence.Metadata
	if metadata.RecordID != "artifact-evidence-"+adaptation.RequestSHA256 ||
		metadata.CreatedAtUnixMS != wantTime || metadata.AggregateID != request.Binding.AggregateID ||
		metadata.ContextSHA256 != request.Binding.ContextSHA256 ||
		metadata.PolicySHA256 != request.Binding.PolicySHA256 || metadata.ProjectID != request.Binding.ProjectID ||
		metadata.Scope != request.Binding.Scope || metadata.Sequence != request.Binding.Sequence ||
		metadata.SourceRevision != request.Binding.SourceRevision ||
		metadata.SourceTreeSHA256 != request.Binding.SourceTreeSHA256 {
		t.Fatalf("metadata mapping drifted: %#v", metadata)
	}
	if metadata.CreatedBy != expectedPrincipal(request.Artifact.RunID) {
		t.Fatalf("created_by = %#v", metadata.CreatedBy)
	}
}

func assertEvidenceSpec(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	spec := adaptation.Evidence.Evidence.Spec
	if spec.ArtifactSHA256 == nil || *spec.ArtifactSHA256 != request.Artifact.SHA256 ||
		spec.EvidenceType != "artifact" || spec.Directness != "direct" ||
		spec.SourceTrust != "observed" || spec.ContentRole != "untrusted_data" ||
		spec.Sensitivity != request.Binding.Sensitivity ||
		spec.ObservedAtUnixMS != mustUnixMilliseconds(t, request.Artifact.CreatedAt) {
		t.Fatalf("evidence spec mapping drifted: %#v", spec)
	}
	if spec.Locator.ContentSHA256 != request.Artifact.SHA256 ||
		spec.Locator.LocatorRef != request.Artifact.Path || spec.Locator.LocatorType != "artifact" ||
		spec.Locator.ExitCode != nil || spec.Locator.LineStart != nil || spec.Locator.LineEnd != nil {
		t.Fatalf("locator mapping drifted: %#v", spec.Locator)
	}
	assertCollectorAndSnapshot(t, adaptation, request)
}

func assertCollectorAndSnapshot(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	spec := adaptation.Evidence.Evidence.Spec
	wantCollector := governancecontract.Collector{
		CollectorID: adapterPrincipalID, CollectorType: "tool", CollectorVersion: adapterVersion,
		ParametersSHA256: adaptation.RequestSHA256, RunID: request.Artifact.RunID,
	}
	if spec.Collector != wantCollector {
		t.Fatalf("collector = %#v, want %#v", spec.Collector, wantCollector)
	}
	if spec.SourceSnapshot.SnapshotID != "artifact-snapshot-"+adaptation.SourceSHA256 ||
		spec.SourceSnapshot.SnapshotSHA256 != adaptation.SourceSHA256 ||
		spec.SourceSnapshot.SnapshotType != "artifact" {
		t.Fatalf("source snapshot = %#v", spec.SourceSnapshot)
	}
}

func assertEvidenceStatus(t *testing.T, adaptation *Adaptation) {
	t.Helper()
	evidence := adaptation.Evidence.Evidence
	if evidence.APIVersion != governancecontract.APIVersion || evidence.Kind != governancecontract.EvidenceKind ||
		evidence.Integrity.Canonicalization != governancecontract.Canonicalization {
		t.Fatalf("governance envelope drifted: %#v", evidence)
	}
	if evidence.Status.State != "valid" || len(evidence.Status.ReasonCodes) != 0 ||
		evidence.Status.ValidUntilUnixMS != nil ||
		evidence.Status.ValidFromUnixMS != evidence.Spec.ObservedAtUnixMS {
		t.Fatalf("status mapping drifted: %#v", evidence.Status)
	}
}

func TestAdaptCanonicalRequestMatchesTypedAdapt(t *testing.T) {
	typed, err := Adapt(validRequest())
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := AdaptCanonicalRequest(typed.RequestJSON())
	if err != nil {
		t.Fatal(err)
	}
	if typed.RequestSHA256 != decoded.RequestSHA256 || typed.SourceSHA256 != decoded.SourceSHA256 ||
		!bytes.Equal(typed.EvidenceJSON(), decoded.EvidenceJSON()) {
		t.Fatal("typed and canonical-byte adaptation differ")
	}
}

func TestAdaptIdentitySeparatesSourceAndBinding(t *testing.T) {
	base := validRequest()
	first, err := Adapt(base)
	if err != nil {
		t.Fatal(err)
	}
	bindingVariant := base
	bindingVariant.Binding.ProjectID = "catalyst-next"
	second, err := Adapt(bindingVariant)
	if err != nil {
		t.Fatal(err)
	}
	if first.SourceSHA256 != second.SourceSHA256 || first.RequestSHA256 == second.RequestSHA256 {
		t.Fatal("binding change did not preserve source identity and change request identity")
	}
	artifactVariant := base
	artifactVariant.Artifact.Size++
	third, err := Adapt(artifactVariant)
	if err != nil {
		t.Fatal(err)
	}
	if first.SourceSHA256 == third.SourceSHA256 || first.RequestSHA256 == third.RequestSHA256 {
		t.Fatal("artifact change did not alter source and request identities")
	}
}

func TestSourceIdentityPreservesSubMillisecondTimestamp(t *testing.T) {
	firstRequest := validRequest()
	secondRequest := validRequest()
	secondRequest.Artifact.CreatedAt = "2026-08-10T12:34:56.987999999+06:30"
	first, err := Adapt(firstRequest)
	if err != nil {
		t.Fatal(err)
	}
	second, err := Adapt(secondRequest)
	if err != nil {
		t.Fatal(err)
	}
	if first.Evidence.Evidence.Metadata.CreatedAtUnixMS != second.Evidence.Evidence.Metadata.CreatedAtUnixMS {
		t.Fatal("timestamps expected to floor to the same Unix millisecond")
	}
	if first.SourceSHA256 == second.SourceSHA256 || first.RequestSHA256 == second.RequestSHA256 {
		t.Fatal("original timestamp spelling must remain identity-bearing")
	}
}

func TestAdaptationDefendsSealedEvidenceFromInputAliases(t *testing.T) {
	request := validRequest()
	adaptation, err := Adapt(request)
	if err != nil {
		t.Fatal(err)
	}
	want := string(adaptation.EvidenceJSON())
	request.Binding.Subjects[0] = "mutated"
	request.Binding.SupersedesRecordIDs = append(request.Binding.SupersedesRecordIDs, "mutated")
	requestJSON := adaptation.RequestJSON()
	requestJSON[0] = '['
	sourceJSON := adaptation.SourceJSON()
	sourceJSON[0] = '['
	if got := string(adaptation.EvidenceJSON()); got != want ||
		adaptation.RequestJSON()[0] != '{' || adaptation.SourceJSON()[0] != '{' {
		t.Fatal("adaptation aliases caller or returned byte slices")
	}
}

func TestEvidenceCarriesNoClaimAtomOrEffectAttestation(t *testing.T) {
	adaptation, err := Adapt(validRequest())
	if err != nil {
		t.Fatal(err)
	}
	evidenceJSON := string(adaptation.EvidenceJSON())
	for _, forbidden := range []string{"KnowledgeClaim", "CognitiveAtom", "authority_ref", "effect_attestation"} {
		if strings.Contains(evidenceJSON, forbidden) {
			t.Fatalf("evidence contains forbidden capability %q", forbidden)
		}
	}
}

func expectedPrincipal(runID string) governancecontract.Principal {
	return governancecontract.Principal{
		AuthorityDomain: "shadow", PrincipalID: adapterPrincipalID,
		PrincipalType: "tool", Role: adapterRole, RunID: runID,
	}
}

func mustUnixMilliseconds(t *testing.T, value string) int64 {
	t.Helper()
	parsed, err := time.Parse(time.RFC3339Nano, value)
	if err != nil {
		t.Fatal(err)
	}
	return parsed.UnixMilli()
}

func manualDomainDigest(domain string, payload []byte) string {
	hasher := sha256.New()
	hasher.Write([]byte(domain))
	hasher.Write([]byte{0})
	hasher.Write(payload)
	return hex.EncodeToString(hasher.Sum(nil))
}
