package transitionreceiptcontract

import "fmt"

func validateGrantRef(node map[string]any, label string) error {
	if err := requireKeys(node, "authority_domain", "grant_id", "grant_sha256"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	identifier, idErr := stringValue(node, "grant_id")
	digest, hashErr := stringValue(node, "grant_sha256")
	domain, domainErr := stringValue(node, "authority_domain")
	if hashErr != nil || validateHash(digest, label+".grant_sha256") != nil ||
		idErr != nil || identifier != "capability-grant-"+digest ||
		domainErr != nil || validateText(domain, label+".authority_domain", maxShortBytes) != nil {
		return fmt.Errorf("%s identity is invalid", label)
	}
	return nil
}

func validateApprovalRef(node map[string]any, label string) error {
	if err := requireKeys(node, "approval_id", "approval_sha256", "authority_domain"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	identifier, idErr := stringValue(node, "approval_id")
	digest, hashErr := stringValue(node, "approval_sha256")
	domain, domainErr := stringValue(node, "authority_domain")
	if hashErr != nil || validateHash(digest, label+".approval_sha256") != nil ||
		idErr != nil || validateText(identifier, label+".approval_id", maxShortBytes) != nil ||
		domainErr != nil || validateText(domain, label+".authority_domain", maxShortBytes) != nil {
		return fmt.Errorf("%s identity is invalid", label)
	}
	return nil
}

func validateWaiverRef(node map[string]any, label string) error {
	if err := requireKeys(node, "authority_domain", "waiver_id", "waiver_sha256"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	for _, key := range []string{"authority_domain", "waiver_id"} {
		value, err := stringValue(node, key)
		if err != nil || validateText(value, label+"."+key, maxShortBytes) != nil {
			return fmt.Errorf("%s.%s is invalid", label, key)
		}
	}
	digest, err := stringValue(node, "waiver_sha256")
	if err != nil || validateHash(digest, label+".waiver_sha256") != nil {
		return fmt.Errorf("%s.waiver_sha256 is invalid", label)
	}
	return nil
}

func validateEvidenceRef(node map[string]any, label string) error {
	if err := requireKeys(node, "canonical_sha256", "record_id"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	recordID, idErr := stringValue(node, "record_id")
	digest, hashErr := stringValue(node, "canonical_sha256")
	if idErr != nil || validateText(recordID, label+".record_id", maxShortBytes) != nil ||
		hashErr != nil || validateHash(digest, label+".canonical_sha256") != nil {
		return fmt.Errorf("%s is invalid", label)
	}
	return nil
}

func validateReferenceArray(values []any, label string, maximum int,
	validator func(map[string]any, string) error) error {
	if len(values) > maximum {
		return fmt.Errorf("%s may contain at most %d items", label, maximum)
	}
	for index, value := range values {
		node, ok := value.(map[string]any)
		itemLabel := fmt.Sprintf("%s[%d]", label, index)
		if !ok || validator(node, itemLabel) != nil {
			return fmt.Errorf("%s is invalid", itemLabel)
		}
	}
	return validateSortedNodes(values, label)
}
