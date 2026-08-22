package capabilityregistry

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const (
	goldenFixturePath   = "../../../docs/contracts/fixtures/capability-registry-v1.json"
	pinnedGoldenBytes   = 28758
	pinnedGoldenSHA256  = "0ce4929ad82ce70ef0520be80b7bd3eaf47f5ff1205d0a53e12fbe1115ed11b5"
	goldenFixtureAPI    = "forgeos.capability-registry-golden/v1"
	goldenFixtureIntent = "exact_cross_language_declared_resolution_without_authority"
)

var goldenResolutions = map[string]string{
	"legacy_repository_reader_not_registered": "capability_id_not_found",
	"registered_key_digest_mismatch":          "capability_contract_digest_mismatch",
	"resolved_exact":                          "resolved_exact",
}

type goldenFixture struct {
	registry    map[string]any
	requests    map[string]any
	assessments map[string]any
}

func TestCrossLanguageGoldenFixture(t *testing.T) {
	fixture := loadGoldenFixture(t)
	registryRaw := mustCanonical(t, fixture.registry)
	if !bytes.Equal(registryRaw, []byte(pinnedRegistryJSON)) {
		t.Fatal("embedded registry bytes differ from the physical golden")
	}
	registry, err := DecodeRegistry(registryRaw)
	if err != nil {
		t.Fatalf("decode golden registry: %v", err)
	}
	for caseID, resolution := range goldenResolutions {
		checkGoldenCase(t, registry, fixture.requests[caseID],
			fixture.assessments[caseID], resolution)
	}
}

func TestCommandEmitsExactGoldenBytes(t *testing.T) {
	fixture := loadGoldenFixture(t)
	registryRaw := mustCanonical(t, fixture.registry)
	requestRaw := mustCanonical(t, fixture.requests["resolved_exact"])
	assessmentRaw := mustCanonical(t, fixture.assessments["resolved_exact"])
	directory := t.TempDir()
	registryPath := writeGoldenInput(t, directory, "registry.json", registryRaw)
	requestPath := writeGoldenInput(t, directory, "request.json", requestRaw)
	tests := []struct {
		args   []string
		stdin  []byte
		output []byte
	}{
		{[]string{"validate", "--registry", "-"}, registryRaw, registryRaw},
		{[]string{"validate", "--registry", registryPath}, nil, registryRaw},
		{[]string{"resolve", "--registry", registryPath, "--request", "-"}, requestRaw, assessmentRaw},
		{[]string{"resolve", "--registry", "-", "--request", requestPath}, registryRaw, assessmentRaw},
	}
	for index, testCase := range tests {
		assertGoldenCommand(t, index, testCase.args, testCase.stdin, testCase.output)
	}
}

func TestExpectedContractSingletonBoundary(t *testing.T) {
	fixture := loadGoldenFixture(t)
	registryRaw := mustCanonical(t, fixture.registry)
	registry, err := DecodeRegistry(registryRaw)
	if err != nil {
		t.Fatal(err)
	}
	registryPath := writeGoldenInput(t, t.TempDir(), "registry.json", registryRaw)
	base := fixture.requests["resolved_exact"].(map[string]any)
	cases := []struct {
		field, value, resolution string
	}{
		{"capability_id", "unregistered-capability", "capability_id_not_found"},
		{"capability_version", "2", "capability_version_not_found"},
	}
	for _, testCase := range cases {
		nonnull := mutateGoldenRequest(t, base, testCase.field, testCase.value, true)
		nonnullRaw := mustCanonical(t, nonnull)
		if _, err := DecodeRequest(nonnullRaw); err == nil {
			t.Fatalf("nonnull unknown %s accepted", testCase.field)
		}
		stdout, stderr := &countingWriter{}, &bytes.Buffer{}
		args := []string{"resolve", "--registry", registryPath, "--request", "-"}
		if code := Command(args, bytes.NewReader(nonnullRaw), stdout, stderr); code != 1 || stdout.writes != 0 {
			t.Fatalf("nonnull unknown %s emitted an assessment", testCase.field)
		}
		nullable := mutateGoldenRequest(t, base, testCase.field, testCase.value, false)
		request, err := DecodeRequest(mustCanonical(t, nullable))
		if err != nil {
			t.Fatalf("null unknown %s rejected: %v", testCase.field, err)
		}
		assessment, err := Resolve(registry, request)
		if err != nil || assessment["resolution"] != testCase.resolution {
			t.Fatalf("null unknown %s = %v/%v", testCase.field, assessment, err)
		}
	}
}

func TestFrozenRegistryRejectsResealedContractAndTestMutations(t *testing.T) {
	mutations := []func(map[string]any){
		func(value map[string]any) {
			firstEntry(value)["tests"].([]any)[0].(map[string]any)["entrypoint"] = "attacker test"
		},
		func(value map[string]any) {
			goldenContract(value)["trigger"].(map[string]any)["predicates"].([]any)[0].(map[string]any)["value"] = "forgeos.canonical-json/v2"
		},
		func(value map[string]any) {
			goldenContract(value)["quality_gates"].([]any)[0].(map[string]any)["required_test_ids"] = []any{"python-contract-suite"}
		},
		func(value map[string]any) {
			contract := goldenContract(value)
			contract["effects"] = []any{"repo.read"}
			contract["permission_requirements"] = []any{map[string]any{
				"effect_id": "repo.read", "requirement_id": "read-input", "scope_profile": "repo_read"}}
		},
	}
	for index, mutate := range mutations {
		value := cloneJSON(loadGoldenFixture(t).registry).(map[string]any)
		mutate(value)
		resealGoldenRegistry(t, value)
		if _, err := DecodeRegistry(mustCanonical(t, value)); err == nil {
			t.Fatalf("resealed contract/test mutation %d accepted", index)
		}
	}
}

func TestFrozenRegistryRejectsResealedContentAndEntryMutations(t *testing.T) {
	mutations := []func(map[string]any){
		func(value map[string]any) { reverseAny(goldenSets(value)) },
		func(value map[string]any) {
			sets := goldenSets(value)
			firstEntry(value)["content_sets"] = append(sets, cloneJSON(sets[0]))
		},
		func(value map[string]any) {
			mutateGoldenFiles(t, value, func(files []any) []any { return files[:len(files)-1] })
		},
		func(value map[string]any) { mutateGoldenFiles(t, value, addGoldenFile) },
		func(value map[string]any) { mutateGoldenFiles(t, value, driftGoldenFile) },
		func(value map[string]any) { mutateGoldenFiles(t, value, cycleGoldenFile) },
		func(value map[string]any) {
			entries := value["entries"].([]any)
			value["entries"] = append(entries, cloneJSON(entries[0]))
		},
	}
	for index, mutate := range mutations {
		value := cloneJSON(loadGoldenFixture(t).registry).(map[string]any)
		mutate(value)
		resealGoldenRegistry(t, value)
		if _, err := DecodeRegistry(mustCanonical(t, value)); err == nil {
			t.Fatalf("resealed content/entry mutation %d accepted", index)
		}
	}
}

func loadGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	raw, err := os.ReadFile(goldenFixturePath)
	if err != nil {
		t.Fatal(err)
	}
	actualSHA := fmt.Sprintf("%x", sha256.Sum256(raw))
	if len(raw) != pinnedGoldenBytes || actualSHA != pinnedGoldenSHA256 {
		t.Fatalf("golden physical pin = %d/%s", len(raw), actualSHA)
	}
	if bytes.Count(raw, []byte{'\n'}) != 1 || raw[len(raw)-1] != '\n' {
		t.Fatal("golden must have exactly one terminal LF")
	}
	value, err := parseStrictJSON(raw[:len(raw)-1], maxGoldenBytes)
	if err != nil {
		t.Fatalf("decode golden: %v", err)
	}
	root, ok := value.(map[string]any)
	if !ok || requireKeys(root, "api_version", "assessments", "fixture_semantics", "registry", "requests") != nil ||
		root["api_version"] != goldenFixtureAPI || root["fixture_semantics"] != goldenFixtureIntent {
		t.Fatal("golden envelope drifted")
	}
	registry, registryOK := root["registry"].(map[string]any)
	requests, requestOK := root["requests"].(map[string]any)
	assessments, assessmentOK := root["assessments"].(map[string]any)
	if !registryOK || !requestOK || !assessmentOK ||
		!hasExactGoldenCases(requests) || !hasExactGoldenCases(assessments) {
		t.Fatal("golden case maps drifted")
	}
	return goldenFixture{registry, requests, assessments}
}

func firstEntry(value map[string]any) map[string]any {
	return value["entries"].([]any)[0].(map[string]any)
}

func goldenContract(value map[string]any) map[string]any {
	return firstEntry(value)["contract"].(map[string]any)
}

func goldenSets(value map[string]any) []any {
	return firstEntry(value)["content_sets"].([]any)
}

func resealGoldenRegistry(t *testing.T, value map[string]any) {
	t.Helper()
	resealPrefixed(t, goldenContract(value), contractDigestDomain,
		"capability_contract_id", "capability_contract_sha256", "capability-contract-")
	resealPrefixed(t, firstEntry(value), entryDigestDomain,
		"entry_id", "entry_sha256", "capability-registry-entry-")
	resealPrefixed(t, value, registryDigestDomain,
		"registry_id", "registry_sha256", "capability-registry-")
}

func resealPrefixed(t *testing.T, value map[string]any, domain, id, digest, prefix string) {
	t.Helper()
	computed, err := digestDocument(domain, value, id, digest)
	if err != nil {
		t.Fatal(err)
	}
	value[id], value[digest] = prefix+computed, computed
}

func mutateGoldenFiles(t *testing.T, value map[string]any, mutate func([]any) []any) {
	t.Helper()
	contentSet := goldenSets(value)[0].(map[string]any)
	contentSet["files"] = mutate(contentSet["files"].([]any))
	digest, err := digestDocument(contentSetDigestDomain, contentSet, "set_sha256")
	if err != nil {
		t.Fatal(err)
	}
	contentSet["set_sha256"] = digest
}

func reverseAny(values []any) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}

func addGoldenFile(files []any) []any {
	return append(files, map[string]any{"content_bytes": int64(1), "content_sha256": strings.Repeat("0", 64),
		"media_type": "text/x-python", "path": "extra.py", "selector": nil})
}

func driftGoldenFile(files []any) []any {
	files[0].(map[string]any)["content_sha256"] = strings.Repeat("0", 64)
	return files
}

func cycleGoldenFile(files []any) []any {
	files[0].(map[string]any)["path"] = "docs/contracts/capability-registry-v1.schema.json"
	return files
}

func checkGoldenCase(
	t *testing.T, registry map[string]any, requestValue, assessmentValue any, resolution string,
) {
	t.Helper()
	requestObject, requestOK := requestValue.(map[string]any)
	assessmentObject, assessmentOK := assessmentValue.(map[string]any)
	if !requestOK || !assessmentOK {
		t.Fatal("golden case is not an object pair")
	}
	request, err := DecodeRequest(mustCanonical(t, requestObject))
	if err != nil {
		t.Fatalf("decode request: %v", err)
	}
	actual, err := Resolve(registry, request)
	if err != nil {
		t.Fatalf("resolve request: %v", err)
	}
	expectedRaw := mustCanonical(t, assessmentObject)
	if !bytes.Equal(mustCanonical(t, actual), expectedRaw) || actual["resolution"] != resolution {
		t.Fatalf("resolution differs from Python golden %q", resolution)
	}
	decoded, err := DecodeAssessment(expectedRaw)
	if err != nil || ValidateAssessment(registry, request, decoded) != nil {
		t.Fatalf("validate assessment: %v", err)
	}
}

func mutateGoldenRequest(
	t *testing.T, base map[string]any, field, value string, withContract bool,
) map[string]any {
	t.Helper()
	request := cloneJSON(base).(map[string]any)
	reference := request["expected_reference"].(map[string]any)
	reference[field], reference["origin"] = value, "external_declared"
	if withContract {
		contract := request["expected_contract"].(map[string]any)
		contract[field] = value
		digest, err := digestDocument(contractDigestDomain, contract,
			"capability_contract_id", "capability_contract_sha256")
		if err != nil {
			t.Fatal(err)
		}
		contract["capability_contract_id"] = "capability-contract-" + digest
		contract["capability_contract_sha256"] = digest
		reference["capability_contract_sha256"] = digest
	} else {
		request["expected_contract"] = nil
	}
	digest, err := digestDocument(requestDigestDomain, request, "request_sha256")
	if err != nil {
		t.Fatal(err)
	}
	request["request_sha256"] = digest
	return request
}

func hasExactGoldenCases(value map[string]any) bool {
	if len(value) != len(goldenResolutions) {
		return false
	}
	for caseID := range goldenResolutions {
		if _, exists := value[caseID]; !exists {
			return false
		}
	}
	return true
}

func mustCanonical(t *testing.T, value any) []byte {
	t.Helper()
	raw, err := canonicalJSON(value)
	if err != nil {
		t.Fatal(err)
	}
	return raw
}

func writeGoldenInput(t *testing.T, directory, name string, raw []byte) string {
	t.Helper()
	path := filepath.Join(directory, name)
	if err := os.WriteFile(path, raw, 0o600); err != nil {
		t.Fatal(err)
	}
	return path
}

func assertGoldenCommand(t *testing.T, index int, args []string, stdin, expected []byte) {
	t.Helper()
	stdout, stderr := &bytes.Buffer{}, &bytes.Buffer{}
	if code := Command(args, bytes.NewReader(stdin), stdout, stderr); code != 0 || stderr.Len() != 0 {
		t.Fatalf("command %d = code %d stderr %q", index, code, stderr.String())
	}
	want := append(append([]byte(nil), expected...), '\n')
	if !bytes.Equal(stdout.Bytes(), want) {
		t.Fatalf("command %d output differs from exact golden", index)
	}
}
