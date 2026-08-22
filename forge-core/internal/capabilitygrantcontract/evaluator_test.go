package capabilitygrantcontract

import "testing"

func TestDenyPrecedenceAndDefaultDenyAreDeclaredOnly(t *testing.T) {
	fixture := loadFixture(t)
	vocabulary := fixtureNode(t, fixture, "effect_vocabulary")
	tests := []struct {
		path, relation, reason string
	}{
		{"src/secrets/credentials.go", "denied_by_declaration", "deny_matched"},
		{"src2/domain.go", "outside_declared_scope", "scope_not_covered"},
	}
	for _, testCase := range tests {
		request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
		action := fixtureNode(t, request, "requested_action")
		resources, _ := arrayValue(action, "resources")
		resources[0].(map[string]any)["path"] = testCase.path
		resealRequest(t, request)
		assessment, err := AssessDeclared(vocabulary, request)
		if err != nil {
			t.Fatal(err)
		}
		relations := fixtureNode(t, assessment, "relations")
		if relations["scope"] != testCase.relation || !hasReason(assessment, testCase.reason) {
			t.Fatalf("%s: relation=%v reasons=%v", testCase.path, relations["scope"], assessment["reason_codes"])
		}
		assertNoAuthority(t, assessment)
	}
}

func TestBudgetAndBindingDriftAreReceiptedWithoutAuthority(t *testing.T) {
	fixture := loadFixture(t)
	vocabulary := fixtureNode(t, fixture, "effect_vocabulary")
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	action := fixtureNode(t, request, "requested_action")
	usage := fixtureNode(t, action, "usage")
	usage["output_bytes"] = int64(131073)
	expected := fixtureNode(t, request, "expected")
	bindings := fixtureNode(t, expected, "bindings")
	bindings["source_revision"] = "different-revision"
	resealRequest(t, request)
	assessment, err := AssessDeclared(vocabulary, request)
	if err != nil {
		t.Fatal(err)
	}
	relations := fixtureNode(t, assessment, "relations")
	if relations["budget"] != "exceeds_declared_ceiling" || relations["binding"] != "binding_mismatch" {
		t.Fatalf("unexpected relations: %#v", relations)
	}
	if !hasReason(assessment, "budget_exceeded") || !hasReason(assessment, "binding_mismatch") {
		t.Fatalf("missing drift reasons: %v", assessment["reason_codes"])
	}
	assertNoAuthority(t, assessment)
}

func TestAllowClausesCannotBeCombined(t *testing.T) {
	artifactA := map[string]any{"artifact_kind": "bundle", "artifact_ref": "A", "artifact_sha256": hashOf('a'), "scope_kind": "artifact"}
	artifactB := map[string]any{"artifact_kind": "bundle", "artifact_ref": "B", "artifact_sha256": hashOf('b'), "scope_kind": "artifact"}
	development := environmentResource("development", "dev", 'd')
	production := environmentResource("production", "prod", 'e')
	scope := map[string]any{
		"allow": []any{clause(artifactA, development), clause(artifactB, production)},
		"deny":  []any{}, "effect_id": "migration.apply",
	}
	action := map[string]any{
		"effect_id": "migration.apply", "resources": []any{artifactA, production},
		"usage": emptyUsage(),
	}
	relation, reason := assessScope(scope, action)
	if relation != "outside_declared_scope" || reason != "scope_not_covered" {
		t.Fatalf("partial clauses combined: %s %s", relation, reason)
	}
}

func TestMigrationGeneratePreservesDeclaredEnvironmentQualifier(t *testing.T) {
	staging := environmentResource("staging", "stage", 'e')
	development := environmentResource("development", "dev", 'd')
	path := map[string]any{"match": "exact", "path": "out/migration.sql", "scope_kind": "repo_path"}
	scope := map[string]any{
		"allow": []any{clause(staging, path)}, "deny": []any{}, "effect_id": "migration.generate",
	}
	tests := []struct {
		name      string
		resources []any
		relation  string
	}{
		{"same environment", []any{staging, path}, "covered_by_declaration"},
		{"omitted environment", []any{path}, "outside_declared_scope"},
		{"different environment", []any{development, path}, "outside_declared_scope"},
	}
	for _, testCase := range tests {
		action := map[string]any{"effect_id": "migration.generate", "resources": testCase.resources}
		relation, _ := assessScope(scope, action)
		if relation != testCase.relation {
			t.Fatalf("%s: got %s, want %s", testCase.name, relation, testCase.relation)
		}
	}
}

func TestScopeValidationRejectsUnsafeCeilings(t *testing.T) {
	fixture := loadFixture(t)
	grant := fixtureNode(t, fixture, "grant")
	scope := cloneNode(fixtureNode(t, grant, "scope"))
	scope["effect_id"] = "repo.write"
	if _, err := validateScope(scope); err == nil {
		t.Fatal("repo.write accepted subtree allow ceiling")
	}
	unsafe := map[string]any{"match": "exact", "path": "../secret", "scope_kind": "repo_path"}
	if _, err := validateResource(unsafe); err == nil {
		t.Fatal("parent-traversal repo path was accepted")
	}
	secret := map[string]any{"broker_id": "broker", "scope_kind": "secret_ref", "secret_ref": "db", "version_ref": "latest"}
	if _, err := validateResource(secret); err == nil {
		t.Fatal("moving secret version alias was accepted")
	}
}

func TestProductionExternalOperatorRequirementIsStructuralOnly(t *testing.T) {
	environment := environmentResource("production", "prod", 'e')
	scope := map[string]any{"allow": []any{clause(environment)}, "deny": []any{}, "effect_id": "release.execute"}
	issuer := map[string]any{"authority_class": "forgeos_kernel"}
	if err := validateProductionRestriction(scope, issuer, nil); err == nil {
		t.Fatal("ForgeOS issuer accepted for a production release declaration")
	}
}

func hasReason(assessment map[string]any, target string) bool {
	reasons, _ := arrayValue(assessment, "reason_codes")
	for _, reason := range reasons {
		if reason == target {
			return true
		}
	}
	return false
}

func assertNoAuthority(t *testing.T, assessment map[string]any) {
	t.Helper()
	if assessment["authorization_decision"] != "none" || assessment["permission_attestation"] != false ||
		assessment["effect_attestation"] != false || assessment["result"] != assessedDeclarationsOnly {
		t.Fatalf("assessment asserted authority: %#v", assessment)
	}
}

func hashOf(value byte) string {
	result := make([]byte, 64)
	for index := range result {
		result[index] = value
	}
	return string(result)
}

func environmentResource(class, id string, hashByte byte) map[string]any {
	return map[string]any{"environment_class": class, "environment_id": id,
		"environment_sha256": hashOf(hashByte), "scope_kind": "environment"}
}

func clause(resources ...map[string]any) map[string]any {
	values := make([]any, len(resources))
	for index, resource := range resources {
		values[index] = resource
	}
	return map[string]any{"resources": values}
}

func emptyUsage() map[string]any {
	return map[string]any{"call_count": int64(1), "cost_usd_micros": int64(0), "input_tokens": int64(0),
		"network_bytes": int64(0), "output_bytes": int64(0), "output_tokens": int64(0), "timeout_ms": int64(1)}
}
