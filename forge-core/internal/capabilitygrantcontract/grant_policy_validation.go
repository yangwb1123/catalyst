package capabilitygrantcontract

import "fmt"

func validateApprovalRefs(values []any) error {
	if len(values) > 32 {
		return fmt.Errorf("approval_refs may contain at most 32 items")
	}
	for index, value := range values {
		reference, ok := value.(map[string]any)
		if !ok || requireKeys(reference, "approval_id", "approval_sha256", "authority_domain") != nil {
			return fmt.Errorf("approval_ref %d has invalid fields", index)
		}
		for _, key := range []string{"approval_id", "authority_domain"} {
			text, err := stringValue(reference, key)
			if err != nil || validateText(text, key, 160) != nil {
				return fmt.Errorf("approval_ref %d %s is invalid", index, key)
			}
		}
		hash, err := stringValue(reference, "approval_sha256")
		if err != nil || validateHash(hash, "approval_sha256") != nil {
			return fmt.Errorf("approval_ref %d approval_sha256 is invalid", index)
		}
	}
	if err := validateSortedNodes(values, canonicalNodeKey); err != nil {
		return fmt.Errorf("approval_refs: %w", err)
	}
	return nil
}

func validateSeparationOfDuty(node map[string]any, issuer, subject map[string]any) error {
	if err := requireKeys(node, "requester", "required_distinctions"); err != nil {
		return err
	}
	requester, err := objectValue(node, "requester")
	if err != nil || validatePrincipal(requester) != nil {
		return fmt.Errorf("separation requester is invalid")
	}
	distinctions, err := readStringArray(node, "required_distinctions", 1, 5)
	if err != nil {
		return err
	}
	for _, distinction := range distinctions {
		if err := validateEnum(distinction, "required_distinction", "approver_not_issuer",
			"approver_not_requester", "approver_not_subject", "issuer_not_requester",
			"issuer_not_subject"); err != nil {
			return err
		}
	}
	if containsString(distinctions, "issuer_not_requester") && samePrincipal(issuer, requester) {
		return fmt.Errorf("issuer and requester violate separation of duty")
	}
	if containsString(distinctions, "issuer_not_subject") && samePrincipal(issuer, subject) {
		return fmt.Errorf("issuer and subject violate separation of duty")
	}
	return nil
}

func validateUsagePolicy(node map[string]any, budget map[string]any) error {
	if err := requireKeys(node, "atomic_reservation_required", "concurrent_use", "consumption_mode",
		"replay", "uncertain_effect", "usage_ledger_required"); err != nil {
		return err
	}
	atomic, atomicErr := boolValue(node, "atomic_reservation_required")
	ledger, ledgerErr := boolValue(node, "usage_ledger_required")
	if atomicErr != nil || ledgerErr != nil || !atomic || !ledger {
		return fmt.Errorf("v1 requires atomic reservation and an external usage ledger")
	}
	literals := map[string]string{
		"concurrent_use": "forbidden", "replay": "receipt_only_no_reexecute",
		"uncertain_effect": "quarantine",
	}
	for key, expected := range literals {
		if err := requireStringLiteral(node, key, expected); err != nil {
			return err
		}
	}
	mode, err := stringValue(node, "consumption_mode")
	if err != nil || validateEnum(mode, "consumption_mode", "bounded_calls", "single_use") != nil {
		return fmt.Errorf("consumption_mode is unsupported")
	}
	maxCalls, _ := intValue(budget, "max_calls")
	if mode == "single_use" && maxCalls != 1 {
		return fmt.Errorf("single_use requires max_calls=1")
	}
	return nil
}

func validateProductionRestriction(scope map[string]any, issuer map[string]any, approvals []any) error {
	effectID, _ := stringValue(scope, "effect_id")
	if effectID != "migration.apply" && effectID != "release.execute" {
		return nil
	}
	production, err := scopeContainsProduction(scope)
	if err != nil || !production {
		return err
	}
	authorityClass, _ := stringValue(issuer, "authority_class")
	if authorityClass != "external_operator" || len(approvals) == 0 {
		return fmt.Errorf("production %s requires declared external_operator issuer and approval_refs", effectID)
	}
	return nil
}

func scopeContainsProduction(scope map[string]any) (bool, error) {
	clauses, err := arrayValue(scope, "allow")
	if err != nil {
		return false, err
	}
	for _, clauseValue := range clauses {
		clause := clauseValue.(map[string]any)
		resources, _ := arrayValue(clause, "resources")
		for _, resourceValue := range resources {
			resource := resourceValue.(map[string]any)
			kind, _ := stringValue(resource, "scope_kind")
			class, _ := stringValue(resource, "environment_class")
			if kind == "environment" && class == "production" {
				return true, nil
			}
		}
	}
	return false, nil
}

func samePrincipal(left, right map[string]any) bool {
	for _, key := range []string{"authority_domain", "principal_id", "principal_type"} {
		if left[key] != right[key] {
			return false
		}
	}
	return true
}
