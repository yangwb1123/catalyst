package kerneldecisioncontract

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"unicode"
	"unicode/utf8"

	op "forgeos/forge-core/internal/kerneloperationalcontract"
)

var hashPattern = regexp.MustCompile(`^[0-9a-f]{64}$`)
var identifierPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]{0,159}$`)
var legacyAtomPattern = regexp.MustCompile(`^atom-[0-9a-f]{64}$`)
var adrPattern = regexp.MustCompile(`^ADR-[0-9]{4,}$`)

func text(value, label string, maximum int) error {
	if value == "" || len(value) > maximum || !utf8.ValidString(value) {
		return fmt.Errorf("%s must be nonempty UTF-8 text <= %d bytes", label, maximum)
	}
	for _, character := range value {
		if unicode.IsControl(character) || character == 0x061c || character == 0x200e ||
			character == 0x200f || character >= 0x2028 && character <= 0x202e ||
			character >= 0x2066 && character <= 0x2069 {
			return fmt.Errorf("%s contains a forbidden Unicode scalar", label)
		}
	}
	return nil
}

func identifier(value, label string) error {
	if !identifierPattern.MatchString(value) {
		return fmt.Errorf("%s must match identifier grammar", label)
	}
	return nil
}

func hash(value, label string) error {
	if !hashPattern.MatchString(value) {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", label)
	}
	return nil
}

func identity(id, digest, prefix, label string, blank bool) error {
	if blank && id == "" && digest == "" {
		return nil
	}
	if err := hash(digest, label+"_sha256"); err != nil {
		return err
	}
	if id != prefix+digest {
		return fmt.Errorf("%s identity does not bind digest", label)
	}
	return nil
}

func validateAttestations(value DecisionAttestations) error {
	raw, err := canonicalBytes(value, maxStringBytes)
	if err != nil {
		return err
	}
	var values map[string]bool
	if err := json.Unmarshal(raw, &values); err != nil || len(values) != 22 {
		return fmt.Errorf("decision attestations must have exact twenty-two fields")
	}
	for _, enabled := range values {
		if enabled {
			return fmt.Errorf("all twenty-two decision attestations must be false")
		}
	}
	return nil
}

func validatePrincipal(value op.Principal, label string) error {
	if err := text(value.AuthorityDomain, label+".authority_domain", maxShortBytes); err != nil {
		return err
	}
	if err := text(value.PrincipalID, label+".principal_id", maxShortBytes); err != nil {
		return err
	}
	if !oneOf(value.PrincipalType, "agent", "human", "operator", "service") {
		return fmt.Errorf("%s.principal_type is unsupported", label)
	}
	return nil
}

func validateTask(value op.TaskBinding) error {
	strings := []string{value.ChangeID, value.EnvironmentID, value.NodeID,
		value.ProjectID, value.Role, value.RunID, value.TaskID}
	for _, item := range strings {
		if err := text(item, "task_binding field", maxShortBytes); err != nil {
			return err
		}
	}
	if !oneOf(value.EnvironmentClass, "development", "local", "production", "staging", "test") {
		return fmt.Errorf("task_binding.environment_class is unsupported")
	}
	for _, optional := range []*string{value.AttemptID, value.TargetID} {
		if optional != nil {
			if err := text(*optional, "task_binding optional field", maxShortBytes); err != nil {
				return err
			}
		}
	}
	return nil
}

func validateBindings(value op.OperationalBindings) error {
	for _, digest := range []string{value.ContextSHA256, value.EnvironmentSHA256,
		value.PolicySHA256, value.SourceTreeSHA256} {
		if err := hash(digest, "bindings digest"); err != nil {
			return err
		}
	}
	for _, item := range []string{value.EnvironmentProfileID, value.SourceProfileID,
		value.SourceRevision} {
		if err := text(item, "bindings text", maxShortBytes); err != nil {
			return err
		}
	}
	return nil
}

func validateCapability(value op.CapabilityIdentity) error {
	if err := hash(value.CapabilityContractSHA256, "capability_contract_sha256"); err != nil {
		return err
	}
	if err := text(value.CapabilityID, "capability_id", maxShortBytes); err != nil {
		return err
	}
	return text(value.CapabilityVersion, "capability_version", maxShortBytes)
}

func validateStringSet(values []string, label string, maximum int, nonempty bool) error {
	if values == nil || len(values) > maximum || nonempty && len(values) == 0 {
		return fmt.Errorf("%s cardinality is outside frozen bounds", label)
	}
	for _, value := range values {
		if err := identifier(value, label); err != nil {
			return err
		}
	}
	if !sort.StringsAreSorted(values) || duplicateStrings(values) {
		return fmt.Errorf("%s must be strictly sorted and unique", label)
	}
	return nil
}

func duplicateStrings(values []string) bool {
	for index := 1; index < len(values); index++ {
		if values[index-1] == values[index] {
			return true
		}
	}
	return false
}

func oneOf(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
}
