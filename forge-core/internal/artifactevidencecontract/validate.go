package artifactevidencecontract

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	"forgeos/forge-core/internal/artifact"
)

var (
	hashPattern        = regexp.MustCompile(`^[a-f0-9]{64}$`)
	identifierPattern  = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)
	rfc3339NanoPattern = regexp.MustCompile(`^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\.[0-9]{1,9})?(Z|[+-][0-9]{2}:[0-9]{2})$`)
)

func validateRequest(request *Request) error {
	if request == nil {
		return fmt.Errorf("artifact evidence request is nil")
	}
	if request.APIVersion != APIVersion {
		return fmt.Errorf("api_version must be %q", APIVersion)
	}
	if request.Canonicalization != Canonicalization {
		return fmt.Errorf("canonicalization must be %q", Canonicalization)
	}
	if err := validateArtifact(request.Artifact); err != nil {
		return fmt.Errorf("artifact: %w", err)
	}
	if err := validateBinding(request.Binding); err != nil {
		return fmt.Errorf("binding: %w", err)
	}
	return nil
}

func validateArtifact(record artifact.Record) error {
	if record.Format != artifact.FormatV1 {
		return fmt.Errorf("_format must be %q", artifact.FormatV1)
	}
	if err := validateIdentifier("run_id", record.RunID); err != nil {
		return err
	}
	if err := validateArtifactTextFields(record); err != nil {
		return err
	}
	if err := validateRepositoryPath("path", record.Path); err != nil {
		return err
	}
	if err := validateHash("sha256", record.SHA256); err != nil {
		return err
	}
	if err := validateHash("prompt_sha256", record.PromptSHA256); err != nil {
		return err
	}
	if record.Size < 1 {
		return fmt.Errorf("size must be positive")
	}
	_, err := artifactUnixMillis(record.CreatedAt)
	return err
}

func validateArtifactTextFields(record artifact.Record) error {
	fields := []struct{ name, value string }{
		{"workflow", record.Workflow}, {"phase", record.Phase},
		{"agent", record.Agent}, {"model", record.Model},
	}
	for _, field := range fields {
		if err := validateText(field.name, field.value); err != nil {
			return err
		}
	}
	return nil
}

func validateBinding(binding Binding) error {
	if err := validateBindingIdentifiers(binding); err != nil {
		return err
	}
	if err := validateBindingHashes(binding); err != nil {
		return err
	}
	if binding.Sequence < 1 {
		return fmt.Errorf("sequence must be positive")
	}
	if !inSet(binding.Sensitivity, "public", "internal", "confidential", "restricted") {
		return fmt.Errorf("unsupported sensitivity %q", binding.Sensitivity)
	}
	if err := validateIdentifierList("subjects", binding.Subjects, true); err != nil {
		return err
	}
	return validateIdentifierList("supersedes_record_ids", binding.SupersedesRecordIDs, false)
}

func validateBindingIdentifiers(binding Binding) error {
	fields := []struct{ name, value string }{
		{"aggregate_id", binding.AggregateID}, {"project_id", binding.ProjectID},
		{"scope", binding.Scope}, {"source_revision", binding.SourceRevision},
	}
	for _, field := range fields {
		if err := validateIdentifier(field.name, field.value); err != nil {
			return err
		}
	}
	return nil
}

func validateBindingHashes(binding Binding) error {
	fields := []struct{ name, value string }{
		{"context_sha256", binding.ContextSHA256},
		{"policy_sha256", binding.PolicySHA256},
		{"source_tree_sha256", binding.SourceTreeSHA256},
	}
	for _, field := range fields {
		if err := validateHash(field.name, field.value); err != nil {
			return err
		}
	}
	return nil
}

func validateIdentifier(name, value string) error {
	if len(value) > 160 || !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid identifier", name)
	}
	return nil
}

func validateHash(name, value string) error {
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
	for _, value := range values {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	if !sort.StringsAreSorted(values) {
		return fmt.Errorf("%s must already be lexicographically sorted", name)
	}
	for index := 1; index < len(values); index++ {
		if values[index] == values[index-1] {
			return fmt.Errorf("%s contains duplicate %q", name, values[index])
		}
	}
	return nil
}

func validateRepositoryPath(name, value string) error {
	if err := validateText(name, value); err != nil {
		return err
	}
	if value == "" || strings.HasPrefix(value, "/") || strings.Contains(value, `\`) {
		return fmt.Errorf("%s must be a safe normalized repo-relative path", name)
	}
	if len(value) >= 2 && isASCIIAlpha(value[0]) && value[1] == ':' {
		return fmt.Errorf("%s must be a safe normalized repo-relative path", name)
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return fmt.Errorf("%s must be a safe normalized repo-relative path", name)
		}
	}
	return nil
}

func validateText(name, value string) error {
	if err := validateString(value); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	if strings.TrimSpace(value) == "" {
		return fmt.Errorf("%s must not be blank", name)
	}
	length := utf8.RuneCountInString(value)
	if length < 1 || length > 4096 {
		return fmt.Errorf("%s must contain 1..4096 Unicode scalars", name)
	}
	return nil
}

func artifactUnixMillis(value string) (int64, error) {
	if err := validateString(value); err != nil {
		return 0, fmt.Errorf("created_at: %w", err)
	}
	length := utf8.RuneCountInString(value)
	if length < 20 || length > 40 {
		return 0, fmt.Errorf("created_at must contain 20..40 Unicode scalars")
	}
	if !rfc3339NanoPattern.MatchString(value) {
		return 0, fmt.Errorf("created_at must be strict RFC3339Nano")
	}
	if value[len(value)-1] != 'Z' {
		offset := value[len(value)-6:]
		hours := int(offset[1]-'0')*10 + int(offset[2]-'0')
		minutes := int(offset[4]-'0')*10 + int(offset[5]-'0')
		if hours > 23 || minutes > 59 {
			return 0, fmt.Errorf("created_at UTC offset is outside 00:00..23:59")
		}
	}
	parsed, err := time.Parse(time.RFC3339Nano, value)
	if err != nil {
		return 0, fmt.Errorf("created_at must be RFC3339Nano: %w", err)
	}
	milliseconds := parsed.UnixMilli()
	if milliseconds < 0 {
		return 0, fmt.Errorf("created_at must not precede the Unix epoch")
	}
	return milliseconds, nil
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
