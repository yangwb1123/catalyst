package planningownership

import (
	"bytes"
	"encoding/base64"
	"strings"
	"testing"
)

func TestSourceShapeAndCoverageMutationsFailClosed(t *testing.T) {
	catalog := string(readFixture(t, catalogFixturePath))
	mapping := string(readFixture(t, mappingFixturePath))
	cases := []struct {
		name    string
		catalog string
		mapping string
	}{
		{"catalog-extra-field", strings.Replace(catalog, "kind: AIEngineeringCapabilityCatalog\n", "kind: AIEngineeringCapabilityCatalog\nextra: x\n", 1), mapping},
		{"node-missing-field", strings.Replace(catalog, "    name: intake-orchestration\n", "", 1), mapping},
		{"duplicate-node-id", strings.Replace(catalog, `  - id: "01"`, `  - id: "00"`, 1), mapping},
		{"duplicate-node-capability", strings.Replace(catalog, "capabilities: [work-intake, project-snapshot", "capabilities: [work-intake, work-intake", 1), mapping},
		{"mapping-extra-field", catalog, strings.Replace(mapping, "kind: CapabilitySkillOwnershipMap\n", "kind: CapabilitySkillOwnershipMap\nextra: x\n", 1)},
		{"package-extra-field", catalog, strings.Replace(mapping, "    implementation_wave: 1\n", "    implementation_wave: 1\n    extra: x\n", 1)},
		{"duplicate-skill", catalog, strings.Replace(mapping, "skill: evidence-claim-management", "skill: change-intake-orchestration", 1)},
		{"duplicate-package-capability", catalog, strings.Replace(mapping, "includes: [evidence-scan]", "includes: [work-intake]", 1)},
		{"duplicate-within-package", catalog, strings.Replace(mapping, "includes: [work-intake, convergence]", "includes: [work-intake, work-intake]", 1)},
		{"missing-owner", catalog, strings.Replace(mapping, "[work-intake, convergence]", "[work-intake]", 1)},
		{"dangling-owner", catalog, strings.Replace(mapping, "[work-intake, convergence]", "[work-intake, convergence, dangling]", 1)},
		{"wave-overflow", catalog, strings.Replace(mapping, "implementation_wave: 1", "implementation_wave: 7", 1)},
		{"source-basename", catalog, strings.Replace(mapping, "source_catalog: capability-catalog.v1.yml", "source_catalog: other.yml", 1)},
		{"mapping-rule-wrong-shape", catalog, strings.Replace(mapping, "mapping_rules:\n  - \"Every", "mapping_rules:\n  - {bad: shape}\n  - \"Every", 1)},
		{"extension-ref-wrong-shape", strings.Replace(catalog, "extension_decision_refs: [ADR-0038, ADR-0039]", "extension_decision_refs: [{bad: shape}]", 1), mapping},
		{"node-array-wrong-shape", strings.Replace(catalog, "entry_criteria: [user_or_runtime_intent_exists]", "entry_criteria: wrong", 1), mapping},
	}
	for _, testCase := range cases {
		t.Run(testCase.name, func(t *testing.T) {
			request, err := BuildRequest([]byte(testCase.catalog), []byte(testCase.mapping))
			if err == nil {
				_, err = Project(request)
			}
			if err == nil {
				t.Fatal("source mutation produced a projection")
			}
		})
	}
}

func TestIgnoredNodeArraysKeepFrozenGenericYAMLShapes(t *testing.T) {
	catalog := string(readFixture(t, catalogFixturePath))
	mapping := readFixture(t, mappingFixturePath)
	mutated := strings.Replace(catalog, "entry_criteria: [user_or_runtime_intent_exists]",
		"entry_criteria: [user_or_runtime_intent_exists, {artifact: Evidence, applies_when: true}, [nested, null], false, 6]", 1)
	if _, err := BuildRequest([]byte(mutated), mapping); err != nil {
		t.Fatalf("bounded ignored-field YAML universe was narrowed: %v", err)
	}
}

func TestBuildRequestRejectsRoleSourceBoundsBeforeYAMLParsing(t *testing.T) {
	validCatalog := readFixture(t, catalogFixturePath)
	validMapping := readFixture(t, mappingFixturePath)
	cases := []struct {
		catalog []byte
		mapping []byte
	}{
		{nil, validMapping},
		{validCatalog, nil},
		{make([]byte, maxCatalogSourceBytes+1), validMapping},
		{validCatalog, make([]byte, maxMappingSourceBytes+1)},
	}
	for index, testCase := range cases {
		if _, err := BuildRequest(testCase.catalog, testCase.mapping); err == nil ||
			!strings.Contains(err.Error(), "before parsing") {
			t.Fatalf("source bound case %d did not fail before parsing: %v", index, err)
		}
	}
}

func TestRequestSourceBindingMutationsFailClosed(t *testing.T) {
	request := currentRequest(t)
	cases := []func(map[string]any){
		func(value map[string]any) { value["catalog_source"].(map[string]any)["content_bytes"] = int64(1) },
		func(value map[string]any) {
			value["catalog_source"].(map[string]any)["content_sha256"] = strings.Repeat("0", 64)
		},
		func(value map[string]any) { value["mapping_source"].(map[string]any)["document_name"] = "other.yml" },
		func(value map[string]any) {
			value["mapping_source"].(map[string]any)["source_role"] = "capability_catalog"
		},
		func(value map[string]any) {
			record := value["mapping_source"].(map[string]any)
			record["content_base64"] = strings.TrimSuffix(record["content_base64"].(string), "=")
		},
		func(value map[string]any) {
			record := value["mapping_source"].(map[string]any)
			encoded := record["content_base64"].(string)
			record["content_base64"] = "-" + encoded[1:]
		},
	}
	for index, mutate := range cases {
		value := cloneObject(request.document)
		mutate(value)
		resealDocument(t, value, requestDigestDomain, "request_sha256")
		if _, err := DecodeRequest(mustCanonical(t, value)); err == nil {
			t.Fatalf("source binding mutation %d accepted", index)
		}
	}
}

func TestCanonicalRequestRejectsDuplicateUnknownAndNoncanonicalJSON(t *testing.T) {
	request := currentRequest(t)
	raw := request.CanonicalBytes()
	cases := [][]byte{
		append([]byte{' '}, raw...), append(cloneBytes(raw), ' '),
		bytes.Replace(raw, []byte(`{"api_version":`), []byte(`{"api_version":"duplicate","api_version":`), 1),
		bytes.Replace(raw, []byte(`{"api_version":`), []byte(`{"extra":null,"api_version":`), 1),
		bytes.Replace(raw, []byte(`"kind":"Planning`), []byte(`"kind":"Planning\u0043`), 1),
	}
	for index, candidate := range cases {
		if _, err := DecodeRequest(candidate); err == nil {
			t.Fatalf("noncanonical request %d accepted", index)
		}
	}
}

func TestResealedProjectionMutationsFailFullReconstruction(t *testing.T) {
	fixture := loadGolden(t)
	root, err := parseCanonicalObject(fixture.projectionRaw, maxProjectionBytes)
	if err != nil {
		t.Fatal(err)
	}
	cases := []func(map[string]any){
		func(value map[string]any) { value["authority_semantics"].(map[string]any)["runtime_routing"] = true },
		func(value map[string]any) { value["coverage"].(map[string]any)["binding_count"] = int64(139) },
		func(value map[string]any) { value["bindings"].([]any)[0].(map[string]any)["owner_skill"] = "attacker" },
		func(value map[string]any) {
			value["bindings"].([]any)[0].(map[string]any)["catalog_occurrence_count"] = int64(64)
		},
		func(value map[string]any) {
			value["bindings"].([]any)[0].(map[string]any)["declared_logical_adapter_ref"] = ".agent/skills/attacker.md"
		},
		func(value map[string]any) { reverseValues(value["bindings"].([]any)) },
		func(value map[string]any) {
			nodes := value["bindings"].([]any)[0].(map[string]any)["catalog_node_ids"].([]any)
			value["bindings"].([]any)[0].(map[string]any)["catalog_node_ids"] = append(nodes, nodes[0])
		},
	}
	for index, mutate := range cases {
		value := cloneObject(root)
		mutate(value)
		resealDocument(t, value, projectionDigestDomain, "projection_sha256")
		if _, err := DecodeProjection(mustCanonical(t, value)); err == nil {
			t.Fatalf("resealed projection mutation %d accepted", index)
		}
	}
}

func TestRequestRejectsNoncanonicalBase64EvenWhenPayloadCouldDecode(t *testing.T) {
	request := currentRequest(t)
	value := cloneObject(request.document)
	record := value["mapping_source"].(map[string]any)
	record["content_base64"] = base64.RawStdEncoding.EncodeToString(request.mapping)
	resealDocument(t, value, requestDigestDomain, "request_sha256")
	if _, err := DecodeRequest(mustCanonical(t, value)); err == nil {
		t.Fatal("unpadded RFC4648 base64 accepted")
	}
}

func currentRequest(t *testing.T) Request {
	t.Helper()
	request, err := BuildRequest(readFixture(t, catalogFixturePath), readFixture(t, mappingFixturePath))
	if err != nil {
		t.Fatal(err)
	}
	return request
}

func resealDocument(t *testing.T, value map[string]any, domain, field string) {
	t.Helper()
	digest, err := documentDigest(domain, value, field)
	if err != nil {
		t.Fatal(err)
	}
	value[field] = digest
}

func mustCanonical(t *testing.T, value any) []byte {
	t.Helper()
	raw, err := canonicalJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func reverseValues(values []any) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}
