package capabilitygrantcontract

import "testing"

func TestProductionReleaseDeclarationNeedsExternalOperatorAndApproval(t *testing.T) {
	fixture := loadFixture(t)
	request := productionReleaseRequest(t, fixture)
	grant := fixtureNode(t, request, "grant")
	proof := fixtureNode(t, grant, "authority_proof")
	issuer := fixtureNode(t, proof, "issuer")
	issuer["authority_class"] = "forgeos_kernel"
	issuer["authority_domain"] = "forgeos.kernel.fixture"
	issuer["principal_id"] = "fixture-pdp"
	issuer["principal_type"] = "service"
	grant["approval_refs"] = []any{}
	resealGrant(t, grant)
	resealRequest(t, request)
	if err := validateAssessmentRequest(request); err == nil {
		t.Fatal("production release accepted ForgeOS issuer without approval")
	}
}

func TestExternalProductionEnvelopeStillCarriesNoAuthority(t *testing.T) {
	fixture := loadFixture(t)
	request := productionReleaseRequest(t, fixture)
	vocabulary := fixtureNode(t, fixture, "effect_vocabulary")
	assessment, err := AssessDeclared(vocabulary, request)
	if err != nil {
		t.Fatal(err)
	}
	relations := fixtureNode(t, assessment, "relations")
	if relations["scope"] != "covered_by_declaration" {
		t.Fatalf("unexpected production scope relation: %v", relations["scope"])
	}
	assertNoAuthority(t, assessment)
}

func TestPhaseSpecificBindingsFailClosed(t *testing.T) {
	fixture := loadFixture(t)
	grant := cloneNode(fixtureNode(t, fixture, "grant"))
	bindings := fixtureNode(t, grant, "bindings")
	for _, key := range []string{"impact_sha256", "plan_sha256", "risk_sha256"} {
		bindings[key] = nil
	}
	grant["issuance_phase"] = "bootstrap_planning"
	resealGrant(t, grant)
	if err := validateGrant(grant); err != nil {
		t.Fatalf("bootstrap nullable bindings rejected: %v", err)
	}
	grant["issuance_phase"] = "plan_finalization"
	resealGrant(t, grant)
	if err := validateGrant(grant); err == nil {
		t.Fatal("plan finalization accepted absent impact/plan/risk bindings")
	}
}

func productionReleaseRequest(t *testing.T, fixture map[string]any) map[string]any {
	t.Helper()
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	grant := fixtureNode(t, request, "grant")
	artifact := map[string]any{"artifact_kind": "release_bundle", "artifact_ref": "release/A",
		"artifact_sha256": hashOf('a'), "scope_kind": "artifact"}
	environment := environmentResource("production", "prod", 'e')
	grant["scope"] = map[string]any{"allow": []any{clause(artifact, environment)},
		"deny": []any{}, "effect_id": "release.execute"}
	grant["approval_refs"] = []any{map[string]any{"approval_id": "approval-prod-1",
		"approval_sha256": hashOf('b'), "authority_domain": "operator.example"}}
	proof := fixtureNode(t, grant, "authority_proof")
	issuer := fixtureNode(t, proof, "issuer")
	issuer["authority_class"] = "external_operator"
	issuer["authority_domain"] = "operator.example"
	issuer["principal_id"] = "release-operator"
	issuer["principal_type"] = "operator"
	task := fixtureNode(t, grant, "task_binding")
	task["environment_class"] = "production"
	task["environment_id"] = "prod"
	resealGrant(t, grant)
	expected := fixtureNode(t, request, "expected")
	expected["task_binding"] = cloneNode(task)
	action := fixtureNode(t, request, "requested_action")
	action["effect_id"] = "release.execute"
	action["resources"] = []any{cloneNode(artifact), cloneNode(environment)}
	resealRequest(t, request)
	return request
}
