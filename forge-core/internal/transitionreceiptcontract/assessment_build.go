package transitionreceiptcontract

func baseAssessment(request, receipt, relations map[string]any, reasons []string) map[string]any {
	return map[string]any{
		"api_version": assessmentAPI, "approval_state": "not_evaluated",
		"assessment_mode": assessmentMode, "assessment_sha256": "",
		"authorization_decision": "none", "canonicalization": canonicalization,
		"completion_attestation": false, "controller_authentication_state": "not_evaluated",
		"effect_attestation": false, "evidence_state": "not_evaluated",
		"execution_attestation": false, "expected_target_sha256": request["expected_target_sha256"],
		"grant_state": "not_evaluated", "ledger_state": "not_evaluated",
		"permission_attestation": false, "persistence_attestation": false,
		"policy_decision": "none", "precondition_truth_state": "not_evaluated",
		"reason_codes": stringsToAny(reasons), "receipt_id": receipt["receipt_id"],
		"receipt_sha256": receipt["receipt_sha256"], "relations": relations,
		"request_sha256": request["request_sha256"], "result": assessedDeclarationsOnly,
		"transition_attestation":       false,
		"transition_vocabulary_sha256": receipt["transition_vocabulary_sha256"],
		"waiver_state":                 "not_evaluated",
	}
}

func assessmentDigest(assessment map[string]any) (string, error) {
	preimage := cloneNode(assessment)
	preimage["assessment_sha256"] = ""
	return digestNode(assessmentDomain, preimage)
}
