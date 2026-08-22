package knowledgeupdateproposalcontract

import (
	"fmt"
	"sort"
)

var assessmentKeys = []string{
	"api_version", "assessment_mode", "assessment_sha256", "authorization_decision",
	"canonicalization", "conflict_state", "context_state", "current_knowledge_state",
	"effect_attestation", "evidence_state", "execution_attestation", "expected_target_sha256",
	"freshness_state", "grant_state", "knowledge_adoption_attestation", "permission_attestation",
	"persistence_attestation", "policy_decision", "proposal_id", "proposal_sha256",
	"proposer_authentication_state", "reason_codes", "relations", "request_sha256", "result",
	"truth_attestation",
}

var relationKeys = []string{
	"binding", "grant_ref", "mutations", "proposer", "record_set", "scope", "task_binding", "temporal",
}

var relationOptions = map[string][]string{
	"binding":      {"binding_mismatch", "same_declared_binding"},
	"grant_ref":    {"grant_ref_mismatch", "same_declared_grant_ref"},
	"mutations":    {"mutations_mismatch", "same_declared_mutations"},
	"proposer":     {"proposer_mismatch", "same_declared_proposer"},
	"record_set":   {"record_set_mismatch", "same_declared_record_set"},
	"scope":        {"same_declared_scope", "scope_mismatch"},
	"task_binding": {"same_declared_task_binding", "task_binding_mismatch"},
	"temporal":     {"future_declared_submission", "nonfuture_declared_submission"},
}

var relationReason = map[string]string{
	"binding_mismatch": "binding_mismatch", "grant_ref_mismatch": "grant_ref_mismatch",
	"mutations_mismatch": "mutations_mismatch", "proposer_mismatch": "proposer_mismatch",
	"record_set_mismatch": "record_set_mismatch", "scope_mismatch": "scope_mismatch",
	"task_binding_mismatch":      "task_binding_mismatch",
	"future_declared_submission": "temporal_declaration_mismatch",
}

func assessDeclared(request map[string]any) (map[string]any, error) {
	if err := validateRequest(request); err != nil {
		return nil, err
	}
	proposal := request["knowledge_update_proposal"].(map[string]any)
	actual, err := declaredTarget(proposal)
	if err != nil {
		return nil, err
	}
	expected := request["expected_target"].(map[string]any)
	relations := evaluateRelations(expected, actual,
		proposal["submitted_at_unix_ms"].(int64), request["evaluated_at_unix_ms"].(int64))
	reasons := relationReasons(relations)
	assessment := baseAssessment(request, proposal, relations, reasons)
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

func evaluateRelations(expected, actual map[string]any, submitted, evaluated int64) map[string]any {
	return map[string]any{
		"binding":      sameRelation(expected["bindings"], actual["bindings"], "binding"),
		"grant_ref":    sameRelation(expected["capability_grant_ref"], actual["capability_grant_ref"], "grant_ref"),
		"mutations":    sameRelation(expected["mutations"], actual["mutations"], "mutations"),
		"proposer":     sameRelation(expected["proposer"], actual["proposer"], "proposer"),
		"record_set":   sameRelation(expected["record_set_sha256"], actual["record_set_sha256"], "record_set"),
		"scope":        sameRelation(expected["knowledge_scope"], actual["knowledge_scope"], "scope"),
		"task_binding": sameRelation(expected["task_binding"], actual["task_binding"], "task_binding"),
		"temporal":     relation(submitted <= evaluated, "nonfuture_declared_submission", "future_declared_submission"),
	}
}

func sameRelation(left, right any, name string) string {
	return relation(canonicalValuesEqual(left, right), "same_declared_"+name, name+"_mismatch")
}

func relation(matches bool, positive, negative string) string {
	if matches {
		return positive
	}
	return negative
}

func relationReasons(relations map[string]any) []string {
	reasons := make([]string, 0, len(relations))
	for _, value := range relations {
		text, ok := value.(string)
		if ok && relationReason[text] != "" {
			reasons = append(reasons, relationReason[text])
		}
	}
	sort.Strings(reasons)
	return reasons
}

func baseAssessment(request, proposal, relations map[string]any, reasons []string) map[string]any {
	return map[string]any{
		"api_version": assessmentAPI, "assessment_mode": assessmentMode, "assessment_sha256": "",
		"authorization_decision": "none", "canonicalization": canonicalization,
		"conflict_state": "not_evaluated", "context_state": "not_evaluated",
		"current_knowledge_state": "not_evaluated", "effect_attestation": false,
		"evidence_state": "not_evaluated", "execution_attestation": false,
		"expected_target_sha256": request["expected_target_sha256"], "freshness_state": "not_evaluated",
		"grant_state": "not_evaluated", "knowledge_adoption_attestation": false,
		"permission_attestation": false, "persistence_attestation": false,
		"policy_decision": "none", "proposal_id": proposal["proposal_id"],
		"proposal_sha256": proposal["proposal_sha256"], "proposer_authentication_state": "not_evaluated",
		"reason_codes": stringsToAny(reasons), "relations": relations,
		"request_sha256": request["request_sha256"], "result": assessedDeclarationsOnly,
		"truth_attestation": false,
	}
}

func validateAssessment(assessment map[string]any) error {
	if err := validateCanonicalByteLimit(assessment, maxAssessmentBytes, "knowledge update declared assessment"); err != nil {
		return err
	}
	if err := requireKeys(assessment, assessmentKeys...); err != nil {
		return fmt.Errorf("knowledge update declared assessment: %w", err)
	}
	if err := validateAssessmentLiterals(assessment); err != nil {
		return err
	}
	if err := validateAssessmentIdentity(assessment); err != nil {
		return err
	}
	relations, err := objectValue(assessment, "relations")
	if err != nil || validateRelations(relations) != nil {
		return fmt.Errorf("knowledge update declared assessment relations are invalid")
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
		"api_version": assessmentAPI, "assessment_mode": assessmentMode,
		"authorization_decision": "none", "canonicalization": canonicalization,
		"conflict_state": "not_evaluated", "context_state": "not_evaluated",
		"current_knowledge_state": "not_evaluated", "evidence_state": "not_evaluated",
		"freshness_state": "not_evaluated", "grant_state": "not_evaluated",
		"policy_decision": "none", "proposer_authentication_state": "not_evaluated",
		"result": assessedDeclarationsOnly,
	}
	for key, expected := range literals {
		if err := requireStringLiteral(assessment, key, expected); err != nil {
			return err
		}
	}
	for _, key := range []string{"effect_attestation", "execution_attestation", "knowledge_adoption_attestation",
		"permission_attestation", "persistence_attestation", "truth_attestation"} {
		value, err := boolValue(assessment, key)
		if err != nil || value {
			return fmt.Errorf("%s must be false", key)
		}
	}
	return nil
}

func validateAssessmentIdentity(assessment map[string]any) error {
	for _, key := range []string{"assessment_sha256", "expected_target_sha256", "proposal_sha256", "request_sha256"} {
		value, err := stringValue(assessment, key)
		if err != nil || validateHash(value, key) != nil {
			return fmt.Errorf("%s is invalid", key)
		}
	}
	identifier, err := stringValue(assessment, "proposal_id")
	if err != nil || identifier != "knowledge-update-proposal-"+assessment["proposal_sha256"].(string) {
		return fmt.Errorf("assessment proposal identity is invalid")
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
	reasons, err := readStringArray(assessment, "reason_codes", 0, len(relationReason))
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

func assessmentDigest(assessment map[string]any) (string, error) {
	preimage := cloneNode(assessment)
	preimage["assessment_sha256"] = ""
	if err := validateCanonicalByteLimit(preimage, maxAssessmentBytes, "assessment digest preimage"); err != nil {
		return "", err
	}
	return digestValue(assessmentDomain, preimage)
}
