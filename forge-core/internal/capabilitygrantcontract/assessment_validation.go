package capabilitygrantcontract

import (
	"fmt"
	"slices"
	"sort"
)

var assessmentKeys = []string{
	"api_version", "approval_state", "assessment_mode", "assessment_sha256", "authority_proof_state",
	"authorization_decision", "canonicalization", "effect_attestation", "grant_id", "grant_sha256",
	"permission_attestation", "reason_codes", "relations", "request_sha256", "requested_action_sha256",
	"result", "revocation_state", "usage_state",
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
	if err := validateAssessmentReasons(assessment); err != nil {
		return err
	}
	if err := validateRelations(assessment); err != nil {
		return err
	}
	if err := validateCanonicalByteLimit(assessment, maxAssessmentBytes, "declared assessment"); err != nil {
		return err
	}
	return validateAssessmentDigest(assessment)
}

func validateAssessmentLiterals(assessment map[string]any) error {
	literals := map[string]string{
		"api_version":    "forgeos.capability-grant-declared-assessment/v1",
		"approval_state": "not_evaluated", "assessment_mode": "authority_neutral_declared_envelope_only",
		"authority_proof_state": "not_evaluated", "authorization_decision": "none",
		"canonicalization": "forgeos.canonical-json/v1", "result": assessedDeclarationsOnly,
		"revocation_state": "not_evaluated", "usage_state": "not_evaluated",
	}
	for key, expected := range literals {
		if err := requireStringLiteral(assessment, key, expected); err != nil {
			return err
		}
	}
	for _, key := range []string{"effect_attestation", "permission_attestation"} {
		value, err := boolValue(assessment, key)
		if err != nil || value {
			return fmt.Errorf("%s must be false", key)
		}
	}
	return nil
}

func validateAssessmentIdentity(assessment map[string]any) error {
	for _, key := range []string{"assessment_sha256", "grant_sha256", "request_sha256",
		"requested_action_sha256"} {
		value, err := stringValue(assessment, key)
		if err != nil || validateHash(value, key) != nil {
			return fmt.Errorf("%s is invalid", key)
		}
	}
	grantID, idErr := stringValue(assessment, "grant_id")
	grantHash, _ := stringValue(assessment, "grant_sha256")
	if idErr != nil || grantID != "capability-grant-"+grantHash {
		return fmt.Errorf("assessment grant_id is not derived from grant_sha256")
	}
	return nil
}

func validateAssessmentReasons(assessment map[string]any) error {
	reasons, err := readStringArray(assessment, "reason_codes", 0, 9)
	if err != nil {
		return err
	}
	allowed := []string{"binding_mismatch", "budget_exceeded", "capability_mismatch", "deny_matched",
		"effect_mismatch", "scope_not_covered", "subject_mismatch", "task_mismatch",
		"temporal_window_mismatch"}
	for _, reason := range reasons {
		if !containsString(allowed, reason) {
			return fmt.Errorf("unsupported reason code %q", reason)
		}
	}
	return nil
}

func validateRelations(assessment map[string]any) error {
	relations, err := objectValue(assessment, "relations")
	if err != nil || requireKeys(relations, relationKeys...) != nil {
		return fmt.Errorf("relations fields mismatch")
	}
	allowed := map[string][]string{
		"binding":    {"binding_mismatch", "same_declared_binding"},
		"budget":     {"at_or_below_declared_ceiling", "exceeds_declared_ceiling"},
		"capability": {"capability_mismatch", "same_declared_capability"},
		"effect":     {"effect_mismatch", "same_declared_effect"},
		"scope":      {"covered_by_declaration", "denied_by_declaration", "outside_declared_scope"},
		"subject":    {"same_declared_subject", "subject_mismatch"},
		"task":       {"same_declared_task", "task_mismatch"},
		"temporal":   {"inside_declared_window", "outside_declared_window"},
	}
	for key, options := range allowed {
		value, err := stringValue(relations, key)
		if err != nil || !containsString(options, value) {
			return fmt.Errorf("relation %s is invalid", key)
		}
	}
	if relations["effect"] == "effect_mismatch" && relations["scope"] != "outside_declared_scope" {
		return fmt.Errorf("effect_mismatch requires outside_declared_scope")
	}
	return validateReasonsMatchRelations(assessment, relations)
}

func validateReasonsMatchRelations(assessment, relations map[string]any) error {
	expected := make([]string, 0, 9)
	mapping := map[string]string{
		"binding_mismatch": "binding_mismatch", "exceeds_declared_ceiling": "budget_exceeded",
		"capability_mismatch": "capability_mismatch", "effect_mismatch": "effect_mismatch",
		"denied_by_declaration": "deny_matched", "subject_mismatch": "subject_mismatch",
		"task_mismatch": "task_mismatch", "outside_declared_window": "temporal_window_mismatch",
	}
	for _, key := range relationKeys {
		value, _ := stringValue(relations, key)
		if reason := mapping[value]; reason != "" {
			addReason(&expected, reason)
		}
	}
	if relations["scope"] == "outside_declared_scope" && relations["effect"] == "same_declared_effect" {
		addReason(&expected, "scope_not_covered")
	}
	sort.Strings(expected)
	actual, _ := readStringArray(assessment, "reason_codes", 0, 9)
	if !slices.Equal(actual, expected) {
		return fmt.Errorf("reason_codes do not match declared relations")
	}
	return nil
}

func validateAssessmentDigest(assessment map[string]any) error {
	claimed, _ := stringValue(assessment, "assessment_sha256")
	preimage := cloneNode(assessment)
	preimage["assessment_sha256"] = ""
	computed, err := digestNode(assessmentDigestDomain, preimage)
	if err != nil || computed != claimed {
		return fmt.Errorf("assessment_sha256 does not match canonical assessment")
	}
	return nil
}

func validateAssessmentAgainstRequest(vocabulary, request, assessment map[string]any) error {
	expected, err := assessDeclared(vocabulary, request)
	if err != nil {
		return err
	}
	if !canonicalValuesEqual(expected, assessment) {
		return fmt.Errorf("assessment differs from fresh authority-neutral declared assessment")
	}
	return nil
}
