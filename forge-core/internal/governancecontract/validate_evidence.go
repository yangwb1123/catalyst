package governancecontract

import "fmt"

var locatorByEvidenceType = map[string]string{
	"repo_locator":      "repo",
	"test_run":          "command",
	"gate_result":       "command",
	"runtime_metric":    "metric",
	"external_source":   "external",
	"human_attestation": "attestation",
	"artifact":          "artifact",
}

func validateEvidence(record *EvidenceRecord) error {
	if record.Kind != EvidenceKind {
		return fmt.Errorf("EvidenceRecord kind must be %q", EvidenceKind)
	}
	if err := validateEnvelope(record.APIVersion, record.Kind, record.Integrity, &record.Metadata); err != nil {
		return err
	}
	if err := validateEvidenceFields(&record.Spec); err != nil {
		return err
	}
	if err := validateEvidenceStatus(record); err != nil {
		return err
	}
	return validateEvidenceTime(record)
}

func validateEvidenceFields(spec *EvidenceSpec) error {
	expectedLocator, exists := locatorByEvidenceType[spec.EvidenceType]
	if !exists || spec.Locator.LocatorType != expectedLocator {
		return fmt.Errorf("evidence_type %q requires locator_type %q", spec.EvidenceType, expectedLocator)
	}
	if !inSet(spec.Directness, "direct", "derived", "attested") ||
		!inSet(spec.Sensitivity, "public", "internal", "confidential", "restricted") {
		return fmt.Errorf("unsupported directness or sensitivity")
	}
	if !inSet(spec.SourceTrust, "untrusted", "observed") {
		return fmt.Errorf("source_trust %q is unavailable in shadow", spec.SourceTrust)
	}
	if spec.ContentRole != "untrusted_data" {
		return fmt.Errorf("content_role %q is unavailable in shadow", spec.ContentRole)
	}
	if err := validateIdentifierList("subjects", spec.Subjects, true); err != nil {
		return err
	}
	if err := validateCollector(spec.Collector); err != nil {
		return err
	}
	if err := validateSnapshot(spec.SourceSnapshot); err != nil {
		return err
	}
	return validateLocator(spec)
}

func validateCollector(collector Collector) error {
	if !inSet(collector.CollectorType, "human", "operator", "service", "tool") {
		return fmt.Errorf("unsupported collector_type %q", collector.CollectorType)
	}
	for name, value := range map[string]string{"collector_id": collector.CollectorID, "collector_version": collector.CollectorVersion, "run_id": collector.RunID} {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	return validateHash("collector.parameters_sha256", collector.ParametersSHA256)
}

func validateSnapshot(snapshot SourceSnapshot) error {
	if !inSet(snapshot.SnapshotType, "artifact", "external", "repository", "runtime") {
		return fmt.Errorf("unsupported snapshot_type %q", snapshot.SnapshotType)
	}
	if err := validateIdentifier("snapshot_id", snapshot.SnapshotID); err != nil {
		return err
	}
	return validateHash("snapshot_sha256", snapshot.SnapshotSHA256)
}

func validateLocator(spec *EvidenceSpec) error {
	locator := &spec.Locator
	if err := validateHash("locator.content_sha256", locator.ContentSHA256); err != nil {
		return err
	}
	if err := validateText("locator_ref", locator.LocatorRef); err != nil {
		return err
	}
	switch locator.LocatorType {
	case "repo":
		return validateRepoLocator(locator)
	case "command":
		if locator.ExitCode == nil || locator.LineStart != nil || locator.LineEnd != nil {
			return fmt.Errorf("command locator requires exit_code and forbids line fields")
		}
	default:
		if locator.ExitCode != nil || locator.LineStart != nil || locator.LineEnd != nil {
			return fmt.Errorf("%s locator forbids exit_code and line fields", locator.LocatorType)
		}
	}
	return validateDirectness(spec)
}

func validateRepoLocator(locator *Locator) error {
	if !safeRepositoryPath(locator.LocatorRef) || locator.ExitCode != nil {
		return fmt.Errorf("repo locator must be a safe relative path and cannot have exit_code")
	}
	if (locator.LineStart == nil) != (locator.LineEnd == nil) {
		return fmt.Errorf("repo locator line_start and line_end must appear together")
	}
	if locator.LineStart != nil && (*locator.LineStart < 1 || *locator.LineEnd < *locator.LineStart) {
		return fmt.Errorf("repo locator line range is invalid")
	}
	return nil
}

func validateDirectness(spec *EvidenceSpec) error {
	if spec.EvidenceType == "human_attestation" {
		if spec.Directness != "attested" || !inSet(spec.Collector.CollectorType, "human", "operator") {
			return fmt.Errorf("human_attestation must be attested by a human or operator")
		}
		return nil
	}
	if spec.EvidenceType == "external_source" && spec.Directness != "derived" {
		return fmt.Errorf("external_source must be derived")
	}
	if spec.Directness == "direct" {
		allowedType := inSet(spec.EvidenceType, "repo_locator", "test_run", "gate_result", "runtime_metric", "artifact")
		if !allowedType || !inSet(spec.Collector.CollectorType, "tool", "service") {
			return fmt.Errorf("direct evidence requires an eligible type and tool/service collector")
		}
	}
	return nil
}

func validateEvidenceStatus(record *EvidenceRecord) error {
	status := record.Status
	if !inSet(status.State, "valid", "invalid", "unavailable", "expired") {
		return fmt.Errorf("unsupported evidence state %q", status.State)
	}
	if err := validateStatusTime(status); err != nil {
		return err
	}
	hasArtifact := record.Spec.ArtifactSHA256 != nil
	hasReasons := len(status.ReasonCodes) > 0
	if status.State == "valid" && (!hasArtifact || hasReasons) {
		return fmt.Errorf("valid evidence requires artifact_sha256 and empty reason_codes")
	}
	if status.State == "unavailable" && (hasArtifact || !hasReasons) {
		return fmt.Errorf("unavailable evidence requires null artifact_sha256 and nonempty reason_codes")
	}
	if inSet(status.State, "invalid", "expired") && (!hasArtifact || !hasReasons) {
		return fmt.Errorf("%s evidence requires artifact_sha256 and nonempty reason_codes", status.State)
	}
	return validateOptionalHash("artifact_sha256", record.Spec.ArtifactSHA256)
}

func validateEvidenceTime(record *EvidenceRecord) error {
	if record.Spec.ObservedAtUnixMS < 0 || record.Spec.ObservedAtUnixMS > record.Metadata.CreatedAtUnixMS {
		return fmt.Errorf("observed_at_unix_ms must be nonnegative and no later than created_at_unix_ms")
	}
	if record.Status.ValidFromUnixMS < record.Spec.ObservedAtUnixMS {
		return fmt.Errorf("valid_from_unix_ms must not precede observed_at_unix_ms")
	}
	if record.Status.State == "expired" {
		until := record.Status.ValidUntilUnixMS
		if until == nil || *until > record.Metadata.CreatedAtUnixMS {
			return fmt.Errorf("expired evidence requires valid_until_unix_ms no later than creation")
		}
	}
	return validateDirectness(&record.Spec)
}
