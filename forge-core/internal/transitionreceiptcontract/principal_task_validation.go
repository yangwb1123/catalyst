package transitionreceiptcontract

import "fmt"

func validatePrincipal(node map[string]any, label string, controller bool) error {
	if err := requireKeys(node, "authority_domain", "principal_id", "principal_type"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	for _, key := range []string{"authority_domain", "principal_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	principalType, err := stringValue(node, "principal_type")
	if err != nil {
		return err
	}
	allowed := []string{"agent", "human", "operator", "service"}
	if controller {
		allowed = []string{"human", "operator", "service"}
	}
	return validateEnum(principalType, label+".principal_type", allowed...)
}

func validateTaskBinding(node map[string]any, label string) error {
	keys := []string{"attempt_id", "change_id", "environment_class", "environment_id", "node_id",
		"project_id", "role", "run_id", "target_id", "task_id"}
	if err := requireKeys(node, keys...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	for _, key := range []string{"change_id", "environment_id", "node_id", "project_id", "role", "run_id", "task_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	for _, key := range []string{"attempt_id", "target_id"} {
		value, err := nullableStringValue(node, key)
		if err != nil || (value != nil && validateText(*value, label+"."+key, maxShortBytes) != nil) {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	class, err := stringValue(node, "environment_class")
	if err != nil {
		return err
	}
	return validateEnum(class, label+".environment_class", "development", "local", "production", "staging", "test")
}
