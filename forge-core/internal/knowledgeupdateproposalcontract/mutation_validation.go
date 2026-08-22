package knowledgeupdateproposalcontract

import "fmt"

var mutationKeys = []string{
	"after_claim_ref", "before_claim_ref", "operation", "rationale", "reason_codes",
	"target_aggregate_id", "target_kind",
}

func validateMutationsShape(values []any) error {
	if len(values) < 1 || len(values) > maxMutations {
		return fmt.Errorf("mutations must contain 1..%d items", maxMutations)
	}
	previousTarget := ""
	afterIDs := make(map[string]bool, len(values))
	beforeIDs := make(map[string]bool, len(values))
	for index, value := range values {
		mutation, ok := value.(map[string]any)
		if !ok {
			return fmt.Errorf("mutations[%d] must be an object", index)
		}
		target, afterID, beforeID, err := validateMutationShape(mutation, index)
		if err != nil {
			return err
		}
		if index > 0 && target <= previousTarget {
			return fmt.Errorf("mutations must be strictly UTF-8 sorted by unique target_aggregate_id")
		}
		if afterIDs[afterID] {
			return fmt.Errorf("after_claim_ref %q is reused", afterID)
		}
		afterIDs[afterID] = true
		if beforeID != "" {
			if beforeIDs[beforeID] {
				return fmt.Errorf("before_claim_ref %q forks across mutations", beforeID)
			}
			beforeIDs[beforeID] = true
		}
		previousTarget = target
	}
	for recordID := range afterIDs {
		if beforeIDs[recordID] {
			return fmt.Errorf("mutation after_claim_ref %q cannot also be used as a before_claim_ref", recordID)
		}
	}
	return nil
}

func validateMutationShape(mutation map[string]any, index int) (string, string, string, error) {
	label := fmt.Sprintf("mutations[%d]", index)
	if err := requireKeys(mutation, mutationKeys...); err != nil {
		return "", "", "", fmt.Errorf("%s: %w", label, err)
	}
	if err := requireStringLiteral(mutation, "target_kind", "KnowledgeClaim"); err != nil {
		return "", "", "", fmt.Errorf("%s: %w", label, err)
	}
	operation, err := stringValue(mutation, "operation")
	if err != nil || validateEnum(operation, label+".operation", "create", "supersede") != nil {
		return "", "", "", fmt.Errorf("%s.operation must be create or supersede", label)
	}
	target, err := stringValue(mutation, "target_aggregate_id")
	if err != nil || validateIdentifier(target, label+".target_aggregate_id") != nil {
		return "", "", "", fmt.Errorf("%s.target_aggregate_id is invalid", label)
	}
	rationale, err := stringValue(mutation, "rationale")
	if err != nil || validateText(rationale, label+".rationale", maxRationaleBytes) != nil {
		return "", "", "", fmt.Errorf("%s.rationale is invalid", label)
	}
	reasons, err := readStringArray(mutation, "reason_codes", 1, maxMutationReasons)
	if err != nil {
		return "", "", "", fmt.Errorf("%s: %w", label, err)
	}
	for _, reason := range reasons {
		if err := validateIdentifier(reason, label+".reason_codes"); err != nil {
			return "", "", "", err
		}
	}
	afterID, beforeID, err := validateMutationRefs(mutation, label, operation)
	if err != nil {
		return "", "", "", err
	}
	return target, afterID, beforeID, nil
}

func validateMutationRefs(mutation map[string]any, label, operation string) (string, string, error) {
	after, err := objectValue(mutation, "after_claim_ref")
	if err != nil || validateClaimRef(after, label+".after_claim_ref") != nil {
		return "", "", fmt.Errorf("%s.after_claim_ref is invalid", label)
	}
	afterID, _ := stringValue(after, "record_id")
	beforeID := ""
	if mutation["before_claim_ref"] != nil {
		before, ok := mutation["before_claim_ref"].(map[string]any)
		if !ok || validateClaimRef(before, label+".before_claim_ref") != nil {
			return "", "", fmt.Errorf("%s.before_claim_ref is invalid", label)
		}
		beforeID, _ = stringValue(before, "record_id")
	}
	if operation == "create" && beforeID != "" {
		return "", "", fmt.Errorf("%s create requires null before_claim_ref", label)
	}
	if operation == "supersede" && beforeID == "" {
		return "", "", fmt.Errorf("%s supersede requires before_claim_ref", label)
	}
	if beforeID == afterID {
		return "", "", fmt.Errorf("%s before_claim_ref and after_claim_ref must identify different records", label)
	}
	return afterID, beforeID, nil
}

func validateClaimRef(node map[string]any, label string) error {
	if err := requireKeys(node, "canonical_sha256", "record_id"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	recordID, idErr := stringValue(node, "record_id")
	digest, hashErr := stringValue(node, "canonical_sha256")
	if idErr != nil || validateIdentifier(recordID, label+".record_id") != nil ||
		hashErr != nil || validateHash(digest, label+".canonical_sha256") != nil {
		return fmt.Errorf("%s is invalid", label)
	}
	return nil
}
