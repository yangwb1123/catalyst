package kerneloperationalcontract

import (
	"bytes"
	"fmt"
	"sort"
)

func validateText(value, label string, maximum int) error {
	if value == "" || len(value) > maximum {
		return fmt.Errorf("%s must be non-empty UTF-8 text <= %d bytes", label, maximum)
	}
	if err := validateWireString(value); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	return nil
}

func validateIdentifier(value, label string) error {
	if err := validateText(value, label, maxShortBytes); err != nil {
		return err
	}
	for index, character := range []byte(value) {
		valid := character >= 'a' && character <= 'z' ||
			character >= '0' && character <= '9' ||
			index > 0 && bytes.ContainsRune([]byte("._:/-"), rune(character))
		if !valid {
			return fmt.Errorf("%s does not match the frozen identifier grammar", label)
		}
	}
	return nil
}

func validateHash(value, label string) error {
	if len(value) != 64 {
		return fmt.Errorf("%s must be a lowercase bare SHA-256", label)
	}
	for _, character := range []byte(value) {
		if !(character >= '0' && character <= '9') && !(character >= 'a' && character <= 'f') {
			return fmt.Errorf("%s must be a lowercase bare SHA-256", label)
		}
	}
	return nil
}

func validateNonnegative(value int64, label string, maximum int64) error {
	if value < 0 || value > maximum {
		return fmt.Errorf("%s must be in 0..%d", label, maximum)
	}
	return nil
}

func validatePrincipal(value Principal, label string) error {
	if err := validateText(value.AuthorityDomain, label+".authority_domain", maxShortBytes); err != nil {
		return err
	}
	if err := validateText(value.PrincipalID, label+".principal_id", maxShortBytes); err != nil {
		return err
	}
	if !oneOf(value.PrincipalType, "agent", "human", "operator", "service") {
		return fmt.Errorf("%s.principal_type is unsupported", label)
	}
	return nil
}

func validateTaskBinding(value TaskBinding) error {
	texts := map[string]string{"change_id": value.ChangeID, "environment_id": value.EnvironmentID,
		"node_id": value.NodeID, "project_id": value.ProjectID, "role": value.Role,
		"run_id": value.RunID, "task_id": value.TaskID}
	for label, text := range texts {
		if err := validateText(text, "task_binding."+label, maxShortBytes); err != nil {
			return err
		}
	}
	if !oneOf(value.EnvironmentClass, "development", "local", "production", "staging", "test") {
		return fmt.Errorf("task_binding.environment_class is unsupported")
	}
	for label, optional := range map[string]*string{"attempt_id": value.AttemptID, "target_id": value.TargetID} {
		if optional != nil {
			if err := validateText(*optional, "task_binding."+label, maxShortBytes); err != nil {
				return err
			}
		}
	}
	return nil
}

func validateBindings(value OperationalBindings) error {
	hashes := map[string]string{"context_sha256": value.ContextSHA256,
		"environment_sha256": value.EnvironmentSHA256, "policy_sha256": value.PolicySHA256,
		"source_tree_sha256": value.SourceTreeSHA256}
	for label, hash := range hashes {
		if err := validateHash(hash, "bindings."+label); err != nil {
			return err
		}
	}
	texts := map[string]string{"environment_profile_id": value.EnvironmentProfileID,
		"source_profile_id": value.SourceProfileID, "source_revision": value.SourceRevision}
	for label, text := range texts {
		if err := validateText(text, "bindings."+label, maxShortBytes); err != nil {
			return err
		}
	}
	return nil
}

func validateCapability(value CapabilityIdentity) error {
	if err := validateHash(value.CapabilityContractSHA256, "capability.capability_contract_sha256"); err != nil {
		return err
	}
	if err := validateText(value.CapabilityID, "capability.capability_id", maxShortBytes); err != nil {
		return err
	}
	return validateText(value.CapabilityVersion, "capability.capability_version", maxShortBytes)
}

func validateGrantRef(value CapabilityGrantRef) error {
	if err := validateText(value.AuthorityDomain, "capability_grant_ref.authority_domain", maxShortBytes); err != nil {
		return err
	}
	if err := validateHash(value.GrantSHA256, "capability_grant_ref.grant_sha256"); err != nil {
		return err
	}
	if value.GrantID != "capability-grant-"+value.GrantSHA256 {
		return fmt.Errorf("capability_grant_ref.grant_id must bind grant_sha256")
	}
	return nil
}

func validateAttestations(value Attestations) error {
	if value.Authorization || value.BindingAuthentication || value.Completion ||
		value.ContentProvenance || value.Effect || value.EventAppend || value.Execution ||
		value.GrantAuthentication || value.Outcome || value.Permission || value.Persistence ||
		value.PrincipalAuth || value.Transition || value.UsageMeasurement {
		return fmt.Errorf("every operational attestation must be exactly false")
	}
	return nil
}

func validateArtifact(value ArtifactRef, label string) error {
	if err := validateText(value.ArtifactKind, label+".artifact_kind", maxShortBytes); err != nil {
		return err
	}
	if err := validateText(value.ArtifactRef, label+".artifact_ref", maxReferenceBytes); err != nil {
		return err
	}
	return validateHash(value.ArtifactSHA256, label+".artifact_sha256")
}

func canonicalKey(value any) ([]byte, error) {
	node, err := typedNode(value, maxClosureBytes)
	if err != nil {
		return nil, err
	}
	return canonicalJSON(node, maxClosureBytes)
}

func validateSortedUnique[T any](values []T, label string, maximum int,
	nonempty bool, validate func(T, string) error) error {
	if values == nil || len(values) > maximum || nonempty && len(values) == 0 {
		return fmt.Errorf("%s cardinality is outside the frozen bound", label)
	}
	keys := make([]string, len(values))
	for index, value := range values {
		if err := validate(value, fmt.Sprintf("%s[%d]", label, index)); err != nil {
			return err
		}
		key, err := canonicalKey(value)
		if err != nil {
			return err
		}
		keys[index] = string(key)
	}
	if !sort.StringsAreSorted(keys) || hasDuplicate(keys) {
		return fmt.Errorf("%s must be strictly canonical-byte sorted and unique", label)
	}
	return nil
}

func validateStringSet(values []string, label string, maximum int, nonempty bool) error {
	if values == nil || len(values) > maximum || nonempty && len(values) == 0 {
		return fmt.Errorf("%s cardinality is outside the frozen bound", label)
	}
	for index, value := range values {
		if err := validateIdentifier(value, fmt.Sprintf("%s[%d]", label, index)); err != nil {
			return err
		}
	}
	if !sort.StringsAreSorted(values) || hasDuplicate(values) {
		return fmt.Errorf("%s must be strictly UTF-8 sorted and unique", label)
	}
	return nil
}

func hasDuplicate(values []string) bool {
	for index := 1; index < len(values); index++ {
		if values[index] == values[index-1] {
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
