package knowledgeupdateproposalcontract

import "testing"

func TestDeclaredAssessmentReportsEachExactTargetMismatch(t *testing.T) {
	fixture := loadFixture(t)
	tests := []struct {
		name, relation, expected string
		mutate                   func(map[string]any)
	}{
		{"binding", "binding", "binding_mismatch", func(target map[string]any) {
			target["bindings"].(map[string]any)["context_sha256"] = hashOf("a")
		}},
		{"grant_ref", "grant_ref", "grant_ref_mismatch", func(target map[string]any) {
			target["capability_grant_ref"].(map[string]any)["authority_domain"] = "other.domain"
		}},
		{"mutations", "mutations", "mutations_mismatch", func(target map[string]any) {
			target["mutations"].([]any)[0].(map[string]any)["rationale"] = "different declaration"
		}},
		{"proposer", "proposer", "proposer_mismatch", func(target map[string]any) {
			target["proposer"].(map[string]any)["principal_id"] = "other-agent"
		}},
		{"record_set", "record_set", "record_set_mismatch", func(target map[string]any) {
			target["record_set_sha256"] = hashOf("b")
		}},
		{"scope", "scope", "scope_mismatch", func(target map[string]any) {
			target["knowledge_scope"].(map[string]any)["object_scope_sha256"] = hashOf("c")
		}},
		{"task_binding", "task_binding", "task_binding_mismatch", func(target map[string]any) {
			target["task_binding"].(map[string]any)["role"] = "other-role"
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			request := cloneFixtureObject(t, fixture, "assessment_request")
			test.mutate(request["expected_target"].(map[string]any))
			resealTargetRequest(t, request)
			assessment, err := AssessDeclared(request)
			if err != nil {
				t.Fatal(err)
			}
			relations := assessment["relations"].(map[string]any)
			if relations[test.relation] != test.expected {
				t.Fatalf("relation got %v", relations[test.relation])
			}
			reasons := assessment["reason_codes"].([]any)
			if len(reasons) != 1 || reasons[0] != test.expected {
				t.Fatalf("reasons got %v", reasons)
			}
		})
	}
}

func TestDeclaredAssessmentFutureSubmissionUsesTemporalReason(t *testing.T) {
	request := cloneFixtureObject(t, loadFixture(t), "assessment_request")
	request["evaluated_at_unix_ms"] = request["knowledge_update_proposal"].(map[string]any)["submitted_at_unix_ms"].(int64) - 1
	resealRequest(t, request)
	assessment, err := AssessDeclared(request)
	if err != nil {
		t.Fatal(err)
	}
	if assessment["relations"].(map[string]any)["temporal"] != "future_declared_submission" {
		t.Fatalf("temporal relation got %v", assessment["relations"])
	}
	reasons := assessment["reason_codes"].([]any)
	if len(reasons) != 1 || reasons[0] != "temporal_declaration_mismatch" {
		t.Fatalf("temporal reasons got %v", reasons)
	}
}

func TestAssessmentRejectsStaleDigestAndAuthorityAttestations(t *testing.T) {
	fixture := loadFixture(t)
	request := fixtureObject(t, fixture, "assessment_request")
	assessment := cloneFixtureObject(t, fixture, "expected_assessment")
	assessment["truth_attestation"] = true
	if _, err := CanonicalAssessmentJSON(assessment); err == nil {
		t.Fatal("truth attestation accepted")
	}
	assessment = cloneFixtureObject(t, fixture, "expected_assessment")
	assessment["reason_codes"] = []any{"binding_mismatch"}
	if _, err := CanonicalAssessmentJSON(assessment); err == nil {
		t.Fatal("reason drift accepted")
	}
	assessment = cloneFixtureObject(t, fixture, "expected_assessment")
	assessment["assessment_sha256"] = hashOf("f")
	if err := ValidateAssessment(request, assessment); err == nil {
		t.Fatal("stale assessment digest accepted")
	}
}

func hashOf(character string) string {
	result := ""
	for len(result) < 64 {
		result += character
	}
	return result[:64]
}
