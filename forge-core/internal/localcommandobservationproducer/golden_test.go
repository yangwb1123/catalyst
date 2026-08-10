package localcommandobservationproducer

import (
	"bytes"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
)

const (
	producerFixtureAPIVersion = "forgeos.governance.local-gate-command-observation-production.fixture/v1"
	pureFixtureSemantics      = "PURE_CONTRACT_FIXTURE (deterministic bytes only; no live process execution, pass, criterion, completion, truth, authority, identity, persistence, or external-effect attestation)"
)

type producerGoldenFixture struct {
	APIVersion       string                  `json:"api_version"`
	Expected         producerGoldenExpected  `json:"expected"`
	FixtureSemantics string                  `json:"fixture_semantics"`
	Preimages        producerGoldenPreimages `json:"preimages"`
	Production       json.RawMessage         `json:"production"`
}

type producerGoldenPreimages struct {
	SourceRegularFiles []producerGoldenFilePreimage `json:"source_regular_files"`
	Tool               producerGoldenToolPreimage   `json:"tool"`
}

type producerGoldenFilePreimage struct {
	Path string `json:"path"`
	UTF8 string `json:"utf8"`
}

type producerGoldenToolPreimage struct {
	FinalPath string `json:"final_path"`
	UTF8      string `json:"utf8"`
}

type producerGoldenExpected struct {
	CanonicalEnvironmentManifestJSON string `json:"canonical_environment_manifest_json"`
	CanonicalObservationJSON         string `json:"canonical_observation_json"`
	CanonicalProductionJSON          string `json:"canonical_production_json"`
	CanonicalSourceManifestJSON      string `json:"canonical_source_manifest_json"`
	CanonicalToolManifestJSON        string `json:"canonical_tool_manifest_json"`
	EnvironmentSHA256                string `json:"environment_sha256"`
	ProductionSHA256                 string `json:"production_sha256"`
	Result                           string `json:"result"`
	SourceTreeSHA256                 string `json:"source_tree_sha256"`
	ToolSnapshotSHA256               string `json:"tool_snapshot_sha256"`
}

func TestGoldenLocalCommandObservationProduction(t *testing.T) {
	fixture := loadProducerGoldenFixture(t)
	if fixture.APIVersion != producerFixtureAPIVersion || fixture.FixtureSemantics != pureFixtureSemantics {
		t.Fatalf("fixture identity or non-live semantics drifted: %#v", fixture)
	}
	fromFixture := decodeFixtureProduction(t, fixture.Production)
	canonical, err := canonicalManifest(fromFixture)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenValue(t, "production JSON", string(canonical), fixture.Expected.CanonicalProductionJSON)
	strict, err := decodeCanonicalProduction([]byte(fixture.Expected.CanonicalProductionJSON))
	if err != nil {
		t.Fatalf("strictly decode canonical production: %v", err)
	}
	if !reflect.DeepEqual(strict, fromFixture) {
		t.Fatal("pretty fixture production and exact canonical production differ")
	}
	assertGoldenPreimages(t, strict, fixture.Preimages)
	assertGoldenProfiles(t, strict, fixture.Expected)
	if fixture.Expected.Result != ObservedLocalProcess {
		t.Fatalf("fixture result = %q", fixture.Expected.Result)
	}
}

func assertGoldenPreimages(t *testing.T, value ProductionPackage, preimages producerGoldenPreimages) {
	t.Helper()
	toolBytes := []byte(preimages.Tool.UTF8)
	if preimages.Tool.FinalPath != value.ToolManifest.FinalPath ||
		int64(len(toolBytes)) != value.ToolManifest.Bytes || sha256Bytes(toolBytes) != value.ToolManifest.SHA256 {
		t.Fatal("tool fixture preimage does not bind tool manifest bytes and SHA-256")
	}
	regular := make(map[string]SourceEntry)
	for _, entry := range value.SourceManifest.Entries {
		if entry.Kind == "regular" {
			regular[entry.Path] = entry
		}
	}
	if len(preimages.SourceRegularFiles) != len(regular) {
		t.Fatal("source regular preimages are not a complete closed set")
	}
	for _, preimage := range preimages.SourceRegularFiles {
		entry, exists := regular[preimage.Path]
		bytes := []byte(preimage.UTF8)
		if !exists || entry.ContentSHA256 == nil || int64(len(bytes)) != entry.Bytes ||
			sha256Bytes(bytes) != *entry.ContentSHA256 {
			t.Fatalf("source fixture preimage does not bind %q", preimage.Path)
		}
		delete(regular, preimage.Path)
	}
	if len(regular) != 0 {
		t.Fatalf("source fixture preimages omitted paths %v", regular)
	}
}

func TestDecodeCanonicalProductionRejectsDrift(t *testing.T) {
	fixture := loadProducerGoldenFixture(t)
	raw := fixture.Expected.CanonicalProductionJSON
	duplicateAPI := `{"api_version":"` + ProductionAPIVersion + `"`
	variants := []string{
		" " + raw,
		raw + "\n",
		strings.Replace(raw, `{"api_version":`, `{"unknown":false,"api_version":`, 1),
		strings.Replace(raw, duplicateAPI, duplicateAPI+`,"api_version":"`+ProductionAPIVersion+`"`, 1),
		strings.Replace(raw, `"bytes":16`, `"bytes":16.5`, 1),
		strings.Replace(raw, "gate-link.mjs", "gate\u202e-link.mjs", 1),
		strings.Replace(raw, fixture.Expected.EnvironmentSHA256, strings.Repeat("a", 64), 1),
		strings.Replace(raw, `"profile_id":"resolved-top-level-executable-v1"`, `"profile_id":"other"`, 1),
	}
	for index, variant := range variants {
		if _, err := decodeCanonicalProduction([]byte(variant)); err == nil {
			t.Fatalf("drift variant %d was accepted", index)
		}
	}
}

type producerSemanticMutation struct {
	name   string
	mutate func(*ProductionPackage)
}

func TestDecodeCanonicalProductionRejectsSemanticDrift(t *testing.T) {
	fixture := loadProducerGoldenFixture(t)
	raw := []byte(fixture.Expected.CanonicalProductionJSON)
	for _, test := range producerSemanticMutations() {
		t.Run(test.name, func(t *testing.T) {
			value, err := decodeCanonicalProduction(raw)
			if err != nil {
				t.Fatal(err)
			}
			test.mutate(&value)
			encoded, err := canonicalManifest(value)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := decodeCanonicalProduction(encoded); err == nil {
				t.Fatal("semantic drift was accepted")
			}
		})
	}
}

func producerSemanticMutations() []producerSemanticMutation {
	return []producerSemanticMutation{
		{"nil environment list", func(value *ProductionPackage) {
			value.EnvironmentManifest.Variables = nil
		}},
		{"nil source list", func(value *ProductionPackage) {
			value.SourceManifest.Entries = nil
		}},
		{"nil tool hop list", func(value *ProductionPackage) {
			value.ToolManifest.SymlinkHops = nil
		}},
		{"unsafe source path", func(value *ProductionPackage) {
			value.SourceManifest.Entries[0].Path = ".forge/state"
		}},
		{"unsorted source list", func(value *ProductionPackage) {
			entries := value.SourceManifest.Entries
			entries[0], entries[len(entries)-1] = entries[len(entries)-1], entries[0]
		}},
		{"regular symlink index", mutateRegularIndexMode("120000")},
		{"symlink regular index", mutateSymlinkIndexMode("100644")},
		{"no-hop final path drift", func(value *ProductionPackage) {
			value.ToolManifest.FinalPath = "/fixture/bin/other"
		}},
		{"off-path tool hop", func(value *ProductionPackage) {
			value.ToolManifest.SymlinkHops = []SymlinkHop{{
				Path: "/other/node", Target: "/fixture/bin/node",
			}}
		}},
	}
}

func mutateRegularIndexMode(mode string) func(*ProductionPackage) {
	return func(value *ProductionPackage) {
		for index := range value.SourceManifest.Entries {
			entry := &value.SourceManifest.Entries[index]
			if entry.Kind == "regular" && entry.Tracking == "tracked" {
				entry.IndexMode = stringPointer(mode)
				return
			}
		}
	}
}

func mutateSymlinkIndexMode(mode string) func(*ProductionPackage) {
	return func(value *ProductionPackage) {
		for index := range value.SourceManifest.Entries {
			entry := &value.SourceManifest.Entries[index]
			if entry.Kind == "symlink" && entry.Tracking == "tracked" {
				entry.IndexMode = stringPointer(mode)
				return
			}
		}
	}
}

func assertGoldenProfiles(t *testing.T, value ProductionPackage, expected producerGoldenExpected) {
	t.Helper()
	environmentJSON, environmentDigest, err := digestManifest(environmentDigestDomain, value.EnvironmentManifest)
	if err != nil {
		t.Fatal(err)
	}
	toolJSON, toolDigest, err := digestManifest(toolDigestDomain, value.ToolManifest)
	if err != nil {
		t.Fatal(err)
	}
	sourceJSON, sourceDigest, err := digestManifest(sourceDigestDomain, value.SourceManifest)
	if err != nil {
		t.Fatal(err)
	}
	observationJSON, err := commandcontract.CanonicalObservationJSON(value.Observation)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenValue(t, "environment JSON", string(environmentJSON), expected.CanonicalEnvironmentManifestJSON)
	assertGoldenValue(t, "tool JSON", string(toolJSON), expected.CanonicalToolManifestJSON)
	assertGoldenValue(t, "source JSON", string(sourceJSON), expected.CanonicalSourceManifestJSON)
	assertGoldenValue(t, "observation JSON", string(observationJSON), expected.CanonicalObservationJSON)
	assertGoldenValue(t, "environment digest", environmentDigest, expected.EnvironmentSHA256)
	assertGoldenValue(t, "tool digest", toolDigest, expected.ToolSnapshotSHA256)
	assertGoldenValue(t, "source digest", sourceDigest, expected.SourceTreeSHA256)
	productionDigest := domainDigest(productionDigestDomain, []byte(expected.CanonicalProductionJSON))
	assertGoldenValue(t, "production digest", productionDigest, expected.ProductionSHA256)
}

func decodeFixtureProduction(t *testing.T, raw json.RawMessage) ProductionPackage {
	t.Helper()
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value ProductionPackage
	if err := decoder.Decode(&value); err != nil {
		t.Fatalf("decode fixture production: %v", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatal("fixture production has trailing JSON value")
	}
	if err := validateProductionPackage(value); err != nil {
		t.Fatalf("validate fixture production: %v", err)
	}
	return value
}

func loadProducerGoldenFixture(t *testing.T) producerGoldenFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures", "local-gate-command-observation-producer-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read producer fixture: %v", err)
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.DisallowUnknownFields()
	var fixture producerGoldenFixture
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatalf("decode producer fixture: %v", err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatal("producer fixture has trailing JSON value")
	}
	return fixture
}

func assertGoldenValue(t *testing.T, label, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("%s mismatch\ngot:  %s\nwant: %s", label, got, want)
	}
}
