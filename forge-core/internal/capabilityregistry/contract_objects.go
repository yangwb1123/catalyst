package capabilityregistry

import "fmt"

func validateFailureMode(value map[string]any) (string, error) {
	if err := requireKeys(value, "disposition", "failure_id", "result"); err != nil {
		return "", err
	}
	disposition, err := stringValue(value, "disposition", 1, 64)
	if err != nil || !oneOf(disposition, "fail_closed_no_output", "structured_negative_assessment") {
		return "", fmt.Errorf("failure disposition is invalid")
	}
	if _, err := narrativeValue(value, "result"); err != nil {
		return "", err
	}
	return identifierValue(value, "failure_id")
}

func validateObservability(value map[string]any) (string, error) {
	if err := requireKeys(value, "signal_id", "signal_kind"); err != nil {
		return "", err
	}
	kind, err := stringValue(value, "signal_kind", 1, 16)
	if err != nil || !oneOf(kind, "artifact", "event", "log", "metric", "trace") {
		return "", fmt.Errorf("observability signal kind is invalid")
	}
	return identifierValue(value, "signal_id")
}

func validatePermission(value map[string]any) (string, error) {
	if err := requireKeys(value, "effect_id", "requirement_id", "scope_profile"); err != nil {
		return "", err
	}
	effect, err := identifierValue(value, "effect_id")
	if err != nil {
		return "", err
	}
	requirement, err := identifierValue(value, "requirement_id")
	if err != nil {
		return "", err
	}
	profile, err := identifierValue(value, "scope_profile")
	if err != nil {
		return "", err
	}
	expected, exists := frozenEffects[effect]
	if !exists || profile != expected {
		return "", fmt.Errorf("permission scope_profile is not the frozen effect profile")
	}
	return requirement, nil
}

func validateProof(value map[string]any) (string, error) {
	if err := requireKeys(value, "description", "obligation_id", "verification_refs"); err != nil {
		return "", err
	}
	if _, err := narrativeValue(value, "description"); err != nil {
		return "", err
	}
	refs, err := arrayValue(value, "verification_refs", 1, 64)
	if err != nil || validateContentRefs(refs, false) != nil {
		return "", fmt.Errorf("proof verification refs are invalid")
	}
	return identifierValue(value, "obligation_id")
}

func validateQualityGate(value map[string]any) (string, error) {
	if err := requireKeys(value, "gate_id", "required_test_ids"); err != nil {
		return "", err
	}
	tests, err := arrayValue(value, "required_test_ids", 1, 64)
	if err != nil || requireSortedUniqueStrings(tests, validIdentifier) != nil {
		return "", fmt.Errorf("gate required_test_ids are invalid")
	}
	return identifierValue(value, "gate_id")
}

func validateRule(value map[string]any) (string, error) {
	if err := requireKeys(value, "enforcement_mode", "rule_id", "statement"); err != nil {
		return "", err
	}
	mode, err := stringValue(value, "enforcement_mode", 1, 32)
	if err != nil || !oneOf(mode, "guidance", "hard_gate", "review_trigger") {
		return "", fmt.Errorf("rule enforcement mode is invalid")
	}
	if _, err := narrativeValue(value, "statement"); err != nil {
		return "", err
	}
	return identifierValue(value, "rule_id")
}

func validateRollback(value map[string]any) error {
	if err := requireKeys(value, "description", "mode"); err != nil {
		return err
	}
	if _, err := narrativeValue(value, "description"); err != nil {
		return err
	}
	mode, err := stringValue(value, "mode", 1, 64)
	if err != nil || !oneOf(mode, "compensation_declared", "external_operator_only",
		"not_required_no_effects", "rollback_declared") {
		return fmt.Errorf("rollback mode is invalid")
	}
	return nil
}

func identifierValue(value map[string]any, key string) (string, error) {
	identifier, err := stringValue(value, key, 1, maxIdentifierBytes)
	if err != nil || !validIdentifier(identifier) {
		return "", fmt.Errorf("field %q is not a valid identifier", key)
	}
	return identifier, nil
}

func narrativeValue(value map[string]any, key string) (string, error) {
	narrative, err := stringValue(value, key, 1, 4096)
	if err != nil {
		return "", fmt.Errorf("field %q is not a bounded narrative", key)
	}
	return narrative, nil
}

func validatePermissionRelations(contract map[string]any) error {
	effects, _ := contract["effects"].([]any)
	permissions, _ := contract["permission_requirements"].([]any)
	if len(effects) == 0 && len(permissions) != 0 {
		return fmt.Errorf("permission requirements must be empty when effects are empty")
	}
	allowed := make(map[string]struct{}, len(effects))
	for _, item := range effects {
		allowed[item.(string)] = struct{}{}
	}
	for _, item := range permissions {
		effect := item.(map[string]any)["effect_id"].(string)
		if _, exists := allowed[effect]; !exists {
			return fmt.Errorf("permission requirement references undeclared effect %q", effect)
		}
	}
	return nil
}
