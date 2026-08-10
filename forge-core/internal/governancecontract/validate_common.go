package governancecontract

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
	"unicode/utf8"
)

var (
	hashPattern       = regexp.MustCompile(`^[a-f0-9]{64}$`)
	identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)
)

func validateEnvelope(apiVersion, kind string, integrity Integrity, metadata *Metadata) error {
	if apiVersion != APIVersion {
		return fmt.Errorf("api_version must be %q", APIVersion)
	}
	if kind != EvidenceKind && kind != ClaimKind {
		return fmt.Errorf("unsupported kind %q", kind)
	}
	if integrity.Canonicalization != Canonicalization {
		return fmt.Errorf("canonicalization must be %q", Canonicalization)
	}
	if err := validateHash("integrity.canonical_sha256", integrity.CanonicalSHA256); err != nil {
		return err
	}
	return validateMetadata(metadata)
}

func validateMetadata(metadata *Metadata) error {
	identifierFields := map[string]string{
		"aggregate_id": metadata.AggregateID, "project_id": metadata.ProjectID,
		"record_id": metadata.RecordID, "scope": metadata.Scope,
		"source_revision": metadata.SourceRevision,
	}
	for name, value := range identifierFields {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	if metadata.CreatedAtUnixMS < 0 || metadata.Sequence < 1 {
		return fmt.Errorf("created_at_unix_ms must be nonnegative and sequence must be positive")
	}
	for name, value := range map[string]string{"context_sha256": metadata.ContextSHA256, "policy_sha256": metadata.PolicySHA256, "source_tree_sha256": metadata.SourceTreeSHA256} {
		if err := validateHash(name, value); err != nil {
			return err
		}
	}
	if err := validatePrincipal(metadata.CreatedBy); err != nil {
		return err
	}
	return validateIdentifierList("supersedes_record_ids", metadata.SupersedesRecordIDs, false)
}

func validatePrincipal(principal Principal) error {
	if !inSet(principal.PrincipalType, "agent", "human", "operator", "service", "tool") {
		return fmt.Errorf("unsupported principal_type %q", principal.PrincipalType)
	}
	for name, value := range map[string]string{"authority_domain": principal.AuthorityDomain, "principal_id": principal.PrincipalID, "role": principal.Role, "run_id": principal.RunID} {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	return nil
}

func validateStatusTime(status Status) error {
	if status.ValidFromUnixMS < 0 {
		return fmt.Errorf("valid_from_unix_ms must be nonnegative")
	}
	if status.ValidUntilUnixMS != nil && *status.ValidUntilUnixMS <= status.ValidFromUnixMS {
		return fmt.Errorf("valid_until_unix_ms must be greater than valid_from_unix_ms")
	}
	return validateIdentifierList("reason_codes", status.ReasonCodes, false)
}

func validateIdentifier(name, value string) error {
	if len(value) > 160 || !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s is not a valid identifier", name)
	}
	return nil
}

func validateText(name, value string) error {
	if err := validateString(value); err != nil {
		return fmt.Errorf("%s: %w", name, err)
	}
	length := utf8.RuneCountInString(value)
	if length < 1 || length > 4096 {
		return fmt.Errorf("%s must contain 1..4096 Unicode scalars", name)
	}
	return nil
}

func validateHash(name, value string) error {
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", name)
	}
	return nil
}

func validateOptionalHash(name string, value *string) error {
	if value == nil {
		return nil
	}
	return validateHash(name, *value)
}

func validateIdentifierList(name string, values []string, nonempty bool) error {
	if len(values) > maxArrayItems {
		return fmt.Errorf("%s exceeds %d items", name, maxArrayItems)
	}
	if nonempty && len(values) == 0 {
		return fmt.Errorf("%s must be nonempty", name)
	}
	for _, value := range values {
		if err := validateIdentifier(name, value); err != nil {
			return err
		}
	}
	return validateSortedUnique(name, values)
}

func validateSortedUnique(name string, values []string) error {
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

func inSet(value string, candidates ...string) bool {
	for _, candidate := range candidates {
		if value == candidate {
			return true
		}
	}
	return false
}

func safeRepositoryPath(value string) bool {
	if value == "" || strings.HasPrefix(value, "/") || strings.Contains(value, "\\") {
		return false
	}
	if len(value) >= 2 && isASCIIAlpha(value[0]) && value[1] == ':' {
		return false
	}
	for _, segment := range strings.Split(value, "/") {
		if segment == "" || segment == "." || segment == ".." {
			return false
		}
	}
	return true
}

func isASCIIAlpha(value byte) bool {
	return value >= 'A' && value <= 'Z' || value >= 'a' && value <= 'z'
}
