package transitionreceiptcontract

import "fmt"

var targetKeys = []string{
	"actor", "applicability", "approval_refs", "bindings", "capability_grant_ref",
	"declared_controller", "preconditions", "previous_receipt_id", "previous_receipt_sha256",
	"reason_codes", "sequence", "task_binding", "transition", "transition_vocabulary_sha256",
	"waiver_refs", "work_id",
}

func declaredTarget(receipt map[string]any) (map[string]any, error) {
	if err := validateReceipt(receipt, false); err != nil {
		return nil, err
	}
	target := make(map[string]any, len(targetKeys))
	for _, key := range targetKeys {
		target[key] = cloneValue(receipt[key])
	}
	if err := validateTarget(target); err != nil {
		return nil, err
	}
	return target, nil
}

func validateTarget(target map[string]any) error {
	if err := requireKeys(target, targetKeys...); err != nil {
		return fmt.Errorf("declared target: %w", err)
	}
	if err := validateCanonicalByteLimit(target, maxTargetBytes, "declared target"); err != nil {
		return err
	}
	if err := validateVocabularyBinding(target); err != nil {
		return err
	}
	if err := validateReceiptParts(target); err != nil {
		return err
	}
	return validateReceiptCounts(target)
}

func targetDigest(target map[string]any) (string, error) {
	if err := validateTarget(target); err != nil {
		return "", err
	}
	return digestNode(targetDomain, target)
}
