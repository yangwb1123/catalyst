package approvalrecordcontract

import (
	"slices"
	"strings"
	"testing"
)

func TestProofBytesAreExcludedOnlyFromRecordIdentity(t *testing.T) {
	fixture := loadGolden(t)
	record := cloneNode(fixtureNode(t, fixture, "approval_record"))
	originalHash, err := ApprovalRecordSHA256(record)
	if err != nil {
		t.Fatal(err)
	}
	record["authority_proof"].(map[string]any)["proof_base64url"] =
		"YXV0aG9yaXR5LXByb29mLXJlcGxhY2Vk"
	record["separation_of_duty"].(map[string]any)["proof_base64url"] =
		"c29kLXByb29mLXJlcGxhY2Vk"
	changedHash, err := ApprovalRecordSHA256(record)
	if err != nil || changedHash != originalHash {
		t.Fatalf("record digest changed with proof bytes: %q, %v", changedHash, err)
	}
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	originalRequestHash := request["request_sha256"]
	request["approval_record"] = record
	resealRequest(t, request)
	if request["request_sha256"] == originalRequestHash {
		t.Fatal("request digest failed to bind complete proof bytes")
	}
}

func TestAllDeclaredMismatchesStayAuthorityNeutral(t *testing.T) {
	fixture := loadGolden(t)
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	record := request["approval_record"].(map[string]any)
	validity := record["validity"].(map[string]any)
	validity["revoked_at_unix_ms"] = validity["issued_at_unix_ms"]
	resealRecord(t, record)
	target := request["expected_target"].(map[string]any)
	mutateEveryTargetRelation(target)
	request["evaluated_at_unix_ms"] = validity["expires_at_unix_ms"]
	resealRequest(t, request)
	assessment, err := AssessDeclared(request)
	if err != nil {
		t.Fatal(err)
	}
	want := []any{"approver_mismatch", "authority_binding_mismatch", "binding_mismatch",
		"conditions_mismatch", "decision_mismatch", "declared_revocation_time_reached",
		"risk_acceptance_mismatch", "scope_mismatch", "separation_of_duty_mismatch",
		"subject_mismatch", "temporal_window_mismatch"}
	if !slices.Equal(assessment["reason_codes"].([]any), want) {
		t.Fatalf("reason_codes = %#v", assessment["reason_codes"])
	}
	assertAuthorityNeutral(t, assessment)
}

func mutateEveryTargetRelation(target map[string]any) {
	target["approver"].(map[string]any)["principal_id"] = "different-approver"
	target["authority_binding"].(map[string]any)["key_id"] = "different-key"
	target["bindings"].(map[string]any)["source_revision"] = "different-revision"
	target["conditions"] = []any{}
	target["decision"] = "reject"
	target["risk_acceptance_refs"] = []any{}
	target["scope"].(map[string]any)["change_id"] = "different-change"
	sod := target["separation_of_duty_declaration"].(map[string]any)
	sod["requester"].(map[string]any)["principal_id"] = "different-requester"
	target["subject"].(map[string]any)["principal_id"] = "different-subject"
}

func assertAuthorityNeutral(t *testing.T, assessment map[string]any) {
	t.Helper()
	for _, key := range []string{"approver_identity_state", "authority_proof_state",
		"condition_satisfaction_state", "effective_approval_state", "revocation_registry_state",
		"risk_acceptance_state", "separation_of_duty_proof_state"} {
		if assessment[key] != "not_evaluated" {
			t.Fatalf("%s escalated to %#v", key, assessment[key])
		}
	}
	for _, key := range []string{"effect_attestation", "permission_attestation",
		"persistence_attestation", "transition_attestation"} {
		if assessment[key] != false {
			t.Fatalf("%s became true", key)
		}
	}
	if assessment["authorization_decision"] != "none" || assessment["policy_decision"] != "none" {
		t.Fatal("contract-only assessment made an authority decision")
	}
}

func TestRecordPolicyAndSoDContradictionsFailClosed(t *testing.T) {
	fixture := loadGolden(t)
	tests := []func(map[string]any){
		func(record map[string]any) {
			sod := record["separation_of_duty"].(map[string]any)
			sod["requester"] = cloneValue(record["approver"])
		},
		func(record map[string]any) {
			record["separation_of_duty"].(map[string]any)["implementers"] = []any{}
		},
		func(record map[string]any) {
			source := record["authority_proof"].(map[string]any)["authority_source"].(map[string]any)
			source["authority_class"], source["principal_type"] = "forgeos_kernel", "service"
		},
		func(record map[string]any) {
			artifacts := record["bindings"].(map[string]any)["artifacts"].([]any)
			record["bindings"].(map[string]any)["artifacts"] = append(artifacts, cloneValue(artifacts[0]))
		},
	}
	for index, mutate := range tests {
		record := cloneNode(fixtureNode(t, fixture, "approval_record"))
		mutate(record)
		if err := validateRecord(record, false); err == nil {
			t.Fatalf("malformed case %d unexpectedly passed", index)
		}
	}
}

func TestDeclaredTargetInternalContradictionsFailClosed(t *testing.T) {
	fixture := loadGolden(t)
	tests := []func(map[string]any){
		func(target map[string]any) {
			sod := target["separation_of_duty_declaration"].(map[string]any)
			sod["requester"] = cloneValue(target["approver"])
		},
		func(target map[string]any) {
			target["separation_of_duty_declaration"].(map[string]any)["implementers"] = []any{}
		},
		func(target map[string]any) {
			target["scope"].(map[string]any)["materiality_level"] = "L2"
			sod := target["separation_of_duty_declaration"].(map[string]any)
			sod["required_distinctions"] = []any{"approver_not_implementer", "approver_not_subject"}
		},
		func(target map[string]any) {
			source := target["authority_binding"].(map[string]any)["authority_source"].(map[string]any)
			source["authority_class"], source["principal_id"] = "forgeos_kernel", "kernel"
			source["principal_type"] = "service"
		},
	}
	for index, mutate := range tests {
		request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
		target := request["expected_target"].(map[string]any)
		mutate(target)
		if _, err := DeclaredTargetSHA256(target); err == nil {
			t.Fatalf("internally contradictory target %d unexpectedly passed", index)
		}
		if err := validateRequest(request); err == nil {
			t.Fatalf("request with internally contradictory target %d unexpectedly passed", index)
		}
	}
}

func TestEveryDocumentCeilingAppliesToProgrammaticInput(t *testing.T) {
	limits := []int{maxAssessmentBytes, maxRecordBytes, maxTargetBytes, maxRequestBytes}
	for _, maximum := range limits {
		document := oversizedCanonicalDocument(maximum)
		if err := validateCanonicalByteLimit(document, maximum, "programmatic"); err == nil {
			t.Fatalf("programmatic document exceeded %d bytes without rejection", maximum)
		}
	}
}

func oversizedCanonicalDocument(maximum int) map[string]any {
	item := strings.Repeat("x", maxStringBytes)
	count := maximum/(maxStringBytes+3) + 1
	values := make([]any, count)
	for index := range values {
		values[index] = item
	}
	return map[string]any{"values": values}
}
