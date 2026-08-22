package goimpactprescan

import (
	"fmt"
	"strings"
	"testing"

	"forgeos/forge-core/internal/gopackagegraph"
)

func TestAggregateWitnessBoundFailsWithoutPartialReport(t *testing.T) {
	within := chainObservation(362)
	graph := marshalGraph(within)
	production, err := Build(
		graph, graphDigest(graph), within.Producer.RunID, []string{"p0000/file.go"},
	)
	if err != nil || production == nil {
		t.Fatalf("largest triangular witness sum under bound: %v", err)
	}
	report := production.Envelope().Report
	if len(report.ReachableNodes) != 362 || report.ReachableNodes[len(report.ReachableNodes)-1].Witness.HopCount < 0 {
		t.Fatalf("reachable nodes = %d", len(report.ReachableNodes))
	}
	over := chainObservation(363)
	graph = marshalGraph(over)
	production, err = Build(
		graph, graphDigest(graph), over.Producer.RunID, []string{"p0000/file.go"},
	)
	if err == nil || production != nil {
		t.Fatalf("over aggregate witness bound = nonnil/%v", err)
	}
}

func TestAggregateWitnessExactHelperBoundary(t *testing.T) {
	edge := ReachableEdge{EdgeID: "edge", FromNodeID: "from", ToNodeID: "to"}
	index := &graphIndex{reverse: map[string][]ReachableEdge{"to": {edge}}}
	known := map[string]Witness{"to": {
		EdgeIDs: []string{}, HopCount: 0, NodeIDs: []string{"to"}, SeedNodeID: "to",
	}}
	next, hops, err := expandFrontier(
		index, []string{"to"}, known, maxAggregateWitnesses-1,
	)
	if err != nil || hops != 1 || len(next) != 1 {
		t.Fatalf("aggregate witness exact bound = %d/%d/%v", len(next), hops, err)
	}
	if _, _, err := expandFrontier(
		index, []string{"to"}, known, maxAggregateWitnesses,
	); err == nil {
		t.Fatal("aggregate witness over bound unexpectedly succeeded")
	}
}

func chainObservation(count int) gopackagegraph.Observation {
	files, packages := chainFilesAndPackages(count)
	return gopackagegraph.Observation{
		APIVersion: gopackagegraph.APIVersion, Canonicalization: Canonicalization,
		Coverage: gopackagegraph.Coverage{
			GoEntriesInSelectedSubtree: int64(count), RegularGoFilesParsed: int64(count),
			RegularGoFilesSelected: int64(count),
		},
		Dependencies: chainDependencies(count), Diagnostics: []gopackagegraph.Diagnostic{},
		Files: files, Module: gopackagegraph.Module{
			Directory: ".", GoModBytes: 24, GoModContentSHA256: strings.Repeat("4", 64),
			GoModPath: "go.mod", ModulePath: "example.com/chain",
			NestedModules: []gopackagegraph.NestedModule{},
		},
		ObservedAtUnixMS: 1, Packages: packages,
		Producer: gopackagegraph.Producer{
			ParametersSHA256: strings.Repeat("5", 64),
			ProducerID:       "forgeos.local-go-package-dependency-graph-observer",
			ProducerType:     "tool", ProducerVersion: "v1", RunID: "impact-chain-001",
		},
		ProfileID: gopackagegraph.ProfileID, Source: gopackagegraph.Source{
			SourceRevision:   "git-sha1:3333333333333333333333333333333333333333",
			SourceTreeSHA256: strings.Repeat("6", 64),
		},
	}
}

func chainFilesAndPackages(count int) ([]gopackagegraph.File, []gopackagegraph.Package) {
	files := make([]gopackagegraph.File, 0, count)
	packages := make([]gopackagegraph.Package, 0, count)
	for index := 0; index < count; index++ {
		directory := fmt.Sprintf("p%04d", index)
		path := directory + "/file.go"
		imports := []string{}
		if index > 0 {
			imports = []string{fmt.Sprintf("example.com/chain/p%04d", index-1)}
		}
		files = append(files, graphFile(path, "p", "compile", imports...))
		importPath := "example.com/chain/" + directory
		packages = append(packages, gopackagegraph.Package{
			CompileFiles: []string{path}, Directory: directory, ImportPath: &importPath,
			Name: "p", TestFiles: []string{},
		})
	}
	return files, packages
}

func chainDependencies(count int) []gopackagegraph.Dependency {
	result := make([]gopackagegraph.Dependency, 0, count-1)
	for index := 1; index < count; index++ {
		from := fmt.Sprintf("p%04d", index)
		to := fmt.Sprintf("p%04d", index-1)
		result = append(result, localDependency(
			from, "p", "example.com/chain/"+to, "compile",
			from+"/file.go", to, "p",
		))
	}
	return result
}

func TestReachableNodeBoundAtAndOver(t *testing.T) {
	within := starObservation(maxReachableNodes)
	index, err := indexGraph(within)
	if err != nil {
		t.Fatalf("reachable-node bound within: %v", err)
	}
	seed := index.nodesByKey[packageKey{directory: boundDirectory(0), name: "p"}].NodeID
	nodes, _, err := reverseClosure(index, map[string]struct{}{seed: {}})
	if err != nil || len(nodes) != maxReachableNodes {
		t.Fatalf("reachable nodes = %d/%v", len(nodes), err)
	}
	if _, err := indexGraph(starObservation(maxReachableNodes + 1)); err == nil {
		t.Fatal("over reachable-node bound unexpectedly succeeded")
	}
}

func TestResolvedSeedBoundAtLimit(t *testing.T) {
	observation := starObservation(maxChangedPaths)
	paths := make([]string, maxChangedPaths)
	for index := range paths {
		paths[index] = boundDirectory(index) + "/file.go"
	}
	graph := marshalGraph(observation)
	production, err := Build(graph, graphDigest(graph), observation.Producer.RunID, paths)
	if err != nil || production == nil {
		t.Fatalf("resolved-seed bound within: %v", err)
	}
	if count := len(production.Envelope().Report.ResolvedSeeds); count != maxChangedPaths {
		t.Fatalf("resolved seeds = %d", count)
	}
}

func TestReachableEdgeBoundAtAndOver(t *testing.T) {
	within, err := indexGraph(denseEdgeObservation(false))
	if err != nil {
		t.Fatalf("reachable-edge bound within: %v", err)
	}
	seed := within.nodesByKey[packageKey{directory: boundDirectory(0), name: "p"}].NodeID
	nodes, edges, err := reverseClosure(within, map[string]struct{}{seed: {}})
	if err != nil || len(nodes) != maxReachableNodes || len(edges) != maxReachableEdges {
		t.Fatalf("reachable graph = %d nodes/%d edges/%v", len(nodes), len(edges), err)
	}
	if _, err := indexGraph(denseEdgeObservation(true)); err == nil {
		t.Fatal("over reachable-edge bound unexpectedly succeeded")
	}
}

func TestLocalEdgeSourcePathBoundAtAndOver(t *testing.T) {
	index := twoNodeIndex(t)
	if _, err := makeEdge(index, edgeWithSourcePaths(16_384)); err != nil {
		t.Fatalf("source-path bound within: %v", err)
	}
	if _, err := makeEdge(index, edgeWithSourcePaths(16_385)); err == nil {
		t.Fatal("over source-path bound unexpectedly succeeded")
	}
}

func TestEncodeGraphObservationBoundAtAndOver(t *testing.T) {
	// A semantically valid ADR-0053 graph this large is cumbersome to synthesize,
	// so the exact 16 MiB helper boundary is locked directly here.
	encoded, err := encodeGraphObservation(make([]byte, maxGraphBytes))
	if err != nil || len(encoded) != 22_369_622 {
		t.Fatalf("encoded graph bound = %d/%v", len(encoded), err)
	}
	if _, err := encodeGraphObservation(make([]byte, maxGraphBytes+1)); err == nil {
		t.Fatal("oversized graph unexpectedly encoded")
	}
}

func TestPathAndRunIDBoundsAtAndOver(t *testing.T) {
	if !validRepoPath(strings.Repeat("a", 4_096)) {
		t.Fatal("4096-scalar path rejected")
	}
	if !validRepoPath(strings.Repeat("𐐷", 4_096)) {
		t.Fatal("16KiB path rejected")
	}
	if validRepoPath(strings.Repeat("a", 4_097)) || validRepoPath(strings.Repeat("𐐷", 4_097)) {
		t.Fatal("over path bounds accepted")
	}
	assertRunIDBound(t, strings.Repeat("a", 160), true)
	assertRunIDBound(t, strings.Repeat("a", 161), false)
}

func TestWitnessHopBoundAtAndOver(t *testing.T) {
	within, err := extendWitness(Witness{HopCount: maxWitnessHops - 1}, ReachableEdge{EdgeID: "edge"})
	if err != nil || within.HopCount != maxWitnessHops {
		t.Fatalf("witness bound within = %+v/%v", within, err)
	}
	if _, err := extendWitness(Witness{HopCount: maxWitnessHops}, ReachableEdge{EdgeID: "edge"}); err == nil {
		t.Fatal("over witness-hop bound unexpectedly succeeded")
	}
}

func starObservation(count int) gopackagegraph.Observation {
	files, packages, dependencies := starFilesPackagesDependencies(count)
	return gopackagegraph.Observation{
		APIVersion: gopackagegraph.APIVersion, Canonicalization: Canonicalization,
		Coverage: gopackagegraph.Coverage{
			GoEntriesInSelectedSubtree: int64(count), RegularGoFilesParsed: int64(count),
			RegularGoFilesSelected: int64(count),
		},
		Dependencies: dependencies, Diagnostics: []gopackagegraph.Diagnostic{},
		Files: files, Module: boundModule("example.com/star"),
		ObservedAtUnixMS: 1, Packages: packages, Producer: boundProducer("impact-star-001"),
		ProfileID: gopackagegraph.ProfileID, Source: boundSource(),
	}
}

func starFilesPackagesDependencies(count int) ([]gopackagegraph.File, []gopackagegraph.Package, []gopackagegraph.Dependency) {
	files := make([]gopackagegraph.File, 0, count)
	packages := make([]gopackagegraph.Package, 0, count)
	dependencies := make([]gopackagegraph.Dependency, 0, count-1)
	for index := 0; index < count; index++ {
		directory := boundDirectory(index)
		path := directory + "/file.go"
		imports := []string{}
		if index > 0 {
			imports = []string{boundImportPath("example.com/star", 0)}
			dependencies = append(dependencies, localDependency(
				directory, "p", imports[0], "compile", path, boundDirectory(0), "p",
			))
		}
		files = append(files, graphFile(path, "p", "compile", imports...))
		packages = append(packages, boundPackage("example.com/star", directory, path))
	}
	return files, packages, dependencies
}

func denseEdgeObservation(extra bool) gopackagegraph.Observation {
	modulePath := "example.com/dense"
	packages := make([]gopackagegraph.Package, 0, maxReachableNodes)
	dependencies := make([]gopackagegraph.Dependency, 0, maxReachableEdges+1)
	for index := 0; index < maxReachableNodes; index++ {
		directory := boundDirectory(index)
		packages = append(packages, boundPackage(modulePath, directory, directory+"/file.go"))
	}
	for index := 1; index < maxReachableNodes; index++ {
		dependencies = appendDenseEdges(dependencies, modulePath, index, 0, 4)
	}
	dependencies = appendDenseEdges(dependencies, modulePath, 0, 1, 5)
	if extra {
		dependencies = append(dependencies, localDependency(
			boundDirectory(0), "p", boundImportPath(modulePath, 5), "compile",
			starSeedPath(), boundDirectory(5), "p",
		))
	}
	return gopackagegraph.Observation{Module: boundModule(modulePath), Packages: packages, Dependencies: dependencies}
}

func appendDenseEdges(
	values []gopackagegraph.Dependency,
	modulePath string,
	fromIndex, start, end int,
) []gopackagegraph.Dependency {
	for target := start; target < end; target++ {
		values = append(values, localDependency(
			boundDirectory(fromIndex), "p", boundImportPath(modulePath, target), "compile",
			boundDirectory(fromIndex)+"/file.go", boundDirectory(target), "p",
		))
	}
	return values
}

func assertRunIDBound(t *testing.T, runID string, wantOK bool) {
	t.Helper()
	observation := completeObservation()
	observation.Producer.RunID = runID
	graph := marshalGraph(observation)
	production, err := Build(graph, graphDigest(graph), runID, []string{"service/z/z.go"})
	if wantOK && (err != nil || production == nil) {
		t.Fatalf("run_id length %d rejected: %v", len(runID), err)
	}
	if !wantOK && (err == nil || production != nil) {
		t.Fatalf("run_id length %d unexpectedly accepted", len(runID))
	}
}

func twoNodeIndex(t *testing.T) *graphIndex {
	t.Helper()
	modulePath := "example.com/two"
	index, err := indexGraph(gopackagegraph.Observation{
		Module: boundModule(modulePath),
		Packages: []gopackagegraph.Package{
			boundPackage(modulePath, boundDirectory(0), starSeedPath()),
			boundPackage(modulePath, boundDirectory(1), boundDirectory(1)+"/file.go"),
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	return index
}

func edgeWithSourcePaths(count int) gopackagegraph.Dependency {
	paths := make([]string, count)
	for index := range paths {
		paths[index] = fmt.Sprintf("p00001/f%05d.go", index)
	}
	return gopackagegraph.Dependency{
		FromDirectory: boundDirectory(1), FromPackageName: "p",
		ImportPath: boundImportPath("example.com/two", 0), Relation: "depends_on",
		Resolution: "local", ResolutionDetail: nil, Role: "compile",
		SourcePaths: paths, TargetDirectory: testStringPointer(boundDirectory(0)),
		TargetPackageName: testStringPointer("p"),
	}
}

func boundDirectory(index int) string { return fmt.Sprintf("p%05d", index) }

func boundImportPath(modulePath string, index int) string {
	return modulePath + "/" + boundDirectory(index)
}

func boundPackage(modulePath, directory, path string) gopackagegraph.Package {
	importPath := modulePath + "/" + directory
	return gopackagegraph.Package{
		CompileFiles: []string{path}, Directory: directory, ImportPath: &importPath,
		Name: "p", TestFiles: []string{},
	}
}

func boundModule(modulePath string) gopackagegraph.Module {
	return gopackagegraph.Module{
		Directory: ".", GoModBytes: 24, GoModContentSHA256: strings.Repeat("4", 64),
		GoModPath: "go.mod", ModulePath: modulePath, NestedModules: []gopackagegraph.NestedModule{},
	}
}

func boundProducer(runID string) gopackagegraph.Producer {
	return gopackagegraph.Producer{
		ParametersSHA256: strings.Repeat("5", 64),
		ProducerID:       "forgeos.local-go-package-dependency-graph-observer",
		ProducerType:     "tool", ProducerVersion: "v1", RunID: runID,
	}
}

func boundSource() gopackagegraph.Source {
	return gopackagegraph.Source{
		SourceRevision:   "git-sha1:3333333333333333333333333333333333333333",
		SourceTreeSHA256: strings.Repeat("6", 64),
	}
}

func starSeedPath() string { return boundDirectory(0) + "/file.go" }

func testStringPointer(value string) *string { return &value }
