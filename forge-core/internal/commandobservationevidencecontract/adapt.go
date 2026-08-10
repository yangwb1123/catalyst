package commandobservationevidencecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"

	"forgeos/forge-core/internal/governancecontract"
)

// AdaptCanonicalRequest strictly decodes exact request bytes and adapts them
// without process execution, persistence, authority, claims, atoms, or effects.
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

// CompareEvidence requires a structurally valid candidate to be byte-for-byte
// identical to deterministic re-adaptation of the canonical request.
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
		return fmt.Errorf("EvidenceRecord differs from deterministic command observation re-adaptation")
	}
	return nil
}

func adaptValidated(request Request, requestJSON []byte) (*Adaptation, error) {
	commandJSON, err := canonicalCommandJSON(request.Observation.Command)
	if err != nil {
		return nil, fmt.Errorf("canonical command: %w", err)
	}
	observationJSON, err := canonicalObservationJSON(request.Observation)
	if err != nil {
		return nil, fmt.Errorf("canonical observation: %w", err)
	}
	commandDigest := domainDigest(commandDigestDomain, commandJSON)
	sourceDigest := domainDigest(sourceDigestDomain, observationJSON)
	requestDigest := domainDigest(requestDigestDomain, requestJSON)
	evidence := buildEvidence(request, commandDigest, sourceDigest, requestDigest)
	verified, err := sealAndRevalidateEvidence(evidence)
	if err != nil {
		return nil, err
	}
	return &Adaptation{
		CanonicalCommandJSON:     append([]byte(nil), commandJSON...),
		CanonicalObservationJSON: append([]byte(nil), observationJSON...),
		CanonicalRequestJSON:     append([]byte(nil), requestJSON...),
		CommandSHA256:            commandDigest, Evidence: verified,
		RequestSHA256: requestDigest, Result: AdaptedShadow,
		SourceSnapshotSHA256: sourceDigest,
	}, nil
}

func sealAndRevalidateEvidence(evidence governancecontract.EvidenceRecord) (*governancecontract.Record, error) {
	if evidence.Integrity.CanonicalSHA256 != "" {
		return nil, fmt.Errorf("EvidenceRecord canonical_sha256 must be empty before sealing")
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
		return nil, fmt.Errorf("EvidenceRecord encoder did not terminate predictably")
	}
	return append([]byte(nil), encoded[:len(encoded)-1]...), nil
}

func buildEvidence(request Request, commandDigest, sourceDigest, requestDigest string) governancecontract.EvidenceRecord {
	observation := request.Observation
	artifactDigest := sourceDigest
	exitCode := *observation.Termination.ExitCode
	return governancecontract.EvidenceRecord{
		APIVersion: governancecontract.APIVersion,
		Integrity:  governancecontract.Integrity{Canonicalization: governancecontract.Canonicalization},
		Kind:       governancecontract.EvidenceKind,
		Metadata:   buildMetadata(request, requestDigest),
		Spec:       buildEvidenceSpec(request, commandDigest, sourceDigest, &artifactDigest, &exitCode),
		Status: governancecontract.Status{
			ReasonCodes: []string{}, State: "valid",
			ValidFromUnixMS: observation.EndedAtUnixMS, ValidUntilUnixMS: nil,
		},
	}
}

func buildMetadata(request Request, requestDigest string) governancecontract.Metadata {
	binding, observation := request.Binding, request.Observation
	return governancecontract.Metadata{
		AggregateID: binding.AggregateID, ContextSHA256: binding.ContextSHA256,
		CreatedAtUnixMS: observation.EndedAtUnixMS,
		CreatedBy: governancecontract.Principal{
			AuthorityDomain: "shadow", PrincipalID: adapterPrincipalID,
			PrincipalType: "tool", Role: adapterRole,
			RunID: "command-adaptation-" + requestDigest,
		},
		PolicySHA256: binding.PolicySHA256, ProjectID: binding.ProjectID,
		RecordID: "command-evidence-" + requestDigest, Scope: binding.Scope,
		Sequence: binding.Sequence, SourceRevision: observation.Source.SourceRevision,
		SourceTreeSHA256:    observation.Source.SourceTreeSHA256,
		SupersedesRecordIDs: cloneStrings(binding.SupersedesRecordIDs),
	}
}

func buildEvidenceSpec(request Request, commandDigest, sourceDigest string, artifactDigest *string, exitCode *int64) governancecontract.EvidenceSpec {
	binding, observation := request.Binding, request.Observation
	producer := observation.Producer
	return governancecontract.EvidenceSpec{
		ArtifactSHA256: artifactDigest,
		Collector: governancecontract.Collector{
			CollectorID: producer.ProducerID, CollectorType: producer.ProducerType,
			CollectorVersion: producer.ProducerVersion, ParametersSHA256: commandDigest,
			RunID: producer.RunID,
		},
		ContentRole: "untrusted_data", Directness: "direct",
		EvidenceType: observation.EvidenceType,
		Locator: governancecontract.Locator{
			// The combined hash is the producer-observed drain order. It does not
			// attest an operating-system emission order.
			ContentSHA256: observation.Streams.Combined.SHA256, ExitCode: exitCode,
			LineEnd: nil, LineStart: nil, LocatorRef: "command-observation:" + sourceDigest,
			LocatorType: "command",
		},
		ObservedAtUnixMS: observation.EndedAtUnixMS, Sensitivity: binding.Sensitivity,
		SourceSnapshot: governancecontract.SourceSnapshot{
			SnapshotID:     "command-observation-" + sourceDigest,
			SnapshotSHA256: sourceDigest, SnapshotType: "runtime",
		},
		SourceTrust: "observed", Subjects: cloneStrings(binding.Subjects),
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
