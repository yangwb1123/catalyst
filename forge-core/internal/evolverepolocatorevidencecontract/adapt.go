package evolverepolocatorevidencecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"

	"forgeos/forge-core/internal/governancecontract"
)

// AdaptCanonicalRequest strictly decodes exact request bytes and adapts them
// without file or report reads, persistence, authority, claims, atoms, or
// effects.
func AdaptCanonicalRequest(data []byte) (*Adaptation, error) {
	request, err := DecodeRequest(data)
	if err != nil {
		return nil, err
	}
	return adaptValidated(*request, append([]byte(nil), data...))
}

// Adapt validates and canonically encodes one typed request before adapting it
// to a revalidated shadow EvidenceRecord.
func Adapt(request Request) (*Adaptation, error) {
	requestJSON, err := canonicalRequestJSON(request)
	if err != nil {
		return nil, err
	}
	return adaptValidated(request, requestJSON)
}

// CompareEvidence strictly revalidates candidate bytes and requires exact
// equality with deterministic Evolve locator re-adaptation.
func CompareEvidence(requestJSON, candidateEvidenceJSON []byte) error {
	candidate, err := governancecontract.DecodeRecord(candidateEvidenceJSON)
	if err != nil {
		return fmt.Errorf("candidate Governance EvidenceRecord: %w", err)
	}
	if candidate.Evidence == nil {
		return fmt.Errorf("candidate governance record must be an EvidenceRecord")
	}
	expected, err := AdaptCanonicalRequest(requestJSON)
	if err != nil {
		return err
	}
	if !bytes.Equal(candidate.RecordJSON(), expected.EvidenceJSON()) {
		return fmt.Errorf("evidence record differs from deterministic Evolve repository locator re-adaptation")
	}
	return nil
}

func adaptValidated(request Request, requestJSON []byte) (*Adaptation, error) {
	locatorJSON, err := canonicalLocatorJSON(request.Observation.Locator)
	if err != nil {
		return nil, fmt.Errorf("canonical locator: %w", err)
	}
	observationJSON, err := canonicalObservationJSON(request.Observation)
	if err != nil {
		return nil, fmt.Errorf("canonical observation: %w", err)
	}
	locatorDigest := domainDigest(locatorDigestDomain, locatorJSON)
	sourceDigest := domainDigest(sourceDigestDomain, observationJSON)
	requestDigest := domainDigest(requestDigestDomain, requestJSON)
	evidence := buildEvidence(request, sourceDigest, requestDigest)
	verified, err := sealAndRevalidateEvidence(evidence)
	if err != nil {
		return nil, err
	}
	return &Adaptation{
		CanonicalLocatorJSON:     append([]byte(nil), locatorJSON...),
		CanonicalObservationJSON: append([]byte(nil), observationJSON...),
		CanonicalRequestJSON:     append([]byte(nil), requestJSON...),
		Evidence:                 verified,
		LocatorSHA256:            locatorDigest,
		RequestSHA256:            requestDigest,
		Result:                   AdaptedShadow,
		SourceSnapshotSHA256:     sourceDigest,
	}, nil
}

func sealAndRevalidateEvidence(evidence governancecontract.EvidenceRecord) (*governancecontract.Record, error) {
	if evidence.Integrity.CanonicalSHA256 != "" {
		return nil, fmt.Errorf("evidence record canonical_sha256 must be empty before sealing")
	}
	payload, err := encodeEvidence(evidence)
	if err != nil {
		return nil, fmt.Errorf("encode Governance EvidenceRecord payload: %w", err)
	}
	evidence.Integrity.CanonicalSHA256 = domainDigest(evidenceDigestDomain, payload)
	recordJSON, err := encodeEvidence(evidence)
	if err != nil {
		return nil, fmt.Errorf("encode Governance EvidenceRecord: %w", err)
	}
	verified, err := governancecontract.DecodeRecord(recordJSON)
	if err != nil {
		return nil, fmt.Errorf("revalidate Governance EvidenceRecord: %w", err)
	}
	return verified, nil
}

func encodeEvidence(evidence governancecontract.EvidenceRecord) ([]byte, error) {
	buffer := bytes.NewBuffer(nil)
	encoder := json.NewEncoder(buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(evidence); err != nil {
		return nil, err
	}
	encoded := buffer.Bytes()
	if len(encoded) == 0 || encoded[len(encoded)-1] != '\n' {
		return nil, fmt.Errorf("evidence record encoder did not terminate predictably")
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func buildEvidence(request Request, sourceDigest, requestDigest string) governancecontract.EvidenceRecord {
	observation := request.Observation
	artifactDigest := observation.Content.SHA256
	return governancecontract.EvidenceRecord{
		APIVersion: governancecontract.APIVersion,
		Integrity: governancecontract.Integrity{
			Canonicalization: governancecontract.Canonicalization,
		},
		Kind:     governancecontract.EvidenceKind,
		Metadata: buildMetadata(request, requestDigest),
		Spec:     buildEvidenceSpec(request, sourceDigest, &artifactDigest),
		Status: governancecontract.Status{
			ReasonCodes:      []string{},
			State:            "valid",
			ValidFromUnixMS:  observation.ObservedAtUnixMS,
			ValidUntilUnixMS: nil,
		},
	}
}

func buildMetadata(request Request, requestDigest string) governancecontract.Metadata {
	binding, observation := request.Binding, request.Observation
	return governancecontract.Metadata{
		AggregateID:     binding.AggregateID,
		ContextSHA256:   binding.ContextSHA256,
		CreatedAtUnixMS: observation.ObservedAtUnixMS,
		CreatedBy: governancecontract.Principal{
			AuthorityDomain: "shadow",
			PrincipalID:     adapterPrincipalID,
			PrincipalType:   "tool",
			Role:            adapterRole,
			RunID:           "evolve-locator-adaptation-" + requestDigest,
		},
		PolicySHA256:        binding.PolicySHA256,
		ProjectID:           binding.ProjectID,
		RecordID:            "evolve-locator-evidence-" + requestDigest,
		Scope:               binding.Scope,
		Sequence:            binding.Sequence,
		SourceRevision:      observation.Source.SourceRevision,
		SourceTreeSHA256:    observation.Source.SourceTreeSHA256,
		SupersedesRecordIDs: cloneStrings(binding.SupersedesRecordIDs),
	}
}

func buildEvidenceSpec(request Request, sourceDigest string, artifactDigest *string) governancecontract.EvidenceSpec {
	binding, observation := request.Binding, request.Observation
	producer, locator := observation.Producer, observation.Locator
	var line *int64
	if locator.Line > 0 {
		lineValue := locator.Line
		line = &lineValue
	}
	return governancecontract.EvidenceSpec{
		ArtifactSHA256: artifactDigest,
		Collector: governancecontract.Collector{
			CollectorID:      producer.ProducerID,
			CollectorType:    producer.ProducerType,
			CollectorVersion: producer.ProducerVersion,
			ParametersSHA256: producer.ParametersSHA256,
			RunID:            producer.RunID,
		},
		ContentRole:  "untrusted_data",
		Directness:   "direct",
		EvidenceType: "repo_locator",
		Locator: governancecontract.Locator{
			ContentSHA256: observation.Content.SHA256,
			ExitCode:      nil,
			LineEnd:       line,
			LineStart:     line,
			LocatorRef:    locator.Path,
			LocatorType:   "repo",
		},
		ObservedAtUnixMS: observation.ObservedAtUnixMS,
		Sensitivity:      binding.Sensitivity,
		SourceSnapshot: governancecontract.SourceSnapshot{
			SnapshotID:     "evolve-locator-" + sourceDigest,
			SnapshotSHA256: sourceDigest,
			SnapshotType:   "repository",
		},
		SourceTrust: "observed",
		Subjects:    cloneStrings(binding.Subjects),
	}
}

func domainDigest(domain string, payload []byte) string {
	hasher := sha256.New()
	hasher.Write([]byte(domain))
	hasher.Write([]byte{0})
	hasher.Write(payload)
	return hex.EncodeToString(hasher.Sum(nil))
}

func cloneStrings(values []string) []string {
	result := make([]string, len(values))
	copy(result, values)
	return result
}
