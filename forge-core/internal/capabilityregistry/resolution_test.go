package capabilityregistry

import (
	"strings"
	"testing"
)

func TestResolutionPrecedenceAndLegacyOpaqueReference(t *testing.T) {
	entry := resolutionEntry()
	cases := []struct {
		id         string
		version    string
		digest     string
		resolution string
		matched    bool
	}{
		{"absent", "1", strings.Repeat("a", 64), "capability_id_not_found", false},
		{capabilityID, "latest", strings.Repeat("a", 64), "capability_version_not_found", false},
		{capabilityID, capabilityVersion, strings.Repeat("8", 64), "capability_contract_digest_mismatch", true},
		{capabilityID, capabilityVersion, strings.Repeat("a", 64), "resolved_exact", true},
		{"repository-reader", "1", strings.Repeat("8", 64), "capability_id_not_found", false},
	}
	for _, testCase := range cases {
		request := resolutionRequest(testCase.id, testCase.version, testCase.digest)
		resolution, matched, err := resolveReference(entry, request)
		if err != nil || resolution != testCase.resolution || matched != testCase.matched {
			t.Fatalf("%s/%s = %q/%t/%v", testCase.id, testCase.version, resolution, matched, err)
		}
	}
}

func TestAssessmentRelationsNeverEmitAuthorityOrPartialComparison(t *testing.T) {
	for _, resolution := range []string{
		"capability_id_not_found", "capability_version_not_found",
		"capability_contract_digest_mismatch",
	} {
		relations := assessmentRelations(resolution, true)
		if relations["identity"] != resolution {
			t.Fatalf("%s identity = %v", resolution, relations["identity"])
		}
		assertAllNonIdentityRelations(t, relations, "not_evaluated")
	}
	without := assessmentRelations("resolved_exact", false)
	assertAllNonIdentityRelations(t, without, "not_evaluated")
	with := assessmentRelations("resolved_exact", true)
	for _, key := range nonIdentityRelationKeys {
		if with[key] != "same_declared_"+key {
			t.Fatalf("relation %s = %v", key, with[key])
		}
	}
}

func assertAllNonIdentityRelations(t *testing.T, relations map[string]any, expected string) {
	t.Helper()
	for _, key := range nonIdentityRelationKeys {
		if relations[key] != expected {
			t.Fatalf("relation %s = %v, want %s", key, relations[key], expected)
		}
	}
}

func resolutionEntry() map[string]any {
	return map[string]any{"contract": map[string]any{
		"capability_contract_sha256": strings.Repeat("a", 64),
		"capability_id":              capabilityID, "capability_version": capabilityVersion,
	}}
}

func resolutionRequest(id, version, digest string) map[string]any {
	return map[string]any{
		"expected_contract": nil,
		"expected_reference": map[string]any{
			"capability_contract_sha256": digest, "capability_id": id,
			"capability_version": version, "origin": "external_declared",
		},
	}
}
