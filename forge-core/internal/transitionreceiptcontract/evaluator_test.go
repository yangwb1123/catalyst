package transitionreceiptcontract

import "testing"

func successor(t *testing.T, previous map[string]any, from, to string) map[string]any {
	t.Helper()
	current := cloneNode(previous)
	current["sequence"] = previous["sequence"].(int64) + 1
	current["previous_receipt_id"] = previous["receipt_id"]
	current["previous_receipt_sha256"] = previous["receipt_sha256"]
	transition := current["transition"].(map[string]any)
	transition["declared_at_unix_ms"] = transition["declared_at_unix_ms"].(int64) + 1
	transition["from_state"], transition["to_state"] = from, to
	transition["rework_target"], transition["resume_state"] = nil, nil
	if to == "CHANGES_REQUESTED" {
		transition["rework_target"] = "VERIFYING"
	}
	if isSuspendedState(to) {
		transition["resume_state"] = from
	}
	current["applicability"].(map[string]any)["stage_id"] = to
	resealReceipt(t, current)
	return current
}

func requestFor(t *testing.T, current, previous map[string]any) map[string]any {
	t.Helper()
	target, err := DeclaredTarget(current)
	if err != nil {
		t.Fatal(err)
	}
	var previousValue any
	if previous != nil {
		previousValue = previous
	}
	request := map[string]any{
		"api_version": requestAPI, "canonicalization": canonicalization,
		"evaluated_at_unix_ms": current["transition"].(map[string]any)["declared_at_unix_ms"],
		"expected_target":      target, "expected_target_sha256": "", "previous_receipt": previousValue,
		"request_sha256": "", "transition_receipt": current,
	}
	resealRequest(t, request)
	return request
}

func assessRelation(t *testing.T, request map[string]any, key string) string {
	t.Helper()
	assessment, err := AssessDeclared(request)
	if err != nil {
		t.Fatal(err)
	}
	assertAuthorityNeutral(t, assessment)
	return assessment["relations"].(map[string]any)[key].(string)
}

func TestDeclaredSuccessorAndRecoveryRelations(t *testing.T) {
	fixture := loadGolden(t)
	initial := fixtureNode(t, fixture, "transition_receipt")
	regular := successor(t, initial, "NEEDS_EVIDENCE", "BASELINED")
	request := requestFor(t, regular, initial)
	assessment, err := AssessDeclared(request)
	if err != nil {
		t.Fatal(err)
	}
	for key, value := range assessment["relations"].(map[string]any) {
		if negativeRelations[value.(string)] {
			t.Fatalf("%s unexpectedly negative: %s", key, value)
		}
	}
	suspended := successor(t, initial, "NEEDS_EVIDENCE", "NEEDS_INFO")
	resumed := successor(t, suspended, "NEEDS_INFO", "NEEDS_EVIDENCE")
	if got := assessRelation(t, requestFor(t, resumed, suspended), "recovery"); got != "internally_consistent_declared_recovery" {
		t.Fatalf("resume relation = %s", got)
	}
}

func TestAllDeclaredMismatchRelations(t *testing.T) {
	fixture := loadGolden(t)
	initial := fixtureNode(t, fixture, "transition_receipt")
	assertEdgeMismatch(t, initial)
	assertChainMismatch(t, initial)
	assertContinuityMismatch(t, initial)
	assertPreconditionMismatch(t, initial)
	assertRecoveryMismatch(t, initial)
	assertTargetMismatch(t, initial)
	assertTemporalMismatch(t, initial)
}

func TestChainMismatchDimensions(t *testing.T) {
	fixture := loadGolden(t)
	initial := fixtureNode(t, fixture, "transition_receipt")
	tests := []struct {
		name   string
		mutate func(map[string]any)
	}{
		{"sequence_gap", func(r map[string]any) { r["sequence"] = int64(3) }},
		{"work", func(r map[string]any) { r["work_id"] = "different-work" }},
		{"project", func(r map[string]any) { r["task_binding"].(map[string]any)["project_id"] = "different" }},
		{"change", func(r map[string]any) { r["task_binding"].(map[string]any)["change_id"] = "different" }},
	}
	for _, test := range tests {
		current := successor(t, initial, "NEEDS_EVIDENCE", "BASELINED")
		test.mutate(current)
		resealReceipt(t, current)
		if got := assessRelation(t, requestFor(t, current, initial), "chain"); got != "predecessor_mismatch" {
			t.Fatalf("%s chain relation = %s", test.name, got)
		}
	}
}

func TestPreviousTimeRollbackIsDeclaredMismatch(t *testing.T) {
	fixture := loadGolden(t)
	initial := fixtureNode(t, fixture, "transition_receipt")
	current := successor(t, initial, "NEEDS_EVIDENCE", "BASELINED")
	current["transition"].(map[string]any)["declared_at_unix_ms"] =
		initial["transition"].(map[string]any)["declared_at_unix_ms"].(int64) - 1
	resealReceipt(t, current)
	if got := assessRelation(t, requestFor(t, current, initial), "temporal"); got != "temporal_declaration_mismatch" {
		t.Fatalf("rollback temporal relation = %s", got)
	}
}

func TestChangesRequestedExitUsesPredecessorReworkTarget(t *testing.T) {
	fixture := loadGolden(t)
	initial := fixtureNode(t, fixture, "transition_receipt")
	previous := successor(t, initial, "NEEDS_EVIDENCE", "CHANGES_REQUESTED")
	matching := successor(t, previous, "CHANGES_REQUESTED", "VERIFYING")
	if got := assessRelation(t, requestFor(t, matching, previous), "recovery"); got != "internally_consistent_declared_recovery" {
		t.Fatalf("matching rework relation = %s", got)
	}
	mismatch := successor(t, previous, "CHANGES_REQUESTED", "PLANNED")
	if got := assessRelation(t, requestFor(t, mismatch, previous), "recovery"); got != "rework_or_resume_mismatch" {
		t.Fatalf("mismatched rework relation = %s", got)
	}
}

func assertEdgeMismatch(t *testing.T, initial map[string]any) {
	current := cloneNode(initial)
	current["transition"].(map[string]any)["to_state"] = "CLOSED"
	current["applicability"].(map[string]any)["stage_id"] = "CLOSED"
	resealReceipt(t, current)
	if got := assessRelation(t, requestFor(t, current, nil), "edge"); got != "unlisted_declared_edge" {
		t.Fatalf("edge relation = %s", got)
	}
}

func assertChainMismatch(t *testing.T, initial map[string]any) {
	current := successor(t, initial, "NEEDS_EVIDENCE", "BASELINED")
	current["previous_receipt_sha256"] = "a" + current["previous_receipt_sha256"].(string)[1:]
	current["previous_receipt_id"] = "transition-receipt-" + current["previous_receipt_sha256"].(string)
	resealReceipt(t, current)
	if got := assessRelation(t, requestFor(t, current, initial), "chain"); got != "predecessor_mismatch" {
		t.Fatalf("chain relation = %s", got)
	}
}

func assertContinuityMismatch(t *testing.T, initial map[string]any) {
	current := successor(t, initial, "BASELINED", "DESIGN_DRAFTED")
	if got := assessRelation(t, requestFor(t, current, initial), "continuity"); got != "state_continuity_mismatch" {
		t.Fatalf("continuity relation = %s", got)
	}
}

func assertPreconditionMismatch(t *testing.T, initial map[string]any) {
	current := cloneNode(initial)
	current["preconditions"].([]any)[0].(map[string]any)["result"] = "UNKNOWN"
	resealReceipt(t, current)
	if got := assessRelation(t, requestFor(t, current, nil), "preconditions"); got != "declared_fail_or_unknown_present" {
		t.Fatalf("precondition relation = %s", got)
	}
}

func assertRecoveryMismatch(t *testing.T, initial map[string]any) {
	suspended := successor(t, initial, "NEEDS_EVIDENCE", "NEEDS_INFO")
	blocked := successor(t, suspended, "NEEDS_INFO", "BLOCKED")
	if got := assessRelation(t, requestFor(t, blocked, suspended), "recovery"); got != "rework_or_resume_mismatch" {
		t.Fatalf("recovery relation = %s", got)
	}
}

func assertTargetMismatch(t *testing.T, initial map[string]any) {
	request := requestFor(t, initial, nil)
	request["expected_target"].(map[string]any)["reason_codes"] = []any{}
	resealRequest(t, request)
	if got := assessRelation(t, request, "target"); got != "target_mismatch" {
		t.Fatalf("target relation = %s", got)
	}
}

func assertTemporalMismatch(t *testing.T, initial map[string]any) {
	request := requestFor(t, initial, nil)
	request["evaluated_at_unix_ms"] = int64(0)
	resealRequest(t, request)
	if got := assessRelation(t, request, "temporal"); got != "temporal_declaration_mismatch" {
		t.Fatalf("temporal relation = %s", got)
	}
}

func assertAuthorityNeutral(t *testing.T, assessment map[string]any) {
	t.Helper()
	for _, key := range []string{"approval_state", "controller_authentication_state", "evidence_state",
		"grant_state", "ledger_state", "precondition_truth_state", "waiver_state"} {
		if assessment[key] != "not_evaluated" {
			t.Fatalf("%s escalated to %#v", key, assessment[key])
		}
	}
	for _, key := range []string{"completion_attestation", "effect_attestation", "execution_attestation",
		"permission_attestation", "persistence_attestation", "transition_attestation"} {
		if assessment[key] != false {
			t.Fatalf("%s became true", key)
		}
	}
	if assessment["authorization_decision"] != "none" || assessment["policy_decision"] != "none" {
		t.Fatal("contract-only assessment made an authority decision")
	}
}
