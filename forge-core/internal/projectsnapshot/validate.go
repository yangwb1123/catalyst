package projectsnapshot

import (
	"bytes"
	"fmt"
)

// Validate rechecks every nested self-seal, set seal, fixed semantic field,
// count relation, and the exact compact canonical production bytes.
func Validate(production *Production) error {
	if production == nil || len(production.encoded) == 0 || len(production.encoded) > maxEnvelopeBytes {
		return fmt.Errorf("project snapshot production is absent or outside bounds")
	}
	value := production.envelope
	if err := validateRequest(value.Request); err != nil {
		return err
	}
	if err := validateSnapshot(value.Snapshot, value.Request); err != nil {
		return err
	}
	if value.APIVersion != envelopeVersion || value.Canonicalization != canonicalization ||
		value.Kind != envelopeKind {
		return fmt.Errorf("project snapshot envelope fixed fields drifted")
	}
	expected := value
	expected.EnvelopeSHA256 = ""
	digest, err := domainDigest(envelopeDomain, expected, maxEnvelopeBytes)
	if err != nil || digest != value.EnvelopeSHA256 {
		return fmt.Errorf("project snapshot envelope digest mismatch")
	}
	canonical, err := canonicalJSON(value, maxEnvelopeBytes)
	if err != nil || !bytes.Equal(canonical, production.encoded) {
		return fmt.Errorf("project snapshot production is not exact canonical JSON")
	}
	return nil
}

func validateRequest(value Request) error {
	if value.APIVersion != requestVersion || value.Canonicalization != canonicalization ||
		value.ExtractorID != extractorID || value.ExtractorVersion != extractorVersion ||
		value.PathPolicyID != pathPolicyID || value.ProfileID != profileID {
		return fmt.Errorf("project snapshot request fixed fields drifted")
	}
	if err := validateIdentifier("project_id", value.ProjectID); err != nil {
		return err
	}
	if err := validateIdentifier("run_id", value.RunID); err != nil {
		return err
	}
	expected := value
	expected.RequestSHA256 = ""
	digest, err := domainDigest(requestDomain, expected, maxManifestBytes)
	if err != nil || digest != value.RequestSHA256 {
		return fmt.Errorf("project snapshot request digest mismatch")
	}
	return nil
}

func validateSnapshot(value Snapshot, request Request) error {
	if value.APIVersion != snapshotVersion || value.Atomic || value.AuthorityAttested ||
		value.Canonicalization != canonicalization || value.Consistency != consistencyValue ||
		value.Currentness != unknownValue || value.EffectAttested || value.Freshness != unknownValue ||
		value.Kind != snapshotKind || value.PermissionAttested || value.PersistenceAttested ||
		value.PositiveResult != positiveResult || value.ProfileID != profileID ||
		value.SystemCompleteness != unknownValue || value.TruthAttested {
		return fmt.Errorf("project source snapshot fixed semantics drifted")
	}
	if value.ProjectID != request.ProjectID || value.RunID != request.RunID ||
		value.RequestSHA256 != request.RequestSHA256 || value.Extractor.ExtractorID != extractorID ||
		value.Extractor.ExtractorVersion != extractorVersion {
		return fmt.Errorf("project source snapshot request binding drifted")
	}
	if err := validateManifest(value.SourceManifest); err != nil {
		return err
	}
	if value.SourceManifestSHA256 != value.SourceManifest.SourceManifestSHA256 {
		return fmt.Errorf("project source manifest binding mismatch")
	}
	if err := validateCoverage(value.Coverage, value.SourceManifest); err != nil {
		return err
	}
	if value.CoverageSHA256 != value.Coverage.CoverageSHA256 {
		return fmt.Errorf("project source coverage binding mismatch")
	}
	return validateSnapshotSeals(value)
}

func validateSnapshotSeals(value Snapshot) error {
	identity := snapshotIdentity{
		CoverageSHA256: value.CoverageSHA256, ExtractorID: value.Extractor.ExtractorID,
		ExtractorVersion: value.Extractor.ExtractorVersion, ProfileID: value.ProfileID,
		ProjectID: value.ProjectID, RequestSHA256: value.RequestSHA256, RunID: value.RunID,
		SourceManifestSHA256: value.SourceManifestSHA256,
	}
	digest, err := domainDigest(snapshotIDDomain, identity, maxManifestBytes)
	if err != nil || digest != value.SnapshotIdentitySHA256 ||
		value.SnapshotID != "project-snapshot-"+digest {
		return fmt.Errorf("project source snapshot identity mismatch")
	}
	expected := cloneSnapshot(value)
	expected.SnapshotSHA256 = ""
	digest, err = domainDigest(snapshotDomain, expected, maxEnvelopeBytes)
	if err != nil || digest != value.SnapshotSHA256 {
		return fmt.Errorf("project source snapshot record digest mismatch")
	}
	return nil
}
