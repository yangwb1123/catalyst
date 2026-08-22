package capabilitygrantcontract

import (
	"sort"
)

const assessedDeclarationsOnly = "ASSESSED_DECLARATIONS_ONLY (no issuer authentication, policy decision, approval, revocation, usage, preflight, authorization, permission, persistence, execution, or effect attestation)"

var relationKeys = []string{"binding", "budget", "capability", "effect", "scope", "subject", "task", "temporal"}

func assessDeclared(vocabulary, request map[string]any) (map[string]any, error) {
	if err := validateVocabulary(vocabulary); err != nil {
		return nil, err
	}
	if err := validateAssessmentRequest(request); err != nil {
		return nil, err
	}
	relations, reasons := evaluateRelations(request)
	grant, _ := objectValue(request, "grant")
	action, _ := objectValue(request, "requested_action")
	actionHash, err := requestedActionDigest(action)
	if err != nil {
		return nil, err
	}
	assessment := baseAssessment(grant, request, actionHash, relations, reasons)
	assessment["assessment_sha256"] = ""
	digest, err := digestNode(assessmentDigestDomain, assessment)
	if err != nil {
		return nil, err
	}
	assessment["assessment_sha256"] = digest
	return assessment, nil
}

func baseAssessment(grant, request map[string]any, actionHash string, relations map[string]any,
	reasons []any) map[string]any {
	return map[string]any{
		"api_version":    "forgeos.capability-grant-declared-assessment/v1",
		"approval_state": "not_evaluated", "assessment_mode": "authority_neutral_declared_envelope_only",
		"assessment_sha256": "", "authority_proof_state": "not_evaluated",
		"authorization_decision": "none", "canonicalization": "forgeos.canonical-json/v1",
		"effect_attestation": false, "grant_id": grant["grant_id"], "grant_sha256": grant["grant_sha256"],
		"permission_attestation": false, "reason_codes": reasons, "relations": relations,
		"request_sha256": request["request_sha256"], "requested_action_sha256": actionHash,
		"result": assessedDeclarationsOnly, "revocation_state": "not_evaluated", "usage_state": "not_evaluated",
	}
}

func evaluateRelations(request map[string]any) (map[string]any, []any) {
	grant, _ := objectValue(request, "grant")
	expected, _ := objectValue(request, "expected")
	action, _ := objectValue(request, "requested_action")
	relations := make(map[string]any, len(relationKeys))
	reasons := make([]string, 0)
	compareRelation(relations, &reasons, "binding", grant["bindings"], expected["bindings"],
		"same_declared_binding", "binding_mismatch")
	compareRelation(relations, &reasons, "capability", grant["capability"], expected["capability"],
		"same_declared_capability", "capability_mismatch")
	compareRelation(relations, &reasons, "subject", grant["subject"], expected["subject"],
		"same_declared_subject", "subject_mismatch")
	compareRelation(relations, &reasons, "task", grant["task_binding"], expected["task_binding"],
		"same_declared_task", "task_mismatch")
	evaluateEffectAndScope(relations, &reasons, grant, action)
	evaluateBudgetAndTime(relations, &reasons, grant, action, request)
	sort.Strings(reasons)
	return relations, stringsToAny(reasons)
}

func compareRelation(relations map[string]any, reasons *[]string, key string, left, right any,
	positive, negative string) {
	if canonicalValuesEqual(left, right) {
		relations[key] = positive
		return
	}
	relations[key] = negative
	addReason(reasons, negative)
}

func evaluateEffectAndScope(relations map[string]any, reasons *[]string, grant, action map[string]any) {
	scope, _ := objectValue(grant, "scope")
	grantEffect, _ := stringValue(scope, "effect_id")
	actionEffect, _ := stringValue(action, "effect_id")
	if grantEffect == actionEffect {
		relations["effect"] = "same_declared_effect"
	} else {
		relations["effect"] = "effect_mismatch"
		addReason(reasons, "effect_mismatch")
	}
	scopeRelation, scopeReason := assessScope(scope, action)
	relations["scope"] = scopeRelation
	addReason(reasons, scopeReason)
}

func evaluateBudgetAndTime(relations map[string]any, reasons *[]string, grant, action,
	request map[string]any) {
	budget, _ := objectValue(grant, "budget")
	usage, _ := objectValue(action, "usage")
	if usageWithinBudget(usage, budget) {
		relations["budget"] = "at_or_below_declared_ceiling"
	} else {
		relations["budget"] = "exceeds_declared_ceiling"
		addReason(reasons, "budget_exceeded")
	}
	validity, _ := objectValue(grant, "validity")
	evaluated, _ := intValue(request, "evaluated_at_unix_ms")
	if insideValidity(evaluated, validity) {
		relations["temporal"] = "inside_declared_window"
	} else {
		relations["temporal"] = "outside_declared_window"
		addReason(reasons, "temporal_window_mismatch")
	}
}

func usageWithinBudget(usage, budget map[string]any) bool {
	pairs := [][2]string{
		{"call_count", "max_calls"}, {"cost_usd_micros", "max_cost_usd_micros"},
		{"input_tokens", "max_input_tokens"}, {"network_bytes", "max_network_bytes"},
		{"output_bytes", "max_output_bytes"}, {"output_tokens", "max_output_tokens"},
		{"timeout_ms", "timeout_ms"},
	}
	for _, pair := range pairs {
		used, _ := intValue(usage, pair[0])
		ceiling, _ := intValue(budget, pair[1])
		if used > ceiling {
			return false
		}
	}
	return true
}

func insideValidity(evaluated int64, validity map[string]any) bool {
	notBefore, _ := intValue(validity, "not_before_unix_ms")
	expires, _ := intValue(validity, "expires_at_unix_ms")
	return evaluated >= notBefore && evaluated < expires
}

func canonicalValuesEqual(left, right any) bool {
	leftBytes, leftErr := canonicalJSON(left)
	rightBytes, rightErr := canonicalJSON(right)
	return leftErr == nil && rightErr == nil && string(leftBytes) == string(rightBytes)
}

func addReason(reasons *[]string, reason string) {
	if reason == "" || containsString(*reasons, reason) {
		return
	}
	*reasons = append(*reasons, reason)
}

func stringsToAny(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}
