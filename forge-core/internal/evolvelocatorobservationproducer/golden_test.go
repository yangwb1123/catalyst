package evolvelocatorobservationproducer

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	locatorcontract "forgeos/forge-core/internal/evolverepolocatorevidencecontract"
	"forgeos/forge-core/internal/gitworktreesource"
)

const (
	producerFixtureAPIVersion = "forgeos.governance.local-evolve-repo-locator-observation-production.fixture/v1"
	pureFixtureSemantics      = "PURE_CONTRACT_FIXTURE (deterministic bytes only; no live repository capture, scan judgment, completion, truth, authority, identity, persistence, or effect attestation)"
)

type goldenFixture struct {
	APIVersion       string            `json:"api_version"`
	Expected         goldenExpected    `json:"expected"`
	FixtureSemantics string            `json:"fixture_semantics"`
	Preimages        goldenPreimages   `json:"preimages"`
	Production       ProductionPackage `json:"production"`
}

type goldenExpected struct {
	CanonicalObservationJSONs       []string `json:"canonical_observation_jsons"`
	CanonicalParametersManifestJSON string   `json:"canonical_parameters_manifest_json"`
	CanonicalProductionJSON         string   `json:"canonical_production_json"`
	CanonicalReportManifestJSON     string   `json:"canonical_report_manifest_json"`
	CanonicalSourceManifestJSON     string   `json:"canonical_source_manifest_json"`
	ParametersSHA256                string   `json:"parameters_sha256"`
	ProductionSHA256                string   `json:"production_sha256"`
	ReportSHA256                    string   `json:"report_sha256"`
	Result                          string   `json:"result"`
	SourceTreeSHA256                string   `json:"source_tree_sha256"`
}

type goldenPreimages struct {
	SourceRegularFiles []goldenFile `json:"source_regular_files"`
}

type goldenFile struct {
	Path string `json:"path"`
	UTF8 string `json:"utf8"`
}

func TestGoldenLocalEvolveLocatorObservationProduction(t *testing.T) {
	fixture := loadGoldenFixture(t)
	if fixture.APIVersion != producerFixtureAPIVersion || fixture.FixtureSemantics != pureFixtureSemantics {
		t.Fatalf("fixture identity/semantics drifted: %#v", fixture)
	}
	production, err := sealProduction(fixture.Production)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenBytes(t, "production", production.ProductionJSON(), fixture.Expected.CanonicalProductionJSON)
	assertGoldenValue(t, "production SHA", production.SHA256(), fixture.Expected.ProductionSHA256)
	assertGoldenValue(t, "result", production.Result(), fixture.Expected.Result)
	assertGoldenProfiles(t, fixture)
	assertGoldenObservations(t, production, fixture.Expected.CanonicalObservationJSONs)
	assertGoldenPreimages(t, fixture)
	decoded, err := DecodeProduction([]byte(fixture.Expected.CanonicalProductionJSON))
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(decoded, fixture.Production) {
		t.Fatal("strict golden decode differs from pretty fixture production")
	}
}

func assertGoldenProfiles(t *testing.T, fixture goldenFixture) {
	t.Helper()
	parametersJSON, err := canonicalJSON(fixture.Production.ParametersManifest)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenBytes(t, "parameters", parametersJSON, fixture.Expected.CanonicalParametersManifestJSON)
	assertGoldenValue(t, "parameters SHA", domainDigest(parametersDigestDomain, parametersJSON), fixture.Expected.ParametersSHA256)
	reportJSON, err := canonicalJSON(fixture.Production.ReportManifest)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenBytes(t, "report", reportJSON, fixture.Expected.CanonicalReportManifestJSON)
	assertGoldenValue(t, "report SHA", fixture.Production.ReportManifest.SHA256, fixture.Expected.ReportSHA256)
	sourceJSON, err := gitworktreesource.CanonicalManifestJSON(fixture.Production.SourceManifest)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenBytes(t, "source", sourceJSON, fixture.Expected.CanonicalSourceManifestJSON)
	sourceSHA, err := gitworktreesource.Digest(fixture.Production.SourceManifest)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenValue(t, "source SHA", sourceSHA, fixture.Expected.SourceTreeSHA256)
}

func assertGoldenObservations(t *testing.T, production *Production, expected []string) {
	t.Helper()
	if len(expected) != len(production.Package().Observations) {
		t.Fatalf("golden observation count = %d, want %d", len(expected), len(production.Package().Observations))
	}
	for index, want := range expected {
		assertGoldenBytes(t, "observation", production.ObservationJSON(index), want)
		canonical, err := locatorcontract.CanonicalObservationJSON(production.Package().Observations[index])
		if err != nil {
			t.Fatal(err)
		}
		assertGoldenBytes(t, "standalone observation", canonical, want)
	}
}

func assertGoldenPreimages(t *testing.T, fixture goldenFixture) {
	t.Helper()
	regular := make(map[string]gitworktreesource.SourceEntry)
	for _, entry := range fixture.Production.SourceManifest.Entries {
		if entry.Kind == "regular" {
			regular[entry.Path] = entry
		}
	}
	if len(regular) != len(fixture.Preimages.SourceRegularFiles) {
		t.Fatal("fixture regular-file preimages are not a complete closed set")
	}
	for _, preimage := range fixture.Preimages.SourceRegularFiles {
		entry, exists := regular[preimage.Path]
		bytes := []byte(preimage.UTF8)
		if !exists || entry.ContentSHA256 == nil || entry.Bytes != int64(len(bytes)) ||
			*entry.ContentSHA256 != sha256Bytes(bytes) {
			t.Fatalf("fixture preimage does not bind %q", preimage.Path)
		}
		delete(regular, preimage.Path)
	}
}

func loadGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	path := filepath.Join("..", "..", "..", "docs", "contracts", "fixtures",
		"local-evolve-repo-locator-observation-producer-v1.json")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	var fixture goldenFixture
	if err := json.Unmarshal(data, &fixture); err != nil {
		t.Fatal(err)
	}
	return fixture
}

func assertGoldenBytes(t *testing.T, label string, got []byte, want string) {
	t.Helper()
	if string(got) != want {
		t.Fatalf("%s bytes drifted\ngot:  %s\nwant: %s", label, got, want)
	}
}

func assertGoldenValue(t *testing.T, label, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("%s = %q, want %q", label, got, want)
	}
}
