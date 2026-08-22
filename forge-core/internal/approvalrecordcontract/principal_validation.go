package approvalrecordcontract

import "fmt"

func validatePrincipal(node map[string]any, label string, approverOnly bool) error {
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
	if approverOnly {
		allowed = []string{"human", "operator"}
	}
	return validateEnum(principalType, label+".principal_type", allowed...)
}

func principalKey(node map[string]any) string {
	return node["authority_domain"].(string) + "\x00" + node["principal_id"].(string) +
		"\x00" + node["principal_type"].(string)
}

func validateAuthoritySource(node map[string]any, label string) error {
	if err := requireKeys(node, "authority_class", "authority_domain", "principal_id",
		"principal_type"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	authorityClass, classErr := stringValue(node, "authority_class")
	principalType, typeErr := stringValue(node, "principal_type")
	if classErr != nil || typeErr != nil {
		return fmt.Errorf("%s authority class/principal type is invalid", label)
	}
	if err := validateEnum(authorityClass, label+".authority_class",
		"external_operator", "forgeos_kernel"); err != nil {
		return err
	}
	for _, key := range []string{"authority_domain", "principal_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	if authorityClass == "forgeos_kernel" && principalType != "service" {
		return fmt.Errorf("%s principal type contradicts authority_class", label)
	}
	if authorityClass == "external_operator" && principalType != "human" && principalType != "operator" {
		return fmt.Errorf("%s principal type contradicts authority_class", label)
	}
	return nil
}
