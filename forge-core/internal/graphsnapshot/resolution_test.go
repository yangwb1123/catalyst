package graphsnapshot

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/gopackagegraph"
)

func TestUnresolvedDependenciesDiagnosticsAndBoundariesMapBijectively(t *testing.T) {
	snapshot := buildFixture(t).Envelope().Snapshot
	wantResolutions := map[string]int{
		"ambiguous_local": 1, "cgo_pseudo": 1, "external_candidate": 1,
		"nested_module_boundary": 1, "stdlib_candidate": 5,
		"unresolved_local": 1, "unsupported": 1,
	}
	actual := map[string]int{}
	for _, edge := range snapshot.UnresolvedEdges {
		actual[edge.Resolution]++
		if edge.Resolution == "ambiguous_local" && len(edge.TargetCandidate.TargetNodeIDs) != 2 {
			t.Fatalf("ambiguous target crosswalk = %#v", edge.TargetCandidate)
		}
	}
	for resolution, count := range wantResolutions {
		if actual[resolution] != count {
			t.Fatalf("resolution %s count = %d, want %d (%#v)", resolution, actual[resolution], count, actual)
		}
	}
	kinds := map[string]int{}
	for _, node := range snapshot.UnresolvedNodes {
		kinds[node.Kind]++
		if node.Kind == "go_file_diagnostic" && node.SourceLocators[0].Role != "compile" {
			t.Fatalf("diagnostic locator = %#v", node.SourceLocators[0])
		}
		if node.Kind == "nested_module_boundary" && node.SourceLocators[0].Role != "go_mod" {
			t.Fatalf("boundary locator = %#v", node.SourceLocators[0])
		}
	}
	if kinds["go_file_diagnostic"] != 1 || kinds["nested_module_boundary"] != 2 {
		t.Fatalf("unresolved node kinds = %#v", kinds)
	}
}

func TestDiagnosticLiteralTestSuffixUsesTestRole(t *testing.T) {
	graph, _, observation := loadFixtureGraph(t)
	_ = graph
	observation.Diagnostics[0].Path = "service/internal/broken/bad_test.go"
	raw, digest := marshalObservation(t, observation)
	production, err := Build(raw, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	for _, node := range production.Envelope().Snapshot.UnresolvedNodes {
		if node.Kind == "go_file_diagnostic" && node.SourceLocators[0].Role != "test" {
			t.Fatalf("test diagnostic locator = %#v", node.SourceLocators[0])
		}
	}
}

func TestLocalSelfLoopRemainsAResolvedDerivedEdge(t *testing.T) {
	observation := minimalObservation(".", "root.go", "compile")
	observation.Files[0].Imports = []string{"example.com/root"}
	observation.Dependencies = []gopackagegraph.Dependency{{
		FromDirectory: ".", FromPackageName: "rootpkg", ImportPath: "example.com/root",
		Relation: "depends_on", Resolution: "local", Role: "compile",
		SourcePaths: []string{"root.go"}, TargetDirectory: stringPointerForTest("."),
		TargetPackageName: stringPointerForTest("rootpkg"),
	}}
	raw, digest := marshalObservation(t, observation)
	production, err := Build(raw, digest, observation.Producer.RunID, "fixture-root")
	if err != nil {
		t.Fatal(err)
	}
	for _, edge := range production.Envelope().Snapshot.Edges {
		if edge.Relation == "depends_on" {
			if edge.FromNodeID != edge.ToNodeID || edge.EpistemicStatus != "derived" {
				t.Fatalf("self dependency = %#v", edge)
			}
			return
		}
	}
	t.Fatal("resolved self dependency is absent")
}

func TestFileRenameKeepsSemanticIDAndChangesFullRecord(t *testing.T) {
	graph, digest, observation := loadFixtureGraph(t)
	first, err := Build(graph, digest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	oldPath, newPath := "service/cmd/app/main.go", "service/cmd/app/renamed.go"
	observation.Files[0].Path = newPath
	observation.Packages[0].CompileFiles[0] = newPath
	for index := range observation.Dependencies {
		for pathIndex, path := range observation.Dependencies[index].SourcePaths {
			if path == oldPath {
				observation.Dependencies[index].SourcePaths[pathIndex] = newPath
			}
		}
	}
	raw, changedDigest := marshalObservation(t, observation)
	second, err := Build(raw, changedDigest, observation.Producer.RunID, "fixture-catalyst-go")
	if err != nil {
		t.Fatal(err)
	}
	components := []string{"example.com/service", "cmd/app", "main"}
	left := findNodeByComponents(t, first.Envelope().Snapshot.Nodes, components...)
	right := findNodeByComponents(t, second.Envelope().Snapshot.Nodes, components...)
	if left.NodeID != right.NodeID || left.NodeSHA256 == right.NodeSHA256 ||
		!strings.Contains(string(second.JSON()), newPath) {
		t.Fatal("file rename did not preserve semantic ID while changing the record")
	}
}

func stringPointerForTest(value string) *string { return &value }
