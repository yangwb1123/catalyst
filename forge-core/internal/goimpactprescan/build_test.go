package goimpactprescan

import (
	"bytes"
	"encoding/json"
	"reflect"
	"sort"
	"testing"

	"forgeos/forge-core/internal/gopackagegraph"
)

var richChangedPaths = []string{
	"README.md",
	"service/broken/bad.go",
	"service/d/d.go",
	"service/d/d_extra.go",
	"service/deleted.go",
	"service/nested/x.go",
	"service/notes.txt",
	"service/z/z.go",
}

func TestBuildRichExactReverseClosure(t *testing.T) {
	graph := marshalGraph(richObservation())
	production, err := Build(graph, graphDigest(graph), "impact-fixture-001", richChangedPaths)
	if err != nil {
		t.Fatal(err)
	}
	value := production.Envelope()
	assertRichSeeds(t, value.Report)
	if len(value.Report.ReachableNodes) != 9 || len(value.Report.ReachableEdges) != 10 {
		t.Fatalf("reachable nodes/edges = %d/%d", len(value.Report.ReachableNodes), len(value.Report.ReachableEdges))
	}
	assertRichClosureStatus(t, value.Report)
	assertWitnessTieBreak(t, value.Report)
	assertCycleAndInducedEdge(t, value.Report)
	assertTestEdge(t, value.Report)
	decoded, err := Decode(production.JSON())
	if err != nil || !bytes.Equal(decoded.JSON(), production.JSON()) {
		t.Fatalf("decode exact production: %v", err)
	}
}

func assertRichSeeds(t *testing.T, report Report) {
	t.Helper()
	if len(report.ResolvedSeeds) != 2 || len(report.UnresolvedSeeds) != 5 {
		t.Fatalf("resolved/unresolved seeds = %d/%d", len(report.ResolvedSeeds), len(report.UnresolvedSeeds))
	}
	foundGrouped := false
	for _, seed := range report.ResolvedSeeds {
		if reflect.DeepEqual(seed.ChangedPaths, []string{"service/d/d.go", "service/d/d_extra.go"}) {
			foundGrouped = true
		}
	}
	if !foundGrouped {
		t.Fatal("same-package changed paths were not grouped")
	}
	wantReasons := []string{
		"outside_selected_module", "go_file_diagnostic", "not_in_observed_file_or_diagnostic",
		"inside_nested_module_boundary", "not_a_go_file",
	}
	for index, seed := range report.UnresolvedSeeds {
		if seed.Reason != wantReasons[index] {
			t.Fatalf("unresolved reason %d = %q", index, seed.Reason)
		}
	}
	if report.UnresolvedSeeds[1].DiagnosticCode == nil ||
		*report.UnresolvedSeeds[1].DiagnosticCode != "go_file_parse_error" {
		t.Fatal("diagnostic seed did not retain its stable code")
	}
}

func assertRichClosureStatus(t *testing.T, report Report) {
	t.Helper()
	want := []string{
		"changed_path_unresolved", "go_file_diagnostic_present",
		"nested_module_boundary_dependency_present",
		"unresolved_local_dependency_present", "unsupported_import_dependency_present",
	}
	if report.PackageLexicalClosureStatus != Unknown ||
		!reflect.DeepEqual(report.ClosureReasonCodes, want) {
		t.Fatalf("closure status/reasons = %q/%v", report.PackageLexicalClosureStatus, report.ClosureReasonCodes)
	}
	if report.SystemImpactStatus != Unknown ||
		!reflect.DeepEqual(report.SystemUnknownReasonCodes, systemUnknownReasons) {
		t.Fatal("system impact was not kept canonically unknown")
	}
}

func assertWitnessTieBreak(t *testing.T, report Report) {
	t.Helper()
	nodes := reportNodeIndex(report)
	edges := reportEdgePairIndex(report)
	for _, target := range []string{"service/a", "service/e"} {
		left := candidateWitness(nodes, edges, target, "service/b")
		right := candidateWitness(nodes, edges, target, "service/c")
		want := right
		if witnessLess(left, right) {
			want = left
		}
		if !reflect.DeepEqual(nodes[target].Witness, want) {
			t.Fatalf("%s witness did not use deterministic tie-break", target)
		}
	}
}

func candidateWitness(
	nodes map[string]ReachableNode,
	edges map[string]ReachableEdge,
	target, middle string,
) Witness {
	seed := nodes["service/d"].NodeID
	return Witness{
		EdgeIDs:    []string{edges[middle+">service/d"].EdgeID, edges[target+">"+middle].EdgeID},
		HopCount:   2,
		NodeIDs:    []string{seed, nodes[middle].NodeID, nodes[target].NodeID},
		SeedNodeID: seed,
	}
}

func assertCycleAndInducedEdge(t *testing.T, report Report) {
	t.Helper()
	nodes := reportNodeIndex(report)
	edges := reportEdgePairIndex(report)
	if nodes["service/f"].Witness.HopCount != 1 || nodes["service/g"].Witness.HopCount != 2 {
		t.Fatalf("cycle witnesses = %d/%d", nodes["service/f"].Witness.HopCount, nodes["service/g"].Witness.HopCount)
	}
	cycleEdge := edges["service/f>service/g"].EdgeID
	for _, edgeID := range nodes["service/g"].Witness.EdgeIDs {
		if edgeID == cycleEdge {
			t.Fatal("non-shortest cycle edge unexpectedly entered the witness")
		}
	}
	if cycleEdge == "" {
		t.Fatal("full induced closure omitted a non-witness cycle edge")
	}
}

func assertTestEdge(t *testing.T, report Report) {
	t.Helper()
	for _, edge := range report.ReachableEdges {
		if edge.Role == "test" {
			return
		}
	}
	t.Fatal("test-role local dependency was omitted")
}

func reportNodeIndex(report Report) map[string]ReachableNode {
	result := make(map[string]ReachableNode, len(report.ReachableNodes))
	for _, node := range report.ReachableNodes {
		result[node.Directory] = node
	}
	return result
}

func reportEdgePairIndex(report Report) map[string]ReachableEdge {
	nodeDirectories := make(map[string]string, len(report.ReachableNodes))
	for _, node := range report.ReachableNodes {
		nodeDirectories[node.NodeID] = node.Directory
	}
	result := make(map[string]ReachableEdge, len(report.ReachableEdges))
	for _, edge := range report.ReachableEdges {
		key := nodeDirectories[edge.FromNodeID] + ">" + nodeDirectories[edge.ToNodeID]
		result[key] = edge
	}
	return result
}

func TestBuildCompleteObservationAndNoDependentRemainSystemUnknown(t *testing.T) {
	graph := marshalGraph(completeObservation())
	production, err := Build(graph, graphDigest(graph), "impact-fixture-001", []string{"service/z/z.go"})
	if err != nil {
		t.Fatal(err)
	}
	report := production.Envelope().Report
	if report.PackageLexicalClosureStatus != Complete || len(report.ClosureReasonCodes) != 0 {
		t.Fatalf("package closure = %q/%v", report.PackageLexicalClosureStatus, report.ClosureReasonCodes)
	}
	if len(report.ReachableNodes) != 1 || len(report.ReachableEdges) != 0 ||
		report.SystemImpactStatus != Unknown {
		t.Fatalf("zero-dependent closure/system = %d/%d/%q", len(report.ReachableNodes), len(report.ReachableEdges), report.SystemImpactStatus)
	}
	typedJSON, err := json.Marshal(production.Envelope())
	if err != nil || !bytes.Equal(typedJSON, production.JSON()) {
		t.Fatalf("typed envelope lost canonical empty arrays: %v", err)
	}
}

func TestClosureStatusTreatsAmbiguousAsGapButIgnoresNonGapCandidates(t *testing.T) {
	observation := completeObservation()
	observation.Dependencies = append(observation.Dependencies,
		ambiguousDependency(), externalCandidateDependency(),
		stdlibCandidateDependency(), cgoPseudoDependency(),
	)
	status, reasons := closureStatus(observation, nil)
	if status != Unknown {
		t.Fatalf("closure status = %q", status)
	}
	want := []string{"ambiguous_local_dependency_present"}
	if !reflect.DeepEqual(reasons, want) {
		t.Fatalf("closure reasons = %v", reasons)
	}
}

func TestBuildIsDeterministicAndRejectsInputDrift(t *testing.T) {
	graph := marshalGraph(richObservation())
	digest := graphDigest(graph)
	first, err := Build(graph, digest, "impact-fixture-001", richChangedPaths)
	if err != nil {
		t.Fatal(err)
	}
	second, err := Build(graph, digest, "impact-fixture-001", richChangedPaths)
	if err != nil || !bytes.Equal(first.JSON(), second.JSON()) {
		t.Fatalf("repeat build = %v/equal %v", err, bytes.Equal(first.JSON(), second.JSON()))
	}
	invalid := []struct {
		name  string
		graph []byte
		hash  string
		runID string
		paths []string
	}{
		{name: "digest", graph: graph, hash: "0" + digest[1:], runID: "impact-fixture-001", paths: richChangedPaths},
		{name: "run", graph: graph, hash: digest, runID: "other", paths: richChangedPaths},
		{name: "noncanonical", graph: append(append([]byte{}, graph...), '\n'), hash: digest, runID: "impact-fixture-001", paths: richChangedPaths},
		{name: "unsorted", graph: graph, hash: digest, runID: "impact-fixture-001", paths: []string{"service/z/z.go", "service/d/d.go"}},
		{name: "duplicate", graph: graph, hash: digest, runID: "impact-fixture-001", paths: []string{"service/d/d.go", "service/d/d.go"}},
		{name: "invalid UTF-8", graph: graph, hash: digest, runID: "impact-fixture-001", paths: []string{"service/\xff.go"}},
	}
	for _, test := range invalid {
		t.Run(test.name, func(t *testing.T) {
			if value, err := Build(test.graph, test.hash, test.runID, test.paths); err == nil || value != nil {
				t.Fatalf("Build() = nonnil/%v", err)
			}
		})
	}
}

func ambiguousDependency() gopackagegraph.Dependency {
	detail, target := "multiple_compile_packages", "service/dupe"
	return gopackagegraph.Dependency{
		FromDirectory: "service/a", FromPackageName: "a",
		ImportPath: "example.com/service/dupe", Relation: "depends_on",
		Resolution: "ambiguous_local", ResolutionDetail: &detail,
		Role: "compile", SourcePaths: []string{"service/a/a.go"}, TargetDirectory: &target,
	}
}

func externalCandidateDependency() gopackagegraph.Dependency {
	return gopackagegraph.Dependency{
		FromDirectory: "service/a", FromPackageName: "a",
		ImportPath: "github.com/acme/lib", Relation: "depends_on",
		Resolution: "external_candidate", ResolutionDetail: nil,
		Role: "compile", SourcePaths: []string{"service/a/a.go"},
	}
}

func stdlibCandidateDependency() gopackagegraph.Dependency {
	return gopackagegraph.Dependency{
		FromDirectory: "service/a", FromPackageName: "a",
		ImportPath: "fmt", Relation: "depends_on",
		Resolution: "stdlib_candidate", ResolutionDetail: nil,
		Role: "compile", SourcePaths: []string{"service/a/a.go"},
	}
}

func cgoPseudoDependency() gopackagegraph.Dependency {
	return gopackagegraph.Dependency{
		FromDirectory: "service/a", FromPackageName: "a",
		ImportPath: "C", Relation: "depends_on",
		Resolution: "cgo_pseudo", ResolutionDetail: nil,
		Role: "compile", SourcePaths: []string{"service/a/a.go"},
	}
}

func TestBuildRejectsGraphSemanticDrift(t *testing.T) {
	value := richObservation()
	value.Coverage.RegularGoFilesParsed--
	graph := marshalGraph(value)
	if production, err := Build(graph, graphDigest(graph), "impact-fixture-001", richChangedPaths); err == nil || production != nil {
		t.Fatalf("Build() = nonnil/%v", err)
	}
}

func TestDecodeRejectsEveryEnvelopeLayerTamper(t *testing.T) {
	graph := marshalGraph(richObservation())
	production, err := Build(graph, graphDigest(graph), "impact-fixture-001", richChangedPaths)
	if err != nil {
		t.Fatal(err)
	}
	for _, path := range [][]string{
		{"envelope_sha256"}, {"request", "request_sha256"},
		{"report", "report_sha256"}, {"report", "system_impact_status"},
		{"report", "reachable_nodes", "0", "node_sha256"},
		{"report", "reachable_edges", "0", "edge_sha256"},
	} {
		t.Run(path[len(path)-1], func(t *testing.T) {
			tampered := tamperJSON(t, production.JSON(), path)
			if decoded, err := Decode(tampered); err == nil || decoded != nil {
				t.Fatalf("Decode() = nonnil/%v", err)
			}
		})
	}
}

func TestDecodeRejectsDuplicateFloatAndDeepJSON(t *testing.T) {
	tests := [][]byte{
		[]byte(`{"api_version":"x","api_version":"x"}`),
		[]byte(`{"api_version":1.5}`),
		[]byte(`{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":{"a":0}}}}}}}}}}}}}}}}}`),
	}
	for index, raw := range tests {
		if value, err := Decode(raw); err == nil || value != nil {
			t.Errorf("case %d = nonnil/%v", index, err)
		}
	}
}

func tamperJSON(t *testing.T, raw []byte, path []string) []byte {
	t.Helper()
	var value any
	if err := json.Unmarshal(raw, &value); err != nil {
		t.Fatal(err)
	}
	cursor := value
	for _, component := range path[:len(path)-1] {
		if object, ok := cursor.(map[string]any); ok {
			cursor = object[component]
			continue
		}
		index := int(component[0] - '0')
		cursor = cursor.([]any)[index]
	}
	object := cursor.(map[string]any)
	key := path[len(path)-1]
	object[key] = mutateScalar(object[key])
	encoded, err := json.Marshal(value)
	if err != nil {
		t.Fatal(err)
	}
	return encoded
}

func mutateScalar(value any) any {
	if text, ok := value.(string); ok {
		if text == Unknown {
			return Complete
		}
		if len(text) != 0 {
			first := byte('0')
			if text[0] == '0' {
				first = '1'
			}
			return string(first) + text[1:]
		}
	}
	return "tampered"
}

func TestChangedPathAndGraphBoundsFailBeforeReport(t *testing.T) {
	graph := marshalGraph(richObservation())
	paths := make([]string, maxChangedPaths)
	for index := range paths {
		paths[index] = "service/absent/path" + leftPad(index, 3) + ".go"
	}
	production, err := Build(graph, graphDigest(graph), "impact-fixture-001", paths)
	if err != nil || production == nil {
		t.Fatalf("at changed-path bound: %v", err)
	}
	if len(production.Envelope().Report.UnresolvedSeeds) != maxChangedPaths {
		t.Fatalf("unresolved seeds = %d", len(production.Envelope().Report.UnresolvedSeeds))
	}
	paths = append(paths, "service/absent/zzzz.go")
	if production, err := Build(graph, graphDigest(graph), "impact-fixture-001", paths); err == nil || production != nil {
		t.Fatalf("over changed-path bound = nonnil/%v", err)
	}
	oversized := bytes.Repeat([]byte{'x'}, maxGraphBytes+1)
	if production, err := Build(oversized, graphDigest(oversized), "impact-fixture-001", []string{"service/d/d.go"}); err == nil || production != nil {
		t.Fatalf("oversized graph = nonnil/%v", err)
	}
}

func leftPad(value, width int) string {
	digits := []byte{'0', '0', '0', '0', '0', '0'}
	for index := width - 1; index >= 0; index-- {
		digits[index] = byte('0' + value%10)
		value /= 10
	}
	return string(digits[:width])
}

func TestOutputsAreStrictlyIdentitySorted(t *testing.T) {
	graph := marshalGraph(richObservation())
	production, err := Build(graph, graphDigest(graph), "impact-fixture-001", richChangedPaths)
	if err != nil {
		t.Fatal(err)
	}
	report := production.Envelope().Report
	if !sort.SliceIsSorted(report.ReachableNodes, func(i, j int) bool {
		return report.ReachableNodes[i].NodeID < report.ReachableNodes[j].NodeID
	}) || !sort.SliceIsSorted(report.ReachableEdges, func(i, j int) bool {
		return report.ReachableEdges[i].EdgeID < report.ReachableEdges[j].EdgeID
	}) {
		t.Fatal("reachable identities are not sorted")
	}
}
