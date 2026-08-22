package approvalrecordcontract

import (
	"fmt"
	"sort"
)

var relationKeys = []string{
	"approver", "authority_binding", "binding", "conditions", "decision", "revocation",
	"risk_acceptance", "scope", "separation_of_duty", "subject", "temporal",
}

func assessDeclared(request map[string]any) (map[string]any, error) {
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	record := request["approval_record"].(map[string]any)
	expected := request["expected_target"].(map[string]any)
	actual, err := declaredTarget(record)
	if err != nil {
		return nil, err
	}
	relations, reasons := compareDeclaredTarget(expected, actual)
	evaluated := request["evaluated_at_unix_ms"].(int64)
	compareTemporal(record["validity"].(map[string]any), evaluated, relations, &reasons)
	assessment := baseAssessment(request, record, relations, reasons)
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

func compareDeclaredTarget(expected, actual map[string]any) (map[string]any, []string) {
	definitions := []struct{ key, positive, negative string }{
		{"approver", "same_declared_approver", "approver_mismatch"},
		{"authority_binding", "same_declared_authority_binding", "authority_binding_mismatch"},
		{"bindings", "same_declared_binding", "binding_mismatch"},
		{"conditions", "same_declared_conditions", "conditions_mismatch"},
		{"decision", "same_declared_decision", "decision_mismatch"},
		{"risk_acceptance_refs", "same_declared_risk_acceptance_refs", "risk_acceptance_mismatch"},
		{"scope", "same_declared_scope", "scope_mismatch"},
		{"separation_of_duty_declaration", "same_declared_separation_of_duty", "separation_of_duty_mismatch"},
		{"subject", "same_declared_subject", "subject_mismatch"},
	}
	relations := make(map[string]any, len(relationKeys))
	reasons := make([]string, 0, len(definitions)+2)
	for _, definition := range definitions {
		relationKey := definition.key
		if relationKey == "risk_acceptance_refs" {
			relationKey = "risk_acceptance"
		} else if relationKey == "separation_of_duty_declaration" {
			relationKey = "separation_of_duty"
		} else if relationKey == "bindings" {
			relationKey = "binding"
		}
		if canonicalValuesEqual(expected[definition.key], actual[definition.key]) {
			relations[relationKey] = definition.positive
		} else {
			relations[relationKey] = definition.negative
			reasons = append(reasons, definition.negative)
		}
	}
	return relations, reasons
}

func compareTemporal(validity map[string]any, evaluated int64, relations map[string]any,
	reasons *[]string) {
	notBefore := validity["not_before_unix_ms"].(int64)
	expires := validity["expires_at_unix_ms"].(int64)
	if evaluated >= notBefore && evaluated < expires {
		relations["temporal"] = "inside_declared_window"
	} else {
		relations["temporal"] = "outside_declared_window"
		*reasons = append(*reasons, "temporal_window_mismatch")
	}
	revoked := validity["revoked_at_unix_ms"]
	if revoked != nil && evaluated >= revoked.(int64) {
		relations["revocation"] = "declared_revocation_time_reached"
		*reasons = append(*reasons, "declared_revocation_time_reached")
	} else {
		relations["revocation"] = "declared_revocation_time_not_reached"
	}
	sort.Strings(*reasons)
}

func baseAssessment(request, record, relations map[string]any, reasons []string) map[string]any {
	return map[string]any{
		"api_version": assessmentAPI, "approval_id": record["approval_id"],
		"approval_sha256": record["approval_sha256"], "approver_identity_state": "not_evaluated",
		"assessment_mode": assessmentMode, "assessment_sha256": "",
		"authority_proof_state": "not_evaluated", "authorization_decision": "none",
		"canonicalization": canonicalization, "condition_satisfaction_state": "not_evaluated",
		"effect_attestation": false, "effective_approval_state": "not_evaluated",
		"expected_target_sha256": request["expected_target_sha256"], "permission_attestation": false,
		"persistence_attestation": false, "policy_decision": "none", "reason_codes": stringsToAny(reasons),
		"relations": relations, "request_sha256": request["request_sha256"], "result": assessedDeclarationsOnly,
		"revocation_registry_state": "not_evaluated", "risk_acceptance_state": "not_evaluated",
		"separation_of_duty_proof_state": "not_evaluated", "transition_attestation": false,
	}
}

func canonicalValuesEqual(left, right any) bool {
	leftBytes, leftErr := canonicalJSON(left)
	rightBytes, rightErr := canonicalJSON(right)
	return leftErr == nil && rightErr == nil && string(leftBytes) == string(rightBytes)
}

func stringsToAny(values []string) []any {
	result := make([]any, len(values))
	for index, value := range values {
		result[index] = value
	}
	return result
}

func assessmentDigest(assessment map[string]any) (string, error) {
	preimage := cloneNode(assessment)
	preimage["assessment_sha256"] = ""
	return digestNode(assessmentDomain, preimage)
}

func validateAssessmentAgainstRequest(request, assessment map[string]any) error {
	expected, err := assessDeclared(request)
	if err != nil {
		return err
	}
	if !canonicalValuesEqual(expected, assessment) {
		return fmt.Errorf("assessment differs from fresh authority-neutral declared assessment")
	}
	return nil
}
