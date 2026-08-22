package transitionreceiptcontract

import "sort"

var relationKeys = []string{
	"applicability", "chain", "continuity", "edge", "preconditions", "recovery",
	"target", "temporal",
}

var negativeRelations = map[string]bool{
	"predecessor_mismatch": true, "state_continuity_mismatch": true,
	"unlisted_declared_edge": true, "declared_fail_or_unknown_present": true,
	"rework_or_resume_mismatch": true, "target_mismatch": true,
	"temporal_declaration_mismatch": true,
}

func assessDeclared(request map[string]any) (map[string]any, error) {
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	receipt := request["transition_receipt"].(map[string]any)
	relations, err := evaluateRelations(request)
	if err != nil {
		return nil, err
	}
	reasons := relationReasons(relations)
	assessment := baseAssessment(request, receipt, relations, reasons)
	digest, err := assessmentDigest(assessment)
	if err != nil {
		return nil, err
	}
	assessment["assessment_sha256"] = digest
	if err := validateAssessment(assessment); err != nil {
		return nil, err
	}
	return assessment, nil
}

func evaluateRelations(request map[string]any) (map[string]any, error) {
	current := request["transition_receipt"].(map[string]any)
	previous, _ := request["previous_receipt"].(map[string]any)
	target, err := declaredTarget(current)
	if err != nil {
		return nil, err
	}
	return map[string]any{
		"applicability": "internally_consistent_declared_applicability",
		"chain":         chainRelation(current, previous),
		"continuity":    continuityRelation(current, previous),
		"edge":          edgeRelation(current, previous),
		"preconditions": preconditionsRelation(current),
		"recovery":      recoveryRelation(current, previous),
		"target": relation(canonicalValuesEqual(request["expected_target"], target),
			"same_declared_target", "target_mismatch"),
		"temporal": temporalRelation(current, previous,
			request["evaluated_at_unix_ms"].(int64)),
	}, nil
}

func chainRelation(current, previous map[string]any) string {
	transition := current["transition"].(map[string]any)
	initial := current["sequence"] == int64(1) && previous == nil &&
		current["previous_receipt_id"] == nil && current["previous_receipt_sha256"] == nil &&
		transition["from_state"] == "DRAFT"
	if initial {
		return "initial_declared_chain"
	}
	if previous == nil || current["sequence"] != previous["sequence"].(int64)+1 {
		return "predecessor_mismatch"
	}
	sameIdentity := current["previous_receipt_id"] == previous["receipt_id"] &&
		current["previous_receipt_sha256"] == previous["receipt_sha256"]
	currentTask := current["task_binding"].(map[string]any)
	previousTask := previous["task_binding"].(map[string]any)
	sameScope := current["work_id"] == previous["work_id"] &&
		currentTask["project_id"] == previousTask["project_id"] &&
		currentTask["change_id"] == previousTask["change_id"]
	return relation(sameIdentity && sameScope, "same_declared_predecessor", "predecessor_mismatch")
}

func continuityRelation(current, previous map[string]any) string {
	source := current["transition"].(map[string]any)["from_state"]
	consistent := source == "DRAFT"
	if previous != nil {
		consistent = previous["transition"].(map[string]any)["to_state"] == source
	}
	return relation(consistent, "same_declared_state_continuity", "state_continuity_mismatch")
}

func edgeRelation(current, previous map[string]any) string {
	transition := current["transition"].(map[string]any)
	source := transition["from_state"].(string)
	target := transition["to_state"].(string)
	listed := listedEdge(source, target)
	if previous != nil && isSuspendedState(source) &&
		previous["transition"].(map[string]any)["to_state"] == source {
		resume := previous["transition"].(map[string]any)["resume_state"]
		listed = listed || target == resume
	}
	return relation(listed, "listed_declared_edge", "unlisted_declared_edge")
}

func preconditionsRelation(receipt map[string]any) string {
	for _, value := range receipt["preconditions"].([]any) {
		result := value.(map[string]any)["result"]
		if result == "FAIL" || result == "UNKNOWN" {
			return "declared_fail_or_unknown_present"
		}
	}
	return "declared_pass_or_na_only"
}

func temporalRelation(current, previous map[string]any, evaluated int64) string {
	declaredAt := current["transition"].(map[string]any)["declared_at_unix_ms"].(int64)
	consistent := declaredAt <= evaluated
	if previous != nil {
		previousAt := previous["transition"].(map[string]any)["declared_at_unix_ms"].(int64)
		consistent = consistent && previousAt <= declaredAt
	}
	return relation(consistent, "nondecreasing_declared_time", "temporal_declaration_mismatch")
}

func relation(consistent bool, positive, negative string) string {
	if consistent {
		return positive
	}
	return negative
}

func relationReasons(relations map[string]any) []string {
	reasons := make([]string, 0, len(relations))
	for _, value := range relations {
		if text, ok := value.(string); ok && negativeRelations[text] {
			reasons = append(reasons, text)
		}
	}
	sort.Strings(reasons)
	return reasons
}
