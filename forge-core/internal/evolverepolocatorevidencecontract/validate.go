package evolverepolocatorevidencecontract

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
	"unicode"
)

const (
	maxContentBytes = int64(1_048_576)
	maxDetailBytes  = 512
	maxPathScalars  = 4096
)

var (
	hashPattern       = regexp.MustCompile(`^[a-f0-9]{64}$`)
	identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)
	evolveIDPattern   = regexp.MustCompile(`^[a-z0-9][a-z0-9._-]*$`)
)

func validateRequest(request *Request) error {
	if request == nil {
		return fmt.Errorf("evolve repository locator evidence request is nil")
	}
	if request.APIVersion != APIVersion {
		return fmt.Errorf("api_version must be %q", APIVersion)
	}
	if request.Canonicalization != Canonicalization {
		return fmt.Errorf("canonicalization must be %q", Canonicalization)
	}
	if err := validateBinding(request.Binding); err != nil {
		return fmt.Errorf("binding: %w", err)
	}
	if err := validateObservationSemantics(request.Observation); err != nil {
		return fmt.Errorf("observation: %w", err)
	}
	return nil
}

// ValidateObservation validates a standalone Evolve locator observation. It
// does not read the locator, verify the report, or grant truth or authority.
func ValidateObservation(observation Observation) error {
	if err := validateObservationSemantics(observation); err != nil {
		return err
	}
	_, err := canonicalBounded(observationNode(observation), "observation")
	return err
}

func validateObservationSemantics(observation Observation) error {
	if observation.APIVersion != ObservationAPIVersion {
		return fmt.Errorf("api_version must be %q", ObservationAPIVersion)
	}
	if observation.Canonicalization != Canonicalization {
		return fmt.Errorf("canonicalization must be %q", Canonicalization)
	}
	if err := validateContent(observation.Content); err != nil {
		return fmt.Errorf("content: %w", err)
	}
	if err := validateLocator(observation.Locator); err != nil {
		return fmt.Errorf("locator: %w", err)
	}
	if observation.ObservedAtUnixMS < 0 {
		return fmt.Errorf("observed_at_unix_ms must be nonnegative")
	}
	if err := validateProducer(observation.Producer); err != nil {
		return fmt.Errorf("producer: %w", err)
	}
	if err := validateScanContext(observation.ScanContext); err != nil {
		return fmt.Errorf("scan_context: %w", err)
	}
	if err := validateSource(observation.Source); err != nil {
		return fmt.Errorf("source: %w", err)
	}
	return nil
}

func validateContent(content Content) error {
	if content.Bytes < 1 || content.Bytes > maxContentBytes {
		return fmt.Errorf("bytes must be integer 1..%d", maxContentBytes)
	}
	return validateHash("sha256", content.SHA256)
}

func validateLocator(locator Locator) error {
	if err := validateString(locator.Detail); err != nil {
		return fmt.Errorf("detail: %w", err)
	}
	if strings.TrimSpace(locator.Detail) == "" || containsUnicodeControl(locator.Detail) || len(locator.Detail) > maxDetailBytes {
		return fmt.Errorf("detail must be non-blank text up to %d UTF-8 bytes", maxDetailBytes)
	}
	if locator.Line < 0 {
		return fmt.Errorf("line must be a nonnegative signed-int64 integer")
	}
	return validateRepositoryPath("path", locator.Path)
}

func validateProducer(producer Producer) error {
	for _, field := range []struct{ name, value string }{
		{"producer_id", producer.ProducerID},
		{"producer_version", producer.ProducerVersion},
		{"run_id", producer.RunID},
	} {
		if err := validateIdentifier(field.name, field.value); err != nil {
			return err
		}
	}
	if !inSet(producer.ProducerType, "service", "tool") {
		return fmt.Errorf("unsupported producer_type %q", producer.ProducerType)
	}
	return validateHash("parameters_sha256", producer.ParametersSHA256)
}

func validateScanContext(context ScanContext) error {
	if context.Contract != ScanContract {
		return fmt.Errorf("contract must be %q", ScanContract)
	}
	if !inSet(context.Depth, "advisory", "opportunistic", "standard", "thorough") {
		return fmt.Errorf("unsupported depth %q", context.Depth)
	}
	if !inSet(context.Dimension, "architecture_drift", "code", "dependencies", "performance", "security", "test_coverage") {
		return fmt.Errorf("unsupported dimension %q", context.Dimension)
	}
	if !inSet(context.Relation, "clear", "finding", "opportunity") {
		return fmt.Errorf("unsupported relation %q", context.Relation)
	}
	if context.Relation == "opportunity" {
		if context.OpportunityID == nil {
			return fmt.Errorf("opportunity_id must be a bounded identifier for opportunity relation")
		}
		if err := validateEvolveID("opportunity_id", *context.OpportunityID); err != nil {
			return err
		}
	} else if context.OpportunityID != nil {
		return fmt.Errorf("opportunity_id must be null outside opportunity relation")
	}
	return validateHash("report_sha256", context.ReportSHA256)
}

func validateSource(source Source) error {
	if err := validateIdentifier("source_revision", source.SourceRevision); err != nil {
		return err
	}
	return validateHash("source_tree_sha256", source.SourceTreeSHA256)
}

func validateBinding(binding Binding) error {
	for _, field := range []struct{ name, value string }{
		{"aggregate_id", binding.AggregateID},
		{"project_id", binding.ProjectID},
		{"scope", binding.Scope},
	} {
		if err := validateIdentifier(field.name, field.value); err != nil {
			return err
		}
	}
	if err := validateHash("context_sha256", binding.ContextSHA256); err != nil {
		return err
	}
	if err := validateHash("policy_sha256", binding.PolicySHA256); err != nil {
		return err
	}
	if binding.Sequence < 1 {
		return fmt.Errorf("sequence must be positive")
	}
	if !inSet(binding.Sensitivity, "confidential", "internal", "public", "restricted") {
		return fmt.Errorf("unsupported sensitivity %q", binding.Sensitivity)
	}
	if err := validateIdentifierList("subjects", binding.Subjects, true); err != nil {
		return err
	}
	return validateIdentifierList("supersedes_record_ids", binding.SupersedesRecordIDs, false)
}

func validateIdentifier(name, value string) error {
	if err := validateString(value); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	if len(value) > 160 || !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid bounded identifier", name)
	}
	return nil
}

func validateEvolveID(name, value string) error {
	if err := validateString(value); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	if len(value) > 64 || !evolveIDPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid Evolve identifier", name)
	}
	return nil
}

func validateHash(name, value string) error {
	if err := validateString(value); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", name)
	}
	return nil
}

func validateIdentifierList(name string, values []string, nonempty bool) error {
	if len(values) > maxItems {
		return fmt.Errorf("%s exceeds %d items", name, maxItems)
	}
	if nonempty && len(values) == 0 {
		return fmt.Errorf("%s must be nonempty", name)
	}
	for index, value := range values {
		if err := validateIdentifier(fmt.Sprintf("%s[%d]", name, index), value); err != nil {
			return err
		}
	}
	if !sort.StringsAreSorted(values) {
		return fmt.Errorf("%s must already be UTF-8-byte sorted", name)
	}
	for index := 1; index < len(values); index++ {
		if values[index] == values[index-1] {
			return fmt.Errorf("%s contains duplicate %q", name, values[index])
		}
	}
	return nil
}

func validateRepositoryPath(name, value string) error {
	if err := validateString(value); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	if strings.TrimSpace(value) == "" || len([]rune(value)) > maxPathScalars || strings.HasPrefix(value, "/") || strings.HasPrefix(value, `\`) || strings.Contains(value, `\`) {
		return fmt.Errorf("%s must be a canonical repository-relative path", name)
	}
	if containsUnicodeControl(value) {
		return fmt.Errorf("%s must not contain Unicode control characters", name)
	}
	if len(value) >= 2 && isASCIIAlpha(value[0]) && value[1] == ':' {
		return fmt.Errorf("%s must be a canonical repository-relative path", name)
	}
	segments := strings.Split(value, "/")
	for _, segment := range segments {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("%s must be a canonical repository-relative path", name)
		}
	}
	if asciiEqualFold(segments[0], ".git") || asciiEqualFold(segments[0], ".forge") {
		return fmt.Errorf("%s protected repository control path is forbidden", name)
	}
	return nil
}

func containsUnicodeControl(value string) bool {
	for _, character := range value {
		if unicode.IsControl(character) {
			return true
		}
	}
	return false
}

func asciiEqualFold(left, right string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := 0; index < len(left); index++ {
		leftByte, rightByte := left[index], right[index]
		if leftByte >= 'A' && leftByte <= 'Z' {
			leftByte += 'a' - 'A'
		}
		if rightByte >= 'A' && rightByte <= 'Z' {
			rightByte += 'a' - 'A'
		}
		if leftByte != rightByte {
			return false
		}
	}
	return true
}

func isASCIIAlpha(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}

func inSet(value string, candidates ...string) bool {
	for _, candidate := range candidates {
		if value == candidate {
			return true
		}
	}
	return false
}
