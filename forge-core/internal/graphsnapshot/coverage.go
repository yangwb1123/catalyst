package graphsnapshot

import "sort"

var goCoverageBaseReasons = []string{
	"all_regular_go_files_lexical_union_not_selected_build",
	"compile_test_runtime_reachability_not_observed",
	"go_module_graph_not_resolved",
	"single_selected_go_module_only",
	"source_observation_not_atomic_snapshot",
}

var testSourceGoCoverageBaseReasons = []string{
	"all_regular_go_files_lexical_union_not_selected_build",
	"compile_runtime_reachability_not_observed",
	"go_module_graph_not_resolved",
	"single_selected_go_module_only",
	"source_observation_not_atomic_snapshot",
}

var testSourceCoverageBaseReasons = []string{
	"go_test_files_lexical_source_set_only",
	"selected_test_build_not_observed",
	"source_observation_not_atomic_snapshot",
	"test_case_identity_not_observed",
	"test_execution_not_observed",
	"test_outcome_and_coverage_not_observed",
}

var resolutionCoverageReasons = map[string]string{
	"ambiguous_local":        "ambiguous_local_dependency_present",
	"cgo_pseudo":             "cgo_pseudo_dependency_present",
	"external_candidate":     "external_candidate_dependency_present",
	"nested_module_boundary": "nested_module_boundary_dependency_present",
	"stdlib_candidate":       "stdlib_candidate_dependency_present",
	"unresolved_local":       "unresolved_local_dependency_present",
	"unsupported":            "unsupported_import_dependency_present",
}

func (value *projector) buildCoverage(nodes []Node, edges []Edge) Coverage {
	if value.profile.includeTestSources {
		return value.buildTestSourceCoverage(nodes, edges)
	}
	reasons := append([]string{}, goCoverageBaseReasons...)
	if len(value.observation.Diagnostics) != 0 {
		reasons = append(reasons, "go_file_diagnostic_present")
	}
	if len(value.observation.Module.NestedModules) != 0 {
		reasons = append(reasons, "nested_module_boundary_present")
	}
	if value.observation.Coverage.GoEntriesExcludedNonregular != 0 {
		reasons = append(reasons, "nonregular_go_entries_not_located")
	}
	seen := map[string]struct{}{}
	for _, dependency := range value.observation.Dependencies {
		if reason, exists := resolutionCoverageReasons[dependency.Resolution]; exists {
			seen[reason] = struct{}{}
		}
	}
	for reason := range seen {
		reasons = append(reasons, reason)
	}
	sort.Strings(reasons)
	surfaces := make([]CoverageSurface, 0, len(surfaceNames))
	for _, name := range surfaceNames {
		if name == "go_module_package_lexical" {
			surfaces = append(surfaces, CoverageSurface{
				EdgeCount: int64(len(edges)), NodeCount: int64(len(nodes)),
				ReasonCodes: reasons, Status: "partial", Surface: name,
			})
			continue
		}
		surfaces = append(surfaces, CoverageSurface{
			ReasonCodes: []string{name + "_surface_not_observed"},
			Status:      "not_observed", Surface: name,
		})
	}
	return Coverage{Status: "partial", SurfaceCount: int64(len(surfaces)), Surfaces: surfaces}
}

func (value *projector) buildTestSourceCoverage(nodes []Node, edges []Edge) Coverage {
	goReasons := append([]string{}, testSourceGoCoverageBaseReasons...)
	testReasons := append([]string{}, testSourceCoverageBaseReasons...)
	goConditional, testConditional := map[string]struct{}{}, map[string]struct{}{}
	for _, diagnostic := range value.observation.Diagnostics {
		addPartitionedReason(goConditional, testConditional,
			diagnosticRole(diagnostic.Path), "go_file_diagnostic_present")
	}
	for _, dependency := range value.observation.Dependencies {
		if reason, exists := resolutionCoverageReasons[dependency.Resolution]; exists {
			addPartitionedReason(goConditional, testConditional, dependency.Role, reason)
		}
	}
	if len(value.observation.Module.NestedModules) != 0 {
		goConditional["nested_module_boundary_present"] = struct{}{}
		testConditional["nested_module_boundary_present"] = struct{}{}
	}
	if value.observation.Coverage.GoEntriesExcludedNonregular != 0 {
		goConditional["nonregular_go_entries_not_located"] = struct{}{}
		testConditional["nonregular_go_entries_not_located"] = struct{}{}
	}
	goReasons, testReasons = appendReasons(goReasons, goConditional), appendReasons(testReasons, testConditional)
	goNodes, testNodes, goEdges, testEdges := partitionCoverageRecords(nodes, edges)
	surfaces := buildTestSourceSurfaces(goNodes, testNodes, goEdges, testEdges, goReasons, testReasons)
	return Coverage{Status: "partial", SurfaceCount: int64(len(surfaces)), Surfaces: surfaces}
}

func addPartitionedReason(goReasons, testReasons map[string]struct{}, role, reason string) {
	if role == "test" {
		testReasons[reason] = struct{}{}
		return
	}
	goReasons[reason] = struct{}{}
}

func appendReasons(base []string, conditional map[string]struct{}) []string {
	result := append([]string{}, base...)
	for reason := range conditional {
		result = append(result, reason)
	}
	sort.Strings(result)
	return result
}

func partitionCoverageRecords(nodes []Node, edges []Edge) (int64, int64, int64, int64) {
	var goNodes, testNodes, goEdges, testEdges int64
	testNodeIDs := make(map[string]struct{})
	for _, node := range nodes {
		if node.NodeType == "test" {
			testNodes++
			testNodeIDs[node.NodeID] = struct{}{}
		} else {
			goNodes++
		}
	}
	for _, edge := range edges {
		_, targetsTest := testNodeIDs[edge.ToNodeID]
		if edge.SourceRole != nil && *edge.SourceRole == "test" ||
			edge.SourceRole == nil && edge.Relation == "contains" && targetsTest {
			testEdges++
		} else {
			goEdges++
		}
	}
	return goNodes, testNodes, goEdges, testEdges
}

func buildTestSourceSurfaces(
	goNodes, testNodes, goEdges, testEdges int64,
	goReasons, testReasons []string,
) []CoverageSurface {
	result := make([]CoverageSurface, 0, len(surfaceNames))
	for _, name := range surfaceNames {
		switch name {
		case "go_module_package_lexical":
			result = append(result, CoverageSurface{EdgeCount: goEdges, NodeCount: goNodes,
				ReasonCodes: goReasons, Status: "partial", Surface: name})
		case "test_verification":
			result = append(result, CoverageSurface{EdgeCount: testEdges, NodeCount: testNodes,
				ReasonCodes: testReasons, Status: "partial", Surface: name})
		default:
			result = append(result, CoverageSurface{ReasonCodes: []string{name + "_surface_not_observed"},
				Status: "not_observed", Surface: name})
		}
	}
	return result
}
