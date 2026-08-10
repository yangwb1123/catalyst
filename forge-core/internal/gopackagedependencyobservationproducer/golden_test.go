package gopackagedependencyobservationproducer

import (
	"encoding/json"
	"os"
	"testing"

	"forgeos/forge-core/internal/gitworktreesource"
)

type goldenFixture struct {
	Expected struct {
		CanonicalGraphObservationJSON   string `json:"canonical_graph_observation_json"`
		CanonicalParametersManifestJSON string `json:"canonical_parameters_manifest_json"`
		CanonicalProductionJSON         string `json:"canonical_production_json"`
		CanonicalSourceManifestJSON     string `json:"canonical_source_manifest_json"`
		GraphSHA256                     string `json:"graph_sha256"`
		ParametersSHA256                string `json:"parameters_sha256"`
		ProductionSHA256                string `json:"production_sha256"`
		Result                          string `json:"result"`
		SourceTreeSHA256                string `json:"source_tree_sha256"`
	} `json:"expected"`
	Production ProductionPackage `json:"production"`
}

func TestGoldenFixtureMatchesCanonicalBytesAndAllDigests(t *testing.T) {
	fixture := readGoldenFixture(t)
	production, err := sealProduction(fixture.Production)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenBytes(t, "parameters", production.ParametersManifestJSON(), fixture.Expected.CanonicalParametersManifestJSON)
	assertGoldenBytes(t, "graph", production.GraphObservationJSON(), fixture.Expected.CanonicalGraphObservationJSON)
	assertGoldenBytes(t, "production", production.ProductionJSON(), fixture.Expected.CanonicalProductionJSON)
	if production.ParametersSHA256() != fixture.Expected.ParametersSHA256 ||
		production.GraphSHA256() != fixture.Expected.GraphSHA256 ||
		production.SHA256() != fixture.Expected.ProductionSHA256 ||
		production.Result() != fixture.Expected.Result {
		t.Fatalf("digest/result mismatch: %#v", fixture.Expected)
	}
	sourceJSON, err := gitworktreesource.CanonicalManifestJSON(fixture.Production.SourceManifest)
	if err != nil {
		t.Fatal(err)
	}
	assertGoldenBytes(t, "source", sourceJSON, fixture.Expected.CanonicalSourceManifestJSON)
	sourceSHA256, err := gitworktreesource.Digest(fixture.Production.SourceManifest)
	if err != nil || sourceSHA256 != fixture.Expected.SourceTreeSHA256 {
		t.Fatalf("source digest = %q, %v", sourceSHA256, err)
	}
	decoded, err := DecodeProduction([]byte(fixture.Expected.CanonicalProductionJSON))
	if err != nil || string(decoded.ProductionJSON()) != fixture.Expected.CanonicalProductionJSON {
		t.Fatalf("golden decode = %v", err)
	}
}

func readGoldenFixture(t *testing.T) goldenFixture {
	t.Helper()
	raw, err := os.ReadFile("../../../docs/contracts/fixtures/local-go-package-dependency-graph-observation-producer-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture goldenFixture
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	return fixture
}

func assertGoldenBytes(t *testing.T, label string, actual []byte, expected string) {
	t.Helper()
	if string(actual) != expected {
		t.Fatalf("%s canonical bytes differ\nactual: %s\nexpected: %s", label, actual, expected)
	}
}
