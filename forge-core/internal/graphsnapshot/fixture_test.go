package graphsnapshot

import (
	"encoding/json"
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/gopackagedependencyobservationproducer"
	"forgeos/forge-core/internal/gopackagegraph"
)

type graphFixture struct {
	Expected struct {
		CanonicalGraphObservationJSON string `json:"canonical_graph_observation_json"`
		GraphSHA256                   string `json:"graph_sha256"`
	} `json:"expected"`
}

func loadFixtureGraph(t *testing.T) ([]byte, string, gopackagegraph.Observation) {
	t.Helper()
	raw, err := os.ReadFile("../../../docs/contracts/fixtures/local-go-package-dependency-graph-observation-producer-v1.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture graphFixture
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	graph := []byte(fixture.Expected.CanonicalGraphObservationJSON)
	observation, digest, err := gopackagedependencyobservationproducer.DecodeGraphObservation(graph)
	if err != nil || digest != fixture.Expected.GraphSHA256 {
		t.Fatalf("fixture graph decode = %q, %v", digest, err)
	}
	return graph, digest, observation
}

func buildFixture(t *testing.T) *Production {
	t.Helper()
	graph, digest, observation := loadFixtureGraph(t)
	production, err := Build(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	return production
}

func marshalObservation(t *testing.T, value gopackagegraph.Observation) ([]byte, string) {
	t.Helper()
	raw, err := canonicalJSON(value, maxGraphBytes)
	if err != nil {
		t.Fatal(err)
	}
	return raw, domainDigest(
		"forgeos.governance.local-go-package-dependency-graph-observation.v1", raw,
	)
}

func minimalObservation(directory, filePath, role string) gopackagegraph.Observation {
	importPath := "example.com/root"
	if role == "test" {
		importPath = ""
	}
	var importPointer *string
	if importPath != "" {
		importPointer = &importPath
	}
	compileFiles, testFiles := []string{}, []string{}
	if role == "compile" {
		compileFiles = []string{filePath}
	} else {
		testFiles = []string{filePath}
	}
	return gopackagegraph.Observation{
		APIVersion: gopackagegraph.APIVersion, Canonicalization: canonicalization,
		Coverage: gopackagegraph.Coverage{
			GoEntriesInSelectedSubtree: 1, RegularGoFilesParsed: 1,
			RegularGoFilesSelected: 1,
		},
		Dependencies: []gopackagegraph.Dependency{}, Diagnostics: []gopackagegraph.Diagnostic{},
		Files: []gopackagegraph.File{{
			Bytes: 1, ContentSHA256: strings.Repeat("a", 64), Imports: []string{},
			PackageName: "rootpkg", Path: filePath, Role: role,
		}},
		Module: gopackagegraph.Module{
			Directory: ".", GoModBytes: 1, GoModContentSHA256: strings.Repeat("b", 64),
			GoModPath: "go.mod", ModulePath: "example.com/root",
			NestedModules: []gopackagegraph.NestedModule{},
		},
		ObservedAtUnixMS: 7,
		Packages: []gopackagegraph.Package{{
			CompileFiles: compileFiles, Directory: directory, ImportPath: importPointer,
			Name: "rootpkg", TestFiles: testFiles,
		}},
		Producer: gopackagegraph.Producer{
			ParametersSHA256: strings.Repeat("c", 64),
			ProducerID:       "forgeos.local-go-package-dependency-graph-observer",
			ProducerType:     "tool", ProducerVersion: "v1", RunID: "fixture-root-001",
		},
		ProfileID: gopackagegraph.ProfileID, Source: gopackagegraph.Source{
			SourceRevision:   "git-sha1:" + strings.Repeat("d", 40),
			SourceTreeSHA256: strings.Repeat("e", 64),
		},
	}
}

func findNodeByComponents(t *testing.T, values []Node, components ...string) Node {
	t.Helper()
	for _, value := range values {
		if strings.Join(value.QualifiedNameComponents, "\x00") == strings.Join(components, "\x00") {
			return value
		}
	}
	t.Fatalf("node components %#v are absent", components)
	return Node{}
}
