package evolvelocatorobservationproducer

import (
	"fmt"
	"reflect"
	"strings"

	locatorcontract "forgeos/forge-core/internal/evolverepolocatorevidencecontract"
	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/gitworktreesource"
)

func validateProductionPackage(value ProductionPackage) error {
	if value.APIVersion != ProductionAPIVersion || value.Canonicalization != Canonicalization {
		return fmt.Errorf("production identity is invalid")
	}
	if value.Observations == nil || len(value.Observations) > maxObservations {
		return fmt.Errorf("observations must be an explicit list of at most %d items", maxObservations)
	}
	parametersSHA256, err := validateParameters(value.ParametersManifest)
	if err != nil {
		return err
	}
	report, err := validateReportManifest(value.ReportManifest, value.ParametersManifest.ExpectedDepth)
	if err != nil {
		return err
	}
	sourceSHA256, err := gitworktreesource.Digest(value.SourceManifest)
	if err != nil {
		return fmt.Errorf("source manifest: %w", err)
	}
	if err := gitworktreesource.Validate(value.SourceManifest, sourceSHA256); err != nil {
		return fmt.Errorf("source manifest: %w", err)
	}
	return validateObservationProjection(
		value, report, parametersSHA256, value.ReportManifest.SHA256, sourceSHA256,
	)
}

func validateParameters(value ParametersManifest) (string, error) {
	if value.APIVersion != ParametersAPIVersion || value.Canonicalization != Canonicalization ||
		value.Contract != evolvescan.ContractV1 || !validDepth(value.ExpectedDepth) ||
		value.ReportProfileID != ReportProfileID || value.SourceProfileID != gitworktreesource.ProfileID {
		return "", fmt.Errorf("capture parameters manifest is invalid")
	}
	encoded, err := canonicalJSON(value)
	if err != nil {
		return "", fmt.Errorf("canonical capture parameters: %w", err)
	}
	return domainDigest(parametersDigestDomain, encoded), nil
}

func validateReportManifest(value ReportManifest, expectedDepth string) (evolvescan.Report, error) {
	if value.APIVersion != ReportAPIVersion || value.Canonicalization != Canonicalization ||
		value.ProfileID != ReportProfileID || strings.ContainsRune(value.CanonicalReport, '\n') ||
		!strings.HasPrefix(value.CanonicalReport, evolvescan.MarkerPrefix) {
		return evolvescan.Report{}, fmt.Errorf("report manifest identity or marker is invalid")
	}
	if value.Bytes != int64(len([]byte(value.CanonicalReport))) ||
		value.SHA256 != sha256Bytes([]byte(value.CanonicalReport)) {
		return evolvescan.Report{}, fmt.Errorf("report manifest bytes or digest does not match its preimage")
	}
	canonical, err := evolvescan.Canonicalize(value.CanonicalReport)
	if err != nil || canonical != value.CanonicalReport {
		return evolvescan.Report{}, fmt.Errorf("report manifest is not exact canonical Evolve output")
	}
	report, err := evolvescan.Parse(value.CanonicalReport)
	if err != nil {
		return evolvescan.Report{}, fmt.Errorf("parse report manifest: %w", err)
	}
	if err := validateCapturedReport(report, expectedDepth); err != nil {
		return evolvescan.Report{}, err
	}
	return canonicalReportOrder(report), nil
}

func validateObservationProjection(
	value ProductionPackage,
	report evolvescan.Report,
	parametersSHA256, reportSHA256, sourceSHA256 string,
) error {
	occurrences, err := reportOccurrences(report)
	if err != nil {
		return err
	}
	if len(occurrences) != len(value.Observations) {
		return fmt.Errorf("observations do not cover every report locator occurrence")
	}
	if len(value.Observations) == 0 {
		return nil
	}
	first := value.Observations[0]
	if len(first.Producer.RunID) > 160 || !runIDPattern.MatchString(first.Producer.RunID) ||
		first.ObservedAtUnixMS < 0 {
		return fmt.Errorf("observation run or time is invalid")
	}
	facts, err := manifestFacts(value.SourceManifest, occurrences)
	if err != nil {
		return err
	}
	expected, err := buildObservations(
		occurrences, facts, parametersSHA256, reportSHA256, first.Producer.RunID,
		value.SourceManifest.SourceRevision, sourceSHA256, first.ObservedAtUnixMS,
	)
	if err != nil {
		return err
	}
	setObservationDepth(expected, value.ParametersManifest.ExpectedDepth)
	for index := range expected {
		if !reflect.DeepEqual(value.Observations[index], expected[index]) {
			return fmt.Errorf("observation[%d] does not exactly project its report occurrence", index)
		}
		if _, err := locatorcontract.CanonicalObservationJSON(value.Observations[index]); err != nil {
			return fmt.Errorf("observation[%d]: %w", index, err)
		}
	}
	return nil
}

func manifestFacts(
	manifest gitworktreesource.SourceManifest,
	occurrences []occurrence,
) (map[string]fileFact, error) {
	facts := make(map[string]fileFact)
	for _, item := range occurrences {
		if _, exists := facts[item.evidence.Path]; exists {
			continue
		}
		var matched *gitworktreesource.SourceEntry
		for index := range manifest.Entries {
			if manifest.Entries[index].Path == item.evidence.Path {
				matched = &manifest.Entries[index]
				break
			}
		}
		if matched == nil || matched.Kind != "regular" || matched.ContentSHA256 == nil ||
			matched.Bytes < 1 || matched.Bytes > maxEvidenceFileBytes {
			return nil, fmt.Errorf("locator path %q lacks a bounded regular source entry", item.evidence.Path)
		}
		facts[item.evidence.Path] = fileFact{bytes: matched.Bytes, sha256: *matched.ContentSHA256}
	}
	return facts, nil
}
