package transitionreceiptcontract

import "fmt"

var assessmentKeys = []string{
	"api_version", "approval_state", "assessment_mode", "assessment_sha256",
	"authorization_decision", "canonicalization", "completion_attestation",
	"controller_authentication_state", "effect_attestation", "evidence_state",
	"execution_attestation", "expected_target_sha256", "grant_state", "ledger_state",
	"permission_attestation", "persistence_attestation", "policy_decision",
	"precondition_truth_state", "reason_codes", "receipt_id", "receipt_sha256", "relations",
	"request_sha256", "result", "transition_attestation", "transition_vocabulary_sha256",
	"waiver_state",
}

var relationOptions = map[string][]string{
	"applicability": {"internally_consistent_declared_applicability"},
	"chain":         {"initial_declared_chain", "predecessor_mismatch", "same_declared_predecessor"},
	"continuity":    {"same_declared_state_continuity", "state_continuity_mismatch"},
	"edge":          {"listed_declared_edge", "unlisted_declared_edge"},
	"preconditions": {"declared_fail_or_unknown_present", "declared_pass_or_na_only"},
	"recovery":      {"internally_consistent_declared_recovery", "rework_or_resume_mismatch"},
	"target":        {"same_declared_target", "target_mismatch"},
	"temporal":      {"nondecreasing_declared_time", "temporal_declaration_mismatch"},
}

func validateAssessment(assessment map[string]any) error {
	if err := requireKeys(assessment, assessmentKeys...); err != nil {
		return fmt.Errorf("Transition declared assessment: %w", err)
	}
	if err := validateCanonicalByteLimit(assessment, maxAssessmentBytes,
		"Transition declared assessment"); err != nil {
		return err
	}
	if err := validateAssessmentLiterals(assessment); err != nil {
		return err
	}
	if err := validateAssessmentIdentity(assessment); err != nil {
		return err
	}
	relations, err := objectValue(assessment, "relations")
	if err != nil || validateRelations(relations) != nil {
		return fmt.Errorf("Transition declared assessment relations are invalid")
	}
	if err := validateAssessmentReasons(assessment, relations); err != nil {
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
		"api_version": assessmentAPI, "approval_state": "not_evaluated",
		"assessment_mode": assessmentMode, "authorization_decision": "none",
		"canonicalization": canonicalization, "controller_authentication_state": "not_evaluated",
		"evidence_state": "not_evaluated", "grant_state": "not_evaluated",
		"ledger_state": "not_evaluated", "policy_decision": "none",
		"precondition_truth_state": "not_evaluated", "result": assessedDeclarationsOnly,
		"waiver_state": "not_evaluated",
	}
	for key, expected := range literals {
		value, err := stringValue(assessment, key)
		if err != nil || value != expected {
			return fmt.Errorf("%s must equal %q", key, expected)
		}
	}
	return validateFalseAttestations(assessment)
}

func validateFalseAttestations(assessment map[string]any) error {
	keys := []string{"completion_attestation", "effect_attestation", "execution_attestation",
		"permission_attestation", "persistence_attestation", "transition_attestation"}
	for _, key := range keys {
		value, err := boolValue(assessment, key)
		if err != nil || value {
			return fmt.Errorf("%s must be false", key)
		}
	}
	return nil
}

func validateAssessmentIdentity(assessment map[string]any) error {
	for _, key := range []string{"assessment_sha256", "expected_target_sha256", "receipt_sha256",
		"request_sha256", "transition_vocabulary_sha256"} {
		value, err := stringValue(assessment, key)
		if err != nil || validateHash(value, key) != nil {
			return fmt.Errorf("%s is invalid", key)
		}
	}
	identifier, err := stringValue(assessment, "receipt_id")
	if err != nil || identifier != "transition-receipt-"+assessment["receipt_sha256"].(string) {
		return fmt.Errorf("assessment TransitionReceipt identity is invalid")
	}
	vocabulary, err := authoredVocabulary()
	if err != nil || assessment["transition_vocabulary_sha256"] != vocabulary["vocabulary_sha256"] {
		return fmt.Errorf("assessment does not bind the frozen Transition vocabulary")
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
	reasons, err := readStringArray(assessment, "reason_codes", 0, len(negativeRelations))
	if err != nil {
		return err
	}
	expected := relationReasons(relations)
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
