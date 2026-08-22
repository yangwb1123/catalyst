package capabilityregistry

import "fmt"

func validateContract(value map[string]any) error {
	keys := []string{
		"api_version", "canonicalization", "capability_contract_id", "capability_contract_sha256",
		"capability_id", "capability_version", "domain", "effects", "failure_modes",
		"input_schemas", "kind", "not_applicable", "observability", "output_schemas",
		"permission_requirements", "postconditions", "preconditions", "proof_obligations",
		"quality_gates", "risk_floor", "rollback_or_compensation", "rules", "trigger",
	}
	if err := requireKeys(value, keys...); err != nil {
		return fmt.Errorf("capability contract: %w", err)
	}
	if err := validateContractConstants(value); err != nil {
		return err
	}
	if err := validateContractArrays(value); err != nil {
		return err
	}
	if err := validateContractNested(value); err != nil {
		return err
	}
	if err := requirePrefixedIdentity(value, contractDigestDomain,
		"capability_contract_id", "capability_contract_sha256", "capability-contract-"); err != nil {
		return err
	}
	return requireCanonicalSize(value, maxContractBytes, "capability contract")
}

func validateContractConstants(value map[string]any) error {
	for field, expected := range map[string]string{
		"api_version": contractAPIVersion, "canonicalization": canonicalization,
		"capability_id": capabilityID, "capability_version": capabilityVersion,
		"kind": "CapabilityContract",
	} {
		if err := requireString(value, field, expected); err != nil {
			return err
		}
	}
	domain, err := stringValue(value, "domain", 1, 32)
	if err != nil || !oneOf(domain, "device", "execution", "governance", "planning", "reasoning", "verification") {
		return fmt.Errorf("capability contract domain is invalid")
	}
	risk, err := stringValue(value, "risk_floor", 2, 2)
	if err != nil || !oneOf(risk, "L0", "L1", "L2", "L3", "L4") {
		return fmt.Errorf("capability contract risk_floor is invalid")
	}
	return nil
}

func validateContractArrays(value map[string]any) error {
	stringSets := []struct {
		key       string
		minimum   int
		validator func(string) bool
	}{
		{"effects", 0, validIdentifier}, {"postconditions", 1, validIdentifier},
		{"preconditions", 1, validIdentifier},
	}
	for _, set := range stringSets {
		items, err := arrayValue(value, set.key, set.minimum, 64)
		if err != nil || requireSortedUniqueStrings(items, set.validator) != nil {
			return fmt.Errorf("contract %s must be sorted and unique", set.key)
		}
		if set.key == "effects" && !knownEffects(items) {
			return fmt.Errorf("contract effects contain an item outside the frozen vocabulary")
		}
	}
	inputs, err := arrayValue(value, "input_schemas", 1, 64)
	if err != nil || validateContentRefs(inputs, true) != nil {
		return fmt.Errorf("contract input_schemas are invalid")
	}
	outputs, err := arrayValue(value, "output_schemas", 1, 64)
	if err != nil || validateContentRefs(outputs, true) != nil {
		return fmt.Errorf("contract output_schemas are invalid")
	}
	return nil
}

func knownEffects(values []any) bool {
	for _, item := range values {
		if _, exists := frozenEffects[item.(string)]; !exists {
			return false
		}
	}
	return true
}

func validateContractNested(value map[string]any) error {
	checks := []struct {
		key       string
		minimum   int
		validator func(map[string]any) (string, error)
	}{
		{"failure_modes", 1, validateFailureMode}, {"observability", 1, validateObservability},
		{"permission_requirements", 0, validatePermission}, {"proof_obligations", 1, validateProof},
		{"quality_gates", 1, validateQualityGate}, {"rules", 1, validateRule},
	}
	for _, check := range checks {
		items, err := arrayValue(value, check.key, check.minimum, 64)
		if err != nil || validateNamedSet(items, check.validator) != nil {
			return fmt.Errorf("contract %s are invalid", check.key)
		}
	}
	for _, key := range []string{"not_applicable", "trigger"} {
		object, err := objectValue(value, key)
		if err != nil || validatePredicateSet(object) != nil {
			return fmt.Errorf("contract %s is invalid", key)
		}
	}
	rollback, err := objectValue(value, "rollback_or_compensation")
	if err != nil || validateRollback(rollback) != nil {
		return fmt.Errorf("contract rollback_or_compensation is invalid")
	}
	return validatePermissionRelations(value)
}

func validateNamedSet(values []any, validator func(map[string]any) (string, error)) error {
	objects, err := requireObjectItems(values)
	if err != nil {
		return err
	}
	previous := ""
	for index, object := range objects {
		identity, err := validator(object)
		if err != nil {
			return fmt.Errorf("item %d: %w", index, err)
		}
		if index > 0 && identity <= previous {
			return fmt.Errorf("named set must be identity sorted and unique")
		}
		previous = identity
	}
	return nil
}

func validatePredicateSet(value map[string]any) error {
	if err := requireKeys(value, "mode", "predicates"); err != nil {
		return err
	}
	mode, err := stringValue(value, "mode", 3, 5)
	if err != nil || !oneOf(mode, "all", "any", "never") {
		return fmt.Errorf("predicate mode is invalid")
	}
	items, err := arrayValue(value, "predicates", 0, 64)
	if err != nil || mode == "never" && len(items) != 0 || mode != "never" && len(items) == 0 {
		return fmt.Errorf("predicate set cardinality does not match mode")
	}
	return validateCanonicalObjectSet(items, validatePredicate)
}

func validatePredicate(value map[string]any) error {
	if err := requireKeys(value, "document", "json_pointer", "operator", "value"); err != nil {
		return err
	}
	document, err := stringValue(value, "document", 1, 16)
	if err != nil || !oneOf(document, "input", "output") {
		return fmt.Errorf("predicate document is invalid")
	}
	pointer, err := stringValue(value, "json_pointer", 0, maxRepoPathBytes)
	if err != nil || !validJSONPointer(pointer, false) {
		return fmt.Errorf("predicate JSON pointer is invalid")
	}
	operator, err := stringValue(value, "operator", 1, 16)
	if err != nil || !oneOf(operator, "absent", "equals", "not_equals", "present") {
		return fmt.Errorf("predicate operator is invalid")
	}
	if (operator == "absent" || operator == "present") && value["value"] != nil {
		return fmt.Errorf("presence predicates require null value")
	}
	if operator == "equals" || operator == "not_equals" {
		if text, ok := value["value"].(string); !ok || len(text) > maxRepoPathBytes ||
			validateWireString(text) != nil {
			return fmt.Errorf("predicate value is invalid")
		}
	}
	return nil
}

func validateCanonicalObjectSet(values []any, validator func(map[string]any) error) error {
	objects, err := requireObjectItems(values)
	if err != nil {
		return err
	}
	previous := ""
	for index, object := range objects {
		if err := validator(object); err != nil {
			return err
		}
		encoded, _ := canonicalJSON(object)
		if index > 0 && string(encoded) <= previous {
			return fmt.Errorf("structured set must be canonical-byte sorted and unique")
		}
		previous = string(encoded)
	}
	return nil
}
