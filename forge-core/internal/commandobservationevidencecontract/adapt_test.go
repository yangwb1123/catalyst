package commandobservationevidencecontract

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
	wantSyntheticRun := "command-adaptation-" + adaptation.RequestSHA256
	if metadata.RecordID != "command-evidence-"+adaptation.RequestSHA256 ||
		metadata.CreatedBy != (governancecontract.Principal{
			AuthorityDomain: "shadow", PrincipalID: adapterPrincipalID,
			PrincipalType: "tool", Role: adapterRole, RunID: wantSyntheticRun,
		}) || metadata.SourceRevision != request.Observation.Source.SourceRevision ||
		metadata.SourceTreeSHA256 != request.Observation.Source.SourceTreeSHA256 {
		t.Fatalf("metadata mapping drifted: %#v", metadata)
	}
}

func assertExactEvidenceSpec(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	spec := adaptation.Evidence.Evidence.Spec
	if spec.ArtifactSHA256 == nil || *spec.ArtifactSHA256 != adaptation.SourceSnapshotSHA256 ||
		spec.EvidenceType != "gate_result" || spec.Directness != "direct" ||
		spec.SourceTrust != "observed" || spec.ContentRole != "untrusted_data" {
		t.Fatalf("spec mapping drifted: %#v", spec)
	}
	wantCollector := governancecontract.Collector{
		CollectorID:      request.Observation.Producer.ProducerID,
		CollectorType:    request.Observation.Producer.ProducerType,
		CollectorVersion: request.Observation.Producer.ProducerVersion,
		ParametersSHA256: adaptation.CommandSHA256, RunID: request.Observation.Producer.RunID,
	}
	if spec.Collector != wantCollector {
		t.Fatalf("collector = %#v, want %#v", spec.Collector, wantCollector)
	}
	if spec.Locator.ContentSHA256 != request.Observation.Streams.Combined.SHA256 ||
		spec.Locator.ExitCode == nil || *spec.Locator.ExitCode != 0 ||
		spec.Locator.LocatorRef != "command-observation:"+adaptation.SourceSnapshotSHA256 ||
		spec.Locator.LocatorType != "command" || spec.Locator.LineStart != nil || spec.Locator.LineEnd != nil {
		t.Fatalf("locator mapping drifted: %#v", spec.Locator)
	}
	if spec.SourceSnapshot != (governancecontract.SourceSnapshot{
		SnapshotID:     "command-observation-" + adaptation.SourceSnapshotSHA256,
		SnapshotSHA256: adaptation.SourceSnapshotSHA256, SnapshotType: "runtime",
	}) {
		t.Fatalf("source snapshot drifted: %#v", spec.SourceSnapshot)
	}
}

func assertExactEvidenceStatus(t *testing.T, adaptation *Adaptation, request Request) {
	t.Helper()
	status := adaptation.Evidence.Evidence.Status
	if status.State != "valid" || len(status.ReasonCodes) != 0 ||
		status.ValidUntilUnixMS != nil || status.ValidFromUnixMS != request.Observation.EndedAtUnixMS {
		t.Fatalf("status mapping drifted: %#v", status)
	}
}

func TestTestRunAndNonzeroExitRemainUntrustedObservations(t *testing.T) {
	request := validRequest()
	request.Observation.EvidenceType = "test_run"
	request.Observation.Termination.ExitCode = int64Pointer(7)
	adaptation := mustAdapt(t, request)
	spec := adaptation.Evidence.Evidence.Spec
	if spec.EvidenceType != "test_run" || spec.Locator.ExitCode == nil || *spec.Locator.ExitCode != 7 ||
		adaptation.Evidence.Evidence.Status.State != "valid" {
		t.Fatalf("nonzero test observation mapping = %#v", adaptation.Evidence.Evidence)
	}
	for _, denied := range []string{"execution", "pass", "completion", "truth", "authority", "claim", "atom", "persistence", "effect"} {
		if !strings.Contains(AdaptedShadow, denied) {
			t.Fatalf("positive result omits non-capability %q", denied)
		}
	}
}

func TestAdaptCanonicalRequestMatchesTypedAdapt(t *testing.T) {
	typed := mustAdapt(t, validRequest())
	decoded, err := AdaptCanonicalRequest(typed.RequestJSON())
	if err != nil {
		t.Fatal(err)
	}
	if typed.CommandSHA256 != decoded.CommandSHA256 || typed.SourceSnapshotSHA256 != decoded.SourceSnapshotSHA256 ||
		typed.RequestSHA256 != decoded.RequestSHA256 || !bytes.Equal(typed.EvidenceJSON(), decoded.EvidenceJSON()) {
		t.Fatal("typed and canonical-byte adaptation differ")
	}
}

func TestIdentityDomainsRemainSeparate(t *testing.T) {
	base := mustAdapt(t, validRequest())
	bindingVariant := validRequest()
	bindingVariant.Binding.Scope = "module:harness"
	binding := mustAdapt(t, bindingVariant)
	if base.CommandSHA256 != binding.CommandSHA256 || base.SourceSnapshotSHA256 != binding.SourceSnapshotSHA256 ||
		base.RequestSHA256 == binding.RequestSHA256 {
		t.Fatal("binding mutation crossed identity boundaries")
	}
	observationVariant := validRequest()
	observationVariant.Observation.Producer.RunID = "run-command-0049-b"
	observation := mustAdapt(t, observationVariant)
	if base.CommandSHA256 != observation.CommandSHA256 || base.SourceSnapshotSHA256 == observation.SourceSnapshotSHA256 {
		t.Fatal("observation mutation crossed identity boundaries")
	}
	commandVariant := validRequest()
	commandVariant.Observation.Command.TimeoutMS = nil
	command := mustAdapt(t, commandVariant)
	if base.CommandSHA256 == command.CommandSHA256 || base.SourceSnapshotSHA256 == command.SourceSnapshotSHA256 {
		t.Fatal("command mutation failed to change command and observation identities")
	}
}

func TestAdaptationDefendsOutputFromTypedInputAliases(t *testing.T) {
	request := validRequest()
	adaptation := mustAdapt(t, request)
	wantEvidence := string(adaptation.EvidenceJSON())
	request.Binding.Subjects[0] = "mutated"
	request.Binding.SupersedesRecordIDs = append(request.Binding.SupersedesRecordIDs, "mutated")
	*request.Observation.Termination.ExitCode = 99
	*request.Observation.Command.TimeoutMS = 1
	requestJSON := adaptation.RequestJSON()
	requestJSON[0] = '['
	observationJSON := adaptation.ObservationJSON()
	observationJSON[0] = '['
	commandJSON := adaptation.CommandJSON()
	commandJSON[0] = '['
	if got := string(adaptation.EvidenceJSON()); got != wantEvidence ||
		adaptation.RequestJSON()[0] != '{' || adaptation.ObservationJSON()[0] != '{' ||
		adaptation.CommandJSON()[0] != '{' {
		t.Fatal("adaptation aliases typed input or returned byte slices")
	}
}

func TestEvidenceCarriesNoClaimAtomOrEffectAttestation(t *testing.T) {
	evidenceJSON := string(mustAdapt(t, validRequest()).EvidenceJSON())
	for _, forbidden := range []string{"KnowledgeClaim", "CognitiveAtom", "authority_ref", "effect_attestation"} {
		if strings.Contains(evidenceJSON, forbidden) {
			t.Fatalf("evidence contains forbidden capability %q", forbidden)
		}
	}
}
