package transitionreceiptcontract

import "testing"

func TestGoldenGrantCompatibilityIsDeclaredOnly(t *testing.T) {
	fixture := loadGolden(t)
	grantFixture := loadFixtureFile(t, "capability-grant-v1.json", maxEnvelopeBytes)
	grant := fixtureNode(t, grantFixture, "grant")
	receipt := fixtureNode(t, fixture, "transition_receipt")
	projected, err := ProjectCapabilityGrantRef(grant)
	if err != nil || !canonicalValuesEqual(projected, fixture["expected_capability_grant_ref"]) {
		t.Fatalf("Grant projection drifted: %v", err)
	}
	assessment, err := AssessDeclaredGrantCompatibility(grant, receipt)
	if err != nil {
		t.Fatal(err)
	}
	relations := assessment["relations"].(map[string]any)
	if relations["grant_ref"] != "same_declared_grant_ref" ||
		relations["actor"] != "same_declared_actor" ||
		relations["declared_time"] != "same_declared_time" {
		t.Fatalf("declared Grant relations = %#v", relations)
	}
	if assessment["result"] != grantCompatibilityResult ||
		canonicalValuesEqual(assessment["reason_codes"], []any{}) {
		t.Fatal("Grant helper hid its authority-neutral mismatch")
	}
}

func TestGoldenApprovalCompatibilityIsDeclaredOnly(t *testing.T) {
	fixture := loadGolden(t)
	approvalFixture := loadFixtureFile(t, "approval-record-v1.json", maxEnvelopeBytes)
	record := fixtureNode(t, approvalFixture, "approval_record")
	receipt := fixtureNode(t, fixture, "transition_receipt")
	projected, err := ProjectApprovalRefs([]map[string]any{record})
	if err != nil || !canonicalValuesEqual(projected, fixture["expected_approval_refs"]) {
		t.Fatalf("Approval projection drifted: %v", err)
	}
	assessment, err := AssessDeclaredApprovalCompatibility([]map[string]any{record}, receipt)
	if err != nil {
		t.Fatal(err)
	}
	relations := assessment["relations"].(map[string]any)
	if relations["ref_set"] != "same_declared_ref_set" || relations["scope"] != "scope_mismatch" {
		t.Fatalf("declared Approval relations = %#v", relations)
	}
	if assessment["result"] != approvalCompatibilityResult {
		t.Fatal("Approval helper escalated beyond declared compatibility")
	}
}

func TestCompatibilityMutationsRemainMismatches(t *testing.T) {
	fixture := loadGolden(t)
	grantFixture := loadFixtureFile(t, "capability-grant-v1.json", maxEnvelopeBytes)
	grant := fixtureNode(t, grantFixture, "grant")
	tests := []struct {
		key    string
		mutate func(map[string]any)
	}{
		{"actor", func(r map[string]any) { r["actor"].(map[string]any)["principal_id"] = "different" }},
		{"approval_refs", func(r map[string]any) {
			r["approval_refs"].([]any)[0].(map[string]any)["authority_domain"] = "different"
		}},
		{"bindings", func(r map[string]any) { r["bindings"].(map[string]any)["source_revision"] = "different" }},
		{"declared_time", func(r map[string]any) {
			r["transition"].(map[string]any)["declared_at_unix_ms"] = int64(1700004000000)
		}},
		{"grant_ref", func(r map[string]any) {
			r["capability_grant_ref"].(map[string]any)["authority_domain"] = "different"
		}},
		{"task_binding", func(r map[string]any) { r["task_binding"].(map[string]any)["role"] = "different" }},
	}
	for _, test := range tests {
		receipt := cloneNode(fixtureNode(t, fixture, "transition_receipt"))
		test.mutate(receipt)
		resealReceipt(t, receipt)
		assessment, err := AssessDeclaredGrantCompatibility(grant, receipt)
		if err != nil {
			t.Fatal(err)
		}
		got := assessment["relations"].(map[string]any)[test.key]
		if got != test.key+"_mismatch" {
			t.Fatalf("%s relation = %#v", test.key, got)
		}
	}
}

func TestApprovalScopeCanOnlyProduceADeclaredRelation(t *testing.T) {
	fixture := loadGolden(t)
	approvalFixture := loadFixtureFile(t, "approval-record-v1.json", maxEnvelopeBytes)
	record := fixtureNode(t, approvalFixture, "approval_record")
	receipt := cloneNode(fixtureNode(t, fixture, "transition_receipt"))
	scope := record["scope"].(map[string]any)
	task := receipt["task_binding"].(map[string]any)
	for _, key := range []string{"project_id", "change_id", "environment_class", "environment_id"} {
		task[key] = scope[key]
	}
	resealReceipt(t, receipt)
	assessment, err := AssessDeclaredApprovalCompatibility([]map[string]any{record}, receipt)
	if err != nil {
		t.Fatal(err)
	}
	if assessment["relations"].(map[string]any)["scope"] != "same_declared_scope" ||
		assessment["result"] != approvalCompatibilityResult {
		t.Fatalf("Approval compatibility = %#v", assessment)
	}
}
