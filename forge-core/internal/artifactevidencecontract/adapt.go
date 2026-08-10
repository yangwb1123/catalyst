package artifactevidencecontract

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"

	"forgeos/forge-core/internal/governancecontract"
)

// AdaptCanonicalRequest strictly decodes exact request bytes and adapts them
// without persistence, authority, claims, atoms, or external effects.
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
// equality with deterministic re-adaptation of the canonical request.
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
		return fmt.Errorf("EvidenceRecord differs from deterministic artifact re-adaptation")
	}
	return nil
}

func adaptValidated(request Request, requestJSON []byte) (*Adaptation, error) {
	sourceJSON, err := canonicalJSON(artifactNode(request.Artifact))
	if err != nil {
		return nil, fmt.Errorf("canonical artifact source: %w", err)
	}
	sourceDigest := domainDigest(sourceDigestDomain, sourceJSON)
	requestDigest := domainDigest(requestDigestDomain, requestJSON)
	milliseconds, err := artifactUnixMillis(request.Artifact.CreatedAt)
	if err != nil {
		return nil, err
	}
	evidence := buildEvidence(request, sourceDigest, requestDigest, milliseconds)
	verified, err := sealAndRevalidateEvidence(evidence)
	if err != nil {
		return nil, err
	}
	return &Adaptation{
		CanonicalRequestJSON: append([]byte(nil), requestJSON...),
		CanonicalSourceJSON:  append([]byte(nil), sourceJSON...), Evidence: verified,
		RequestSHA256: requestDigest, Result: AdaptedShadow, SourceSHA256: sourceDigest,
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

func buildEvidence(request Request, sourceDigest, requestDigest string, milliseconds int64) governancecontract.EvidenceRecord {
	artifactDigest := request.Artifact.SHA256
	return governancecontract.EvidenceRecord{
		APIVersion: governancecontract.APIVersion,
		Integrity: governancecontract.Integrity{
			Canonicalization: governancecontract.Canonicalization,
		},
		Kind:     governancecontract.EvidenceKind,
		Metadata: buildMetadata(request, requestDigest, milliseconds),
		Spec:     buildEvidenceSpec(request, sourceDigest, requestDigest, milliseconds, &artifactDigest),
		Status: governancecontract.Status{
			ReasonCodes: []string{}, State: "valid", ValidFromUnixMS: milliseconds,
			ValidUntilUnixMS: nil,
		},
	}
}

func buildMetadata(request Request, requestDigest string, milliseconds int64) governancecontract.Metadata {
	binding := request.Binding
	return governancecontract.Metadata{
		AggregateID: binding.AggregateID, ContextSHA256: binding.ContextSHA256,
		CreatedAtUnixMS: milliseconds,
		CreatedBy: governancecontract.Principal{
			AuthorityDomain: "shadow", PrincipalID: adapterPrincipalID,
			PrincipalType: "tool", Role: adapterRole, RunID: request.Artifact.RunID,
		},
		PolicySHA256: binding.PolicySHA256, ProjectID: binding.ProjectID,
		RecordID: "artifact-evidence-" + requestDigest, Scope: binding.Scope,
		Sequence: binding.Sequence, SourceRevision: binding.SourceRevision,
		SourceTreeSHA256:    binding.SourceTreeSHA256,
		SupersedesRecordIDs: cloneStrings(binding.SupersedesRecordIDs),
	}
}

func buildEvidenceSpec(request Request, sourceDigest, requestDigest string, milliseconds int64, artifactDigest *string) governancecontract.EvidenceSpec {
	return governancecontract.EvidenceSpec{
		ArtifactSHA256: artifactDigest,
		Collector: governancecontract.Collector{
			CollectorID: adapterPrincipalID, CollectorType: "tool",
			CollectorVersion: adapterVersion, ParametersSHA256: requestDigest,
			RunID: request.Artifact.RunID,
		},
		ContentRole: "untrusted_data", Directness: "direct", EvidenceType: "artifact",
		Locator: governancecontract.Locator{
			ContentSHA256: request.Artifact.SHA256, ExitCode: nil, LineEnd: nil,
			LineStart: nil, LocatorRef: request.Artifact.Path, LocatorType: "artifact",
		},
		ObservedAtUnixMS: milliseconds, Sensitivity: request.Binding.Sensitivity,
		SourceSnapshot: governancecontract.SourceSnapshot{
			SnapshotID:     "artifact-snapshot-" + sourceDigest,
			SnapshotSHA256: sourceDigest, SnapshotType: "artifact",
		},
		SourceTrust: "observed", Subjects: cloneStrings(request.Binding.Subjects),
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
