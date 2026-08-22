package transitionreceiptcontract

import "fmt"

func validatePreconditions(values []any, label string) error {
	if len(values) < 1 || len(values) > 64 {
		return fmt.Errorf("%s must contain 1..64 items", label)
	}
	for index, value := range values {
		node, ok := value.(map[string]any)
		if !ok || validatePrecondition(node, fmt.Sprintf("%s[%d]", label, index)) != nil {
			return fmt.Errorf("%s item %d is invalid", label, index)
		}
	}
	return validateSortedNodes(values, label)
}

func validatePrecondition(node map[string]any, label string) error {
	if err := requireKeys(node, "evidence_refs", "precondition_id", "reason_codes", "result"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	identifier, idErr := stringValue(node, "precondition_id")
	result, resultErr := stringValue(node, "result")
	if idErr != nil || validateText(identifier, label+".precondition_id", maxShortBytes) != nil ||
		resultErr != nil || validateEnum(result, label+".result", "FAIL", "NA", "PASS", "UNKNOWN") != nil {
		return fmt.Errorf("%s identity/result is invalid", label)
	}
	_, err := validateReasonCodes(node, label, 0, maxReasonCodes)
	if err != nil {
		return err
	}
	evidence, err := arrayValue(node, "evidence_refs")
	if err != nil {
		return err
	}
	if err := validateReferenceArray(evidence, label+".evidence_refs", 32, validateEvidenceRef); err != nil {
		return err
	}
	return nil
}

func validateApplicability(node map[string]any, label string, strict bool) error {
	if err := requireKeys(node, "decision", "evidence_refs", "reason_codes", "stage_id"); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	stage, stageErr := stringValue(node, "stage_id")
	decision, decisionErr := stringValue(node, "decision")
	if stageErr != nil || validateText(stage, label+".stage_id", maxShortBytes) != nil ||
		decisionErr != nil || validateEnum(decision, label+".decision", "applicable", "not_applicable") != nil {
		return fmt.Errorf("%s stage/decision is invalid", label)
	}
	reasons, err := validateReasonCodes(node, label, 0, maxReasonCodes)
	if err != nil {
		return err
	}
	evidence, err := arrayValue(node, "evidence_refs")
	if err != nil {
		return err
	}
	if err := validateReferenceArray(evidence, label+".evidence_refs", 32, validateEvidenceRef); err != nil {
		return err
	}
	if strict && decision == "applicable" && len(reasons) != 0 {
		return fmt.Errorf("%s applicable requires empty reason_codes", label)
	}
	if strict && decision == "not_applicable" && (len(reasons) == 0 || len(evidence) == 0) {
		return fmt.Errorf("%s not_applicable requires reason_codes and evidence_refs", label)
	}
	return nil
}

func validateReasonCodes(node map[string]any, label string, minimum, maximum int) ([]string, error) {
	reasons, err := readStringArray(node, "reason_codes", minimum, maximum)
	if err != nil {
		return nil, fmt.Errorf("%s.reason_codes: %w", label, err)
	}
	for _, reason := range reasons {
		if err := validateIdentifier(reason, label+".reason_codes"); err != nil {
			return nil, err
		}
	}
	return reasons, nil
}
