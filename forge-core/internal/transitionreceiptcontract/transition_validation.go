package transitionreceiptcontract

import "fmt"

func validateTransition(node map[string]any, label string) error {
	keys := []string{"declared_at_unix_ms", "from_state", "gate_id", "resume_state",
		"rework_target", "to_state"}
	if err := requireKeys(node, keys...); err != nil {
		return fmt.Errorf("%s: %w", label, err)
	}
	declaredAt, timeErr := intValue(node, "declared_at_unix_ms")
	fromState, fromErr := stringValue(node, "from_state")
	toState, toErr := stringValue(node, "to_state")
	if timeErr != nil || declaredAt < 0 || fromErr != nil || !isState(fromState) ||
		toErr != nil || !isState(toState) {
		return fmt.Errorf("%s time/state is invalid", label)
	}
	gate, gateErr := nullableStringValue(node, "gate_id")
	if gateErr != nil || (gate != nil && validateText(*gate, label+".gate_id", maxShortBytes) != nil) {
		return fmt.Errorf("%s.gate_id is invalid", label)
	}
	if err := validateRecoveryTarget(node, label, "rework_target", states); err != nil {
		return err
	}
	return validateRecoveryTarget(node, label, "resume_state", states)
}

func validateRecoveryTarget(node map[string]any, label, key string, allowed []string) error {
	value, err := nullableStringValue(node, key)
	if err != nil {
		return fmt.Errorf("%s.%s is invalid", label, key)
	}
	if value != nil && !containsString(allowed, *value) {
		return fmt.Errorf("%s.%s is not a frozen state", label, key)
	}
	return nil
}

func validatePreviousIdentity(receipt map[string]any, sequence int64) error {
	identifier, idErr := nullableStringValue(receipt, "previous_receipt_id")
	digest, hashErr := nullableStringValue(receipt, "previous_receipt_sha256")
	if idErr != nil || hashErr != nil {
		return fmt.Errorf("previous receipt identity fields are invalid")
	}
	if (identifier == nil) != (digest == nil) {
		return fmt.Errorf("previous receipt identity must be a nullable pair")
	}
	transition := receipt["transition"].(map[string]any)
	if sequence == 1 {
		if identifier != nil || transition["from_state"] != "DRAFT" {
			return fmt.Errorf("initial sequence requires null predecessor and from_state DRAFT")
		}
		return nil
	}
	if identifier == nil || validateHash(*digest, "previous_receipt_sha256") != nil ||
		*identifier != "transition-receipt-"+*digest {
		return fmt.Errorf("subsequent sequence requires a consistent predecessor identity")
	}
	return nil
}

func validateIntrinsicRecovery(transition map[string]any, label string) error {
	source := transition["from_state"].(string)
	target := transition["to_state"].(string)
	rework := transition["rework_target"]
	resume := transition["resume_state"]
	if (rework != nil) != (target == "CHANGES_REQUESTED") {
		return fmt.Errorf("%s rework_target must exist exactly on CHANGES_REQUESTED entry", label)
	}
	if rework != nil && !containsString(reworkStates, rework.(string)) {
		return fmt.Errorf("%s rework_target is outside the frozen six-state set", label)
	}
	if (resume != nil) != isSuspendedState(target) {
		return fmt.Errorf("%s resume_state must exist exactly on suspended-state entry", label)
	}
	if resume != nil && !(source == "NEEDS_INFO" && target == "BLOCKED") && resume != source {
		return fmt.Errorf("%s resume_state must preserve from_state", label)
	}
	return nil
}
