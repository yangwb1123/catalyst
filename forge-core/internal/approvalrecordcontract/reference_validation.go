package approvalrecordcontract

import "fmt"

func validateConditions(values []any, label string) error {
	if len(values) > 32 {
		return fmt.Errorf("%s may contain at most 32 items", label)
	}
	for index, value := range values {
		node, ok := value.(map[string]any)
		if !ok || requireKeys(node, "condition_id", "condition_ref", "condition_sha256") != nil {
			return fmt.Errorf("%s item %d has invalid fields", label, index)
		}
		identifier, idErr := stringValue(node, "condition_id")
		reference, refErr := stringValue(node, "condition_ref")
		hash, hashErr := stringValue(node, "condition_sha256")
		if idErr != nil || validateText(identifier, label+".condition_id", maxShortBytes) != nil ||
			refErr != nil || validateText(reference, label+".condition_ref", 4096) != nil ||
			hashErr != nil || validateHash(hash, label+".condition_sha256") != nil {
			return fmt.Errorf("%s item %d is invalid", label, index)
		}
	}
	if err := validateSortedNodes(values, canonicalNodeKey); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	return nil
}

func validateRiskRefs(values []any, label string) error {
	if len(values) > 32 {
		return fmt.Errorf("%s may contain at most 32 items", label)
	}
	for index, value := range values {
		node, ok := value.(map[string]any)
		if !ok || requireKeys(node, "authority_domain", "risk_acceptance_id",
			"risk_acceptance_sha256") != nil {
			return fmt.Errorf("%s item %d has invalid fields", label, index)
		}
		for _, key := range []string{"authority_domain", "risk_acceptance_id"} {
			text, err := stringValue(node, key)
			if err != nil || validateText(text, label+"."+key, maxShortBytes) != nil {
				return fmt.Errorf("%s item %d %s is invalid", label, index, key)
			}
		}
		hash, err := stringValue(node, "risk_acceptance_sha256")
		if err != nil || validateHash(hash, label+".risk_acceptance_sha256") != nil {
			return fmt.Errorf("%s item %d hash is invalid", label, index)
		}
	}
	if err := validateSortedNodes(values, canonicalNodeKey); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	return nil
}

func validateApprovalRef(reference map[string]any) error {
	if err := requireKeys(reference, "approval_id", "approval_sha256", "authority_domain"); err != nil {
		return fmt.Errorf("ApprovalRef: %w", err)
	}
	identifier, idErr := stringValue(reference, "approval_id")
	hash, hashErr := stringValue(reference, "approval_sha256")
	domain, domainErr := stringValue(reference, "authority_domain")
	if hashErr != nil || validateHash(hash, "approval_sha256") != nil ||
		idErr != nil || identifier != "approval-record-"+hash ||
		domainErr != nil || validateText(domain, "authority_domain", maxShortBytes) != nil {
		return fmt.Errorf("ApprovalRef identity is invalid")
	}
	return nil
}
