package transitionreceiptcontract

func recoveryRelation(current, previous map[string]any) string {
	transition := current["transition"].(map[string]any)
	consistent := entryRecoveryConsistent(transition, previous) &&
		exitRecoveryConsistent(transition, previous)
	return relation(consistent, "internally_consistent_declared_recovery",
		"rework_or_resume_mismatch")
}

func entryRecoveryConsistent(transition, previous map[string]any) bool {
	source := transition["from_state"].(string)
	target := transition["to_state"].(string)
	rework := transition["rework_target"]
	resume := transition["resume_state"]
	if (rework != nil) != (target == "CHANGES_REQUESTED") {
		return false
	}
	if rework != nil && !containsString(reworkStates, rework.(string)) {
		return false
	}
	if (resume != nil) != isSuspendedState(target) {
		return false
	}
	expected := any(source)
	if source == "NEEDS_INFO" && target == "BLOCKED" && previous != nil {
		expected = previous["transition"].(map[string]any)["resume_state"]
	}
	return resume == nil || resume == expected
}

func exitRecoveryConsistent(transition, previous map[string]any) bool {
	source := transition["from_state"].(string)
	target := transition["to_state"].(string)
	if source == "CHANGES_REQUESTED" {
		if previous == nil {
			return false
		}
		rework := previous["transition"].(map[string]any)["rework_target"]
		return target == rework || containsString([]string{"BLOCKED", "REJECTED", "SUPERSEDED"}, target)
	}
	if !isSuspendedState(source) {
		return true
	}
	if previous == nil {
		return false
	}
	resume := previous["transition"].(map[string]any)["resume_state"]
	escalations := []string{"REJECTED", "SUPERSEDED"}
	if source == "NEEDS_INFO" {
		escalations = append([]string{"BLOCKED"}, escalations...)
	}
	return target == resume || containsString(escalations, target)
}
