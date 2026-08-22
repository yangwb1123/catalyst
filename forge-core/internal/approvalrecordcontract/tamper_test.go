package approvalrecordcontract

import "testing"

func TestSelfDigestsRejectTamperingAtEveryLayer(t *testing.T) {
	fixture := loadGolden(t)
	tests := []struct {
		name string
		node map[string]any
		key  string
		call func(map[string]any) error
	}{
		{"record", cloneNode(fixtureNode(t, fixture, "approval_record")), "approval_sha256",
			func(node map[string]any) error { return validateRecord(node, false) }},
		{"request", cloneNode(fixtureNode(t, fixture, "assessment_request")), "request_sha256",
			validateRequest},
		{"assessment", cloneNode(fixtureNode(t, fixture, "expected_assessment")), "assessment_sha256",
			validateAssessment},
	}
	for _, test := range tests {
		test.node[test.key] = hashOf('f')
		if err := test.call(test.node); err == nil {
			t.Fatalf("%s accepted a forged self digest", test.name)
		}
	}
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	request["expected_target_sha256"] = hashOf('e')
	if err := validateRequest(request); err == nil {
		t.Fatal("request accepted a forged target digest")
	}
}

func TestAssessmentCannotEscalateStateEvenWithFreshSelfDigest(t *testing.T) {
	fixture := loadGolden(t)
	assessment := cloneNode(fixtureNode(t, fixture, "expected_assessment"))
	mutations := []func(map[string]any){
		func(node map[string]any) { node["effective_approval_state"] = "approved" },
		func(node map[string]any) { node["authorization_decision"] = "allow" },
		func(node map[string]any) { node["permission_attestation"] = true },
		func(node map[string]any) { node["condition_satisfaction_state"] = "satisfied" },
	}
	for index, mutate := range mutations {
		candidate := cloneNode(assessment)
		mutate(candidate)
		candidate["assessment_sha256"] = ""
		digest, err := assessmentDigest(candidate)
		if err != nil {
			t.Fatal(err)
		}
		candidate["assessment_sha256"] = digest
		if err := validateAssessment(candidate); err == nil {
			t.Fatalf("authority escalation case %d unexpectedly passed", index)
		}
	}
}

func TestPureAPIsDoNotMutateCallerDocuments(t *testing.T) {
	fixture := loadGolden(t)
	record := fixtureNode(t, fixture, "approval_record")
	request := fixtureNode(t, fixture, "assessment_request")
	recordBefore, _ := canonicalJSON(record)
	requestBefore, _ := canonicalJSON(request)
	if _, err := ApprovalRecordSHA256(record); err != nil {
		t.Fatal(err)
	}
	if _, err := DeclaredTarget(record); err != nil {
		t.Fatal(err)
	}
	if _, err := AssessDeclared(request); err != nil {
		t.Fatal(err)
	}
	recordAfter, _ := canonicalJSON(record)
	requestAfter, _ := canonicalJSON(request)
	if string(recordBefore) != string(recordAfter) || string(requestBefore) != string(requestAfter) {
		t.Fatal("pure contract API mutated caller-owned input")
	}
}

func TestApprovalRefRejectsIdentitySubstitution(t *testing.T) {
	fixture := loadGolden(t)
	record := fixtureNode(t, fixture, "approval_record")
	reference := cloneNode(fixtureNode(t, fixture, "expected_approval_ref"))
	reference["approval_sha256"] = hashOf('d')
	if _, err := ApprovalRefRelation(record, reference); err == nil {
		t.Fatal("ApprovalRef accepted an ID/hash inconsistency")
	}
	validMismatch := cloneNode(fixtureNode(t, fixture, "expected_approval_ref"))
	validMismatch["approval_sha256"] = hashOf('d')
	validMismatch["approval_id"] = "approval-record-" + hashOf('d')
	relation, err := ApprovalRefRelation(record, validMismatch)
	if err != nil || relation != "reference_mismatch" {
		t.Fatalf("valid mismatch = %q, %v", relation, err)
	}
}

func hashOf(value byte) string {
	result := make([]byte, 64)
	for index := range result {
		result[index] = value
	}
	return string(result)
}
