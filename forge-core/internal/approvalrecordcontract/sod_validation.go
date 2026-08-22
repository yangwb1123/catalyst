package approvalrecordcontract

import "fmt"

var sodDeclarationKeys = []string{
	"implementers", "proof_profile_id", "proof_profile_sha256", "requester",
	"required_distinctions",
}

func validateSoDDeclaration(node map[string]any, label string) error {
	if err := requireKeys(node, sodDeclarationKeys...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	requester, err := objectValue(node, "requester")
	if err != nil || validatePrincipal(requester, label+".requester", false) != nil {
		return fmt.Errorf("%s.requester is invalid", label)
	}
	implementers, err := arrayValue(node, "implementers")
	if err != nil || len(implementers) > 32 {
		return fmt.Errorf("%s.implementers must contain 0..32 items", label)
	}
	for index, value := range implementers {
		principal, ok := value.(map[string]any)
		if !ok || validatePrincipal(principal, fmt.Sprintf("%s.implementers[%d]", label, index), false) != nil {
			return fmt.Errorf("%s.implementers[%d] is invalid", label, index)
		}
	}
	if err := validateSortedNodes(implementers, canonicalNodeKey); err != nil {
		return fmt.Errorf("%s.implementers: %w", label, err)
	}
	if _, err := readAllowedStrings(node, "required_distinctions", 1, 3, distinctions); err != nil {
		return err
	}
	profile, profileErr := stringValue(node, "proof_profile_id")
	hash, hashErr := stringValue(node, "proof_profile_sha256")
	if profileErr != nil || validateText(profile, label+".proof_profile_id", maxShortBytes) != nil ||
		hashErr != nil || validateHash(hash, label+".proof_profile_sha256") != nil {
		return fmt.Errorf("%s proof profile is invalid", label)
	}
	return nil
}

func validateSoD(node, record map[string]any) error {
	keys := append(append([]string{}, sodDeclarationKeys...), "proof_base64url")
	if err := requireKeys(node, keys...); err != nil {
		return fmt.Errorf("separation_of_duty: %w", err)
	}
	declaration := make(map[string]any, len(sodDeclarationKeys))
	for _, key := range sodDeclarationKeys {
		declaration[key] = node[key]
	}
	if err := validateSoDDeclaration(declaration, "separation_of_duty"); err != nil {
		return err
	}
	proof, err := stringValue(node, "proof_base64url")
	if err != nil || validateBase64URL(proof, "separation_of_duty.proof_base64url") != nil {
		return fmt.Errorf("separation_of_duty.proof_base64url is invalid")
	}
	return validateDeclaredDistinctions(node, record)
}

func validateDeclaredDistinctions(sod, record map[string]any) error {
	return validateSoDConsistency(sod, record["approver"].(map[string]any),
		record["subject"].(map[string]any), record["scope"].(map[string]any),
		record["risk_acceptance_refs"].([]any))
}

func validateSoDConsistency(sod, approver, subject, scope map[string]any, risks []any) error {
	values, _ := readStringArray(sod, "required_distinctions", 1, 3)
	set := make(map[string]bool, len(values))
	for _, value := range values {
		set[value] = true
	}
	requester := sod["requester"].(map[string]any)
	if set["approver_not_requester"] && principalKey(approver) == principalKey(requester) {
		return fmt.Errorf("approver_not_requester contradicts declared identities")
	}
	if set["approver_not_subject"] && principalKey(approver) == principalKey(subject) {
		return fmt.Errorf("approver_not_subject contradicts declared identities")
	}
	implementers := sod["implementers"].([]any)
	if set["approver_not_implementer"] && principalIn(approver, implementers) {
		return fmt.Errorf("approver_not_implementer contradicts declared identities")
	}
	materiality := scope["materiality_level"]
	if (materiality == "L3" || materiality == "L4") && (len(implementers) == 0 || len(set) != 3) {
		return fmt.Errorf("L3/L4 requires implementers and all SoD distinctions")
	}
	if len(risks) > 0 && !set["approver_not_requester"] {
		return fmt.Errorf("RiskAcceptance refs require approver_not_requester")
	}
	return nil
}

func principalIn(target map[string]any, values []any) bool {
	key := principalKey(target)
	for _, value := range values {
		if principalKey(value.(map[string]any)) == key {
			return true
		}
	}
	return false
}

func readAllowedStrings(parent map[string]any, key string, minimum, maximum int,
	allowed []string) ([]string, error) {
	values, err := readStringArray(parent, key, minimum, maximum)
	if err != nil {
		return nil, err
	}
	for _, value := range values {
		if validateEnum(value, key, allowed...) != nil {
			return nil, fmt.Errorf("%s contains an unsupported value", key)
		}
	}
	return values, nil
}
