package approvalrecordcontract

import "fmt"

var assessmentKeys = []string{
	"api_version", "approval_id", "approval_sha256", "approver_identity_state",
	"assessment_mode", "assessment_sha256", "authority_proof_state", "authorization_decision",
	"canonicalization", "condition_satisfaction_state", "effect_attestation",
	"effective_approval_state", "expected_target_sha256", "permission_attestation",
	"persistence_attestation", "policy_decision", "reason_codes", "relations", "request_sha256",
	"result", "revocation_registry_state", "risk_acceptance_state",
	"separation_of_duty_proof_state", "transition_attestation",
}

var relationOptions = map[string][]string{
	"approver":           {"approver_mismatch", "same_declared_approver"},
	"authority_binding":  {"authority_binding_mismatch", "same_declared_authority_binding"},
	"binding":            {"binding_mismatch", "same_declared_binding"},
	"conditions":         {"conditions_mismatch", "same_declared_conditions"},
	"decision":           {"decision_mismatch", "same_declared_decision"},
	"revocation":         {"declared_revocation_time_not_reached", "declared_revocation_time_reached"},
	"risk_acceptance":    {"risk_acceptance_mismatch", "same_declared_risk_acceptance_refs"},
	"scope":              {"same_declared_scope", "scope_mismatch"},
	"separation_of_duty": {"same_declared_separation_of_duty", "separation_of_duty_mismatch"},
	"subject":            {"same_declared_subject", "subject_mismatch"},
	"temporal":           {"inside_declared_window", "outside_declared_window"},
}

var relationReasons = map[string]string{
	"approver_mismatch": "approver_mismatch", "authority_binding_mismatch": "authority_binding_mismatch",
	"binding_mismatch": "binding_mismatch", "conditions_mismatch": "conditions_mismatch",
	"declared_revocation_time_reached": "declared_revocation_time_reached",
	"decision_mismatch":                "decision_mismatch", "risk_acceptance_mismatch": "risk_acceptance_mismatch",
	"scope_mismatch": "scope_mismatch", "separation_of_duty_mismatch": "separation_of_duty_mismatch",
	"subject_mismatch": "subject_mismatch", "outside_declared_window": "temporal_window_mismatch",
}

func validateAssessment(assessment map[string]any) error {
	if err := requireKeys(assessment, assessmentKeys...); err != nil {
		return fmt.Errorf("declared assessment: %w", err)
	}
	if err := validateAssessmentLiterals(assessment); err != nil {
		return err
	}
	if err := validateAssessmentIdentity(assessment); err != nil {
		return err
	}
	relations, err := objectValue(assessment, "relations")
	if err != nil || validateRelations(relations) != nil {
		return fmt.Errorf("declared assessment relations are invalid")
	}
	if err := validateAssessmentReasons(assessment, relations); err != nil {
		return err
	}
	if err := validateCanonicalByteLimit(assessment, maxAssessmentBytes, "declared assessment"); err != nil {
		return err
	}
	claimed, _ := stringValue(assessment, "assessment_sha256")
	computed, err := assessmentDigest(assessment)
	if err != nil || computed != claimed {
		return fmt.Errorf("assessment_sha256 does not match canonical assessment")
	}
	return nil
}

func validateAssessmentLiterals(assessment map[string]any) error {
	literals := map[string]string{
		"api_version": assessmentAPI, "approver_identity_state": "not_evaluated",
		"assessment_mode": assessmentMode, "authority_proof_state": "not_evaluated",
		"authorization_decision": "none", "canonicalization": canonicalization,
		"condition_satisfaction_state": "not_evaluated", "effective_approval_state": "not_evaluated",
		"policy_decision": "none", "result": assessedDeclarationsOnly,
		"revocation_registry_state": "not_evaluated", "risk_acceptance_state": "not_evaluated",
		"separation_of_duty_proof_state": "not_evaluated",
	}
	for key, expected := range literals {
		value, err := stringValue(assessment, key)
		if err != nil || value != expected {
			return fmt.Errorf("%s must equal %q", key, expected)
		}
	}
	for _, key := range []string{"effect_attestation", "permission_attestation",
		"persistence_attestation", "transition_attestation"} {
		value, err := boolValue(assessment, key)
		if err != nil || value {
			return fmt.Errorf("%s must be false", key)
		}
	}
	return nil
}

func validateAssessmentIdentity(assessment map[string]any) error {
	for _, key := range []string{"approval_sha256", "assessment_sha256", "expected_target_sha256",
		"request_sha256"} {
		value, err := stringValue(assessment, key)
		if err != nil || validateHash(value, key) != nil {
			return fmt.Errorf("%s is invalid", key)
		}
	}
	identifier, err := stringValue(assessment, "approval_id")
	if err != nil || identifier != "approval-record-"+assessment["approval_sha256"].(string) {
		return fmt.Errorf("assessment approval identity is invalid")
	}
	return nil
}

func validateRelations(relations map[string]any) error {
	if err := requireKeys(relations, relationKeys...); err != nil {
		return err
	}
	for _, key := range relationKeys {
		value, err := stringValue(relations, key)
		if err != nil || validateEnum(value, "relation "+key, relationOptions[key]...) != nil {
			return fmt.Errorf("relation %s is invalid", key)
		}
	}
	return nil
}

func validateAssessmentReasons(assessment, relations map[string]any) error {
	reasons, err := readStringArray(assessment, "reason_codes", 0, len(relationReasons))
	if err != nil {
		return err
	}
	expected := make([]string, 0, len(relationReasons))
	for _, key := range relationKeys {
		if reason := relationReasons[relations[key].(string)]; reason != "" {
			expected = append(expected, reason)
		}
	}
	if len(reasons) != len(expected) {
		return fmt.Errorf("reason_codes do not match declared relations")
	}
	for index := range reasons {
		if reasons[index] != expected[index] {
			return fmt.Errorf("reason_codes do not match declared relations")
		}
	}
	return nil
}
