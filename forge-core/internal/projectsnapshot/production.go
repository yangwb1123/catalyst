package projectsnapshot

import "fmt"

type snapshotIdentity struct {
	CoverageSHA256       string `json:"coverage_sha256"`
	ExtractorID          string `json:"extractor_id"`
	ExtractorVersion     string `json:"extractor_version"`
	ProfileID            string `json:"profile_id"`
	ProjectID            string `json:"project_id"`
	RequestSHA256        string `json:"request_sha256"`
	RunID                string `json:"run_id"`
	SourceManifestSHA256 string `json:"source_manifest_sha256"`
}

func buildProduction(
	request Request,
	manifest SourceManifest,
	counts CoverageCounts,
) (*Production, error) {
	coverage, err := buildCoverage(manifest.SourceManifestSHA256, counts)
	if err != nil {
		return nil, err
	}
	snapshot, err := buildSnapshot(request, manifest, coverage)
	if err != nil {
		return nil, err
	}
	envelope := Envelope{
		APIVersion: envelopeVersion, Canonicalization: canonicalization,
		Kind: envelopeKind, Request: request, Snapshot: snapshot,
	}
	envelope.EnvelopeSHA256, err = domainDigest(envelopeDomain, envelope, maxEnvelopeBytes)
	if err != nil {
		return nil, err
	}
	encoded, err := canonicalJSON(envelope, maxEnvelopeBytes)
	if err != nil {
		return nil, err
	}
	production := &Production{envelope: cloneEnvelope(envelope), encoded: encoded}
	if err := Validate(production); err != nil {
		return nil, fmt.Errorf("validate captured project snapshot: %w", err)
	}
	return production, nil
}

func buildSnapshot(
	request Request,
	manifest SourceManifest,
	coverage Coverage,
) (Snapshot, error) {
	value := Snapshot{
		APIVersion: snapshotVersion, Atomic: false, AuthorityAttested: false,
		Canonicalization: canonicalization, Consistency: consistencyValue,
		Coverage: coverage, CoverageSHA256: coverage.CoverageSHA256,
		Currentness: unknownValue, EffectAttested: false,
		Extractor: Extractor{ExtractorID: extractorID, ExtractorVersion: extractorVersion},
		Freshness: unknownValue, Kind: snapshotKind, PermissionAttested: false,
		PersistenceAttested: false, PositiveResult: positiveResult, ProfileID: profileID,
		ProjectID: request.ProjectID, RequestSHA256: request.RequestSHA256, RunID: request.RunID,
		SourceManifest: cloneManifest(manifest), SourceManifestSHA256: manifest.SourceManifestSHA256,
		SystemCompleteness: unknownValue, TruthAttested: false,
	}
	identity := snapshotIdentity{
		CoverageSHA256: coverage.CoverageSHA256, ExtractorID: extractorID,
		ExtractorVersion: extractorVersion, ProfileID: profileID, ProjectID: request.ProjectID,
		RequestSHA256: request.RequestSHA256, RunID: request.RunID,
		SourceManifestSHA256: manifest.SourceManifestSHA256,
	}
	digest, err := domainDigest(snapshotIDDomain, identity, maxManifestBytes)
	if err != nil {
		return Snapshot{}, err
	}
	value.SnapshotIdentitySHA256 = digest
	value.SnapshotID = "project-snapshot-" + digest
	value.SnapshotSHA256, err = domainDigest(snapshotDomain, value, maxEnvelopeBytes)
	return value, err
}
