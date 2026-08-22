package transitionreceiptcontract

import "fmt"

var receiptKeys = []string{
	"actor", "api_version", "applicability", "approval_refs", "bindings", "canonicalization",
	"capability_grant_ref", "declared_controller", "kind", "preconditions", "previous_receipt_id",
	"previous_receipt_sha256", "reason_codes", "receipt_id", "receipt_sha256", "sequence",
	"task_binding", "transition", "transition_vocabulary_sha256", "waiver_refs", "work_id",
}

func validateReceipt(receipt map[string]any, allowEmptyIdentity bool) error {
	if err := requireKeys(receipt, receiptKeys...); err != nil {
		return fmt.Errorf("TransitionReceipt: %w", err)
	}
	if err := validateReceiptLiterals(receipt); err != nil {
		return err
	}
	if err := validateCanonicalByteLimit(receipt, maxReceiptBytes, "TransitionReceipt"); err != nil {
		return err
	}
	if err := validateReceiptParts(receipt); err != nil {
		return err
	}
	if err := validateReceiptCounts(receipt); err != nil {
		return err
	}
	return validateReceiptIdentity(receipt, allowEmptyIdentity)
}

func validateReceiptLiterals(receipt map[string]any) error {
	literals := map[string]string{
		"api_version": receiptAPI, "canonicalization": canonicalization, "kind": recordKind,
	}
	for key, expected := range literals {
		value, err := stringValue(receipt, key)
		if err != nil || value != expected {
			return fmt.Errorf("%s must equal %q", key, expected)
		}
	}
	return validateVocabularyBinding(receipt)
}

func validateVocabularyBinding(receipt map[string]any) error {
	vocabularyHash, err := stringValue(receipt, "transition_vocabulary_sha256")
	if err != nil || validateHash(vocabularyHash, "transition_vocabulary_sha256") != nil {
		return fmt.Errorf("transition_vocabulary_sha256 is invalid")
	}
	vocabulary, err := authoredVocabulary()
	if err != nil || vocabularyHash != vocabulary["vocabulary_sha256"] {
		return fmt.Errorf("TransitionReceipt does not bind the frozen Transition vocabulary")
	}
	return nil
}

func validateReceiptParts(receipt map[string]any) error {
	objects := make(map[string]map[string]any)
	for _, key := range []string{"actor", "applicability", "bindings", "capability_grant_ref",
		"declared_controller", "task_binding", "transition"} {
		value, err := objectValue(receipt, key)
		if err != nil {
			return err
		}
		objects[key] = value
	}
	validators := receiptPartValidators(receipt, objects)
	for _, validator := range validators {
		if err := validator(); err != nil {
			return err
		}
	}
	return validateReceiptScalarParts(receipt)
}

func receiptPartValidators(receipt map[string]any, objects map[string]map[string]any) []func() error {
	return []func() error{
		func() error { return validatePrincipal(objects["actor"], "actor", false) },
		func() error { return validateApplicability(objects["applicability"], "applicability", true) },
		func() error { return validateBindings(objects["bindings"], "bindings") },
		func() error { return validateGrantRef(objects["capability_grant_ref"], "capability_grant_ref") },
		func() error { return validatePrincipal(objects["declared_controller"], "declared_controller", true) },
		func() error { return validateTaskBinding(objects["task_binding"], "task_binding") },
		func() error { return validateTransition(objects["transition"], "transition") },
		func() error {
			if objects["applicability"]["stage_id"] != objects["transition"]["to_state"] {
				return fmt.Errorf("applicability stage must equal transition.to_state")
			}
			return validateIntrinsicRecovery(objects["transition"], "TransitionReceipt")
		},
		func() error { return validateReceiptArrays(receipt) },
	}
}

func validateReceiptArrays(receipt map[string]any) error {
	approvals, approvalErr := arrayValue(receipt, "approval_refs")
	preconditions, preconditionErr := arrayValue(receipt, "preconditions")
	waivers, waiverErr := arrayValue(receipt, "waiver_refs")
	if approvalErr != nil || preconditionErr != nil || waiverErr != nil {
		return fmt.Errorf("TransitionReceipt reference arrays are invalid")
	}
	if err := validateReferenceArray(approvals, "approval_refs", 32, validateApprovalRef); err != nil {
		return err
	}
	if err := validatePreconditions(preconditions, "preconditions"); err != nil {
		return err
	}
	return validateReferenceArray(waivers, "waiver_refs", 32, validateWaiverRef)
}

func validateReceiptScalarParts(receipt map[string]any) error {
	workID, workErr := stringValue(receipt, "work_id")
	sequence, sequenceErr := intValue(receipt, "sequence")
	if workErr != nil || validateText(workID, "work_id", maxShortBytes) != nil {
		return fmt.Errorf("work_id is invalid")
	}
	if sequenceErr != nil || sequence < 1 {
		return fmt.Errorf("sequence must be at least 1")
	}
	if _, err := validateReasonCodes(receipt, "TransitionReceipt", 0, maxReceiptReasons); err != nil {
		return err
	}
	return validatePreviousIdentity(receipt, sequence)
}

func validateReceiptCounts(receipt map[string]any) error {
	totalReasons := len(receipt["reason_codes"].([]any))
	applicability := receipt["applicability"].(map[string]any)
	totalReasons += len(applicability["reason_codes"].([]any))
	totalEvidence := len(applicability["evidence_refs"].([]any))
	for _, value := range receipt["preconditions"].([]any) {
		precondition := value.(map[string]any)
		totalReasons += len(precondition["reason_codes"].([]any))
		totalEvidence += len(precondition["evidence_refs"].([]any))
	}
	if totalReasons > maxReceiptReasons || totalEvidence > 256 {
		return fmt.Errorf("TransitionReceipt aggregate reason/evidence ceiling exceeded")
	}
	return nil
}

func validateReceiptIdentity(receipt map[string]any, allowEmpty bool) error {
	identifier, idErr := stringValue(receipt, "receipt_id")
	claimed, hashErr := stringValue(receipt, "receipt_sha256")
	if idErr != nil || hashErr != nil {
		return fmt.Errorf("TransitionReceipt identity fields are invalid")
	}
	if allowEmpty && identifier == "" && claimed == "" {
		return nil
	}
	if validateHash(claimed, "receipt_sha256") != nil || identifier != "transition-receipt-"+claimed {
		return fmt.Errorf("TransitionReceipt identity does not match its digest")
	}
	computed, err := receiptDigest(receipt)
	if err != nil || computed != claimed {
		return fmt.Errorf("TransitionReceipt self digest does not match")
	}
	return nil
}

func receiptDigest(receipt map[string]any) (string, error) {
	preimage := cloneNode(receipt)
	preimage["receipt_id"] = ""
	preimage["receipt_sha256"] = ""
	return digestNode(receiptDomain, preimage)
}
