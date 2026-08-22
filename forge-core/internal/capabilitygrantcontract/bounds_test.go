package capabilitygrantcontract

import "testing"

func TestProofAndUsageBoundsFailClosed(t *testing.T) {
	fixture := loadFixture(t)
	grant := cloneNode(fixtureNode(t, fixture, "grant"))
	proof := fixtureNode(t, grant, "authority_proof")
	proof["proof_base64url"] = "AAAA"
	resealGrant(t, grant)
	if err := validateGrant(grant); err == nil {
		t.Fatal("undersized declared proof was accepted")
	}
	request := cloneNode(fixtureNode(t, fixture, "assessment_request"))
	action := fixtureNode(t, request, "requested_action")
	usage := fixtureNode(t, action, "usage")
	usage["output_bytes"] = int64(1073741825)
	resealRequest(t, request)
	if err := validateAssessmentRequest(request); err == nil {
		t.Fatal("usage above the v1 resource bound was accepted")
	}
}

func TestScopeEnvironmentCannotUseLocalClass(t *testing.T) {
	resource := environmentResource("local", "local", 'a')
	if _, err := validateResource(resource); err == nil {
		t.Fatal("scope environment accepted local task-only class")
	}
}

func TestAssessmentReasonsMustMatchRelations(t *testing.T) {
	fixture := loadFixture(t)
	assessment := cloneNode(fixtureNode(t, fixture, "expected_assessment"))
	assessment["reason_codes"] = []any{"scope_not_covered"}
	resealAssessment(t, assessment)
	encoded, _ := canonicalJSON(assessment)
	if _, err := DecodeCanonicalAssessment(encoded); err == nil {
		t.Fatal("assessment accepted reasons inconsistent with relations")
	}
}

func TestProcessExecTimeoutCannotUnderstateCommand(t *testing.T) {
	command := map[string]any{
		"argv": []any{"tool"}, "cwd": ".", "environment_sha256": hashOf('a'),
		"scope_kind": "command", "stdin_bytes": int64(0), "stdin_sha256": emptySHA256,
		"timeout_ms": int64(60000), "tool_snapshot_sha256": hashOf('b'),
	}
	usage := emptyUsage()
	usage["timeout_ms"] = int64(1000)
	action := map[string]any{"effect_id": "process.exec", "resources": []any{command}, "usage": usage}
	if err := validateRequestedAction(action); err == nil {
		t.Fatal("process.exec usage understated the exact command timeout")
	}
	usage["timeout_ms"] = int64(60000)
	if err := validateRequestedAction(action); err != nil {
		t.Fatalf("matching command and usage timeouts rejected: %v", err)
	}
}

func TestIPv4MappedIPv6OriginsAreRejected(t *testing.T) {
	for _, host := range []string{"::ffff:c000:201", "::ffff:192.0.2.1"} {
		resource := map[string]any{
			"host": host, "host_kind": "ipv6", "port": int64(443),
			"scheme": "https", "scope_kind": "network_origin",
		}
		if _, err := validateResource(resource); err == nil {
			t.Fatalf("IPv4-mapped IPv6 origin %q was accepted", host)
		}
	}
}

func TestIPv6ZoneIdentifiersAreRejected(t *testing.T) {
	for _, host := range []string{"fe80::1%eth0", "::1%lo"} {
		resource := map[string]any{
			"host": host, "host_kind": "ipv6", "port": int64(443),
			"scheme": "https", "scope_kind": "network_origin",
		}
		if _, err := validateResource(resource); err == nil {
			t.Fatalf("IPv6 zone-qualified origin %q was accepted", host)
		}
	}
}

func TestDNSOriginsCannotAliasCanonicalIPv4(t *testing.T) {
	resource := map[string]any{
		"host": "192.0.2.1", "host_kind": "dns", "port": int64(443),
		"scheme": "https", "scope_kind": "network_origin",
	}
	if _, err := validateResource(resource); err == nil {
		t.Fatal("DNS origin accepted a canonical IPv4 literal")
	}
	resource["host_kind"] = "ipv4"
	if _, err := validateResource(resource); err != nil {
		t.Fatalf("canonical IPv4 origin rejected: %v", err)
	}
}

func TestSecretVersionReferencesUseExactASCIIIdentifiers(t *testing.T) {
	resource := map[string]any{
		"broker_id": "vault", "scope_kind": "secret_ref", "secret_ref": "service/token",
		"version_ref": "version-2026.08.11+build/7@prod:1",
	}
	if _, err := validateResource(resource); err != nil {
		t.Fatalf("valid immutable version reference rejected: %v", err)
	}
	for _, version := range []string{"lateſt", "version 1", "版本1", "LATEST", "v1*"} {
		resource["version_ref"] = version
		if _, err := validateResource(resource); err == nil {
			t.Fatalf("ambiguous or moving version reference %q was accepted", version)
		}
	}
}

func TestAssessmentRejectsImpossibleEffectScopePair(t *testing.T) {
	fixture := loadFixture(t)
	assessment := cloneNode(fixtureNode(t, fixture, "expected_assessment"))
	relations := fixtureNode(t, assessment, "relations")
	relations["effect"] = "effect_mismatch"
	relations["scope"] = "denied_by_declaration"
	assessment["reason_codes"] = []any{"deny_matched", "effect_mismatch"}
	resealAssessment(t, assessment)
	encoded, _ := canonicalJSON(assessment)
	if _, err := DecodeCanonicalAssessment(encoded); err == nil {
		t.Fatal("assessment accepted an impossible effect/scope relation pair")
	}
}

func TestProgrammaticCyclicScopeFailsBeforeCanonicalSorting(t *testing.T) {
	fixture := loadFixture(t)
	grant := cloneNode(fixtureNode(t, fixture, "grant"))
	scope := fixtureNode(t, grant, "scope")
	allow, _ := arrayValue(scope, "allow")
	clause := allow[0].(map[string]any)
	clause["resources"] = []any{clause}
	if _, err := CanonicalGrantJSON(grant); err == nil {
		t.Fatal("programmatic cyclic scope was accepted")
	}
}
