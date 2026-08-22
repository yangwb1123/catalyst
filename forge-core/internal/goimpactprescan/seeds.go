package goimpactprescan

import (
	"path"
	"sort"
	"strings"

	"forgeos/forge-core/internal/gopackagegraph"
)

func resolveSeeds(
	changedPaths []string,
	observation gopackagegraph.Observation,
	index *graphIndex,
) ([]ResolvedSeed, []UnresolvedSeed, map[string]struct{}, error) {
	diagnostics := make(map[string]string, len(observation.Diagnostics))
	for _, item := range observation.Diagnostics {
		diagnostics[item.Path] = item.Code
	}
	grouped := make(map[string][]string)
	unresolved := make([]UnresolvedSeed, 0)
	for _, changedPath := range changedPaths {
		node, outcome := resolveSeed(changedPath, observation, index, diagnostics)
		if outcome != nil {
			unresolved = append(unresolved, *outcome)
			continue
		}
		grouped[node.NodeID] = append(grouped[node.NodeID], changedPath)
	}
	resolved, seedIDs := groupedSeeds(grouped)
	return resolved, unresolved, seedIDs, nil
}

func resolveSeed(
	changedPath string,
	observation gopackagegraph.Observation,
	index *graphIndex,
	diagnostics map[string]string,
) (ReachableNode, *UnresolvedSeed) {
	if !pathWithin(changedPath, observation.Module.Directory) {
		return unresolved(changedPath, "outside_selected_module", nil)
	}
	if withinNestedBoundary(changedPath, observation.Module.NestedModules) {
		return unresolved(changedPath, "inside_nested_module_boundary", nil)
	}
	if !strings.HasSuffix(changedPath, ".go") {
		return unresolved(changedPath, "not_a_go_file", nil)
	}
	if code, exists := diagnostics[changedPath]; exists {
		return unresolved(changedPath, "go_file_diagnostic", &code)
	}
	file, exists := index.files[changedPath]
	if !exists {
		return unresolved(changedPath, "not_in_observed_file_or_diagnostic", nil)
	}
	node, exists := index.nodesByKey[packageKey{directoryForPath(file.Path), file.PackageName}]
	if !exists {
		return unresolved(changedPath, "not_in_observed_file_or_diagnostic", nil)
	}
	return node, nil
}

func unresolved(path, reason string, code *string) (ReachableNode, *UnresolvedSeed) {
	return ReachableNode{}, &UnresolvedSeed{
		ChangedPath: path, DiagnosticCode: cloneString(code), Reason: reason,
	}
}

func groupedSeeds(grouped map[string][]string) ([]ResolvedSeed, map[string]struct{}) {
	result := make([]ResolvedSeed, 0, len(grouped))
	identities := make(map[string]struct{}, len(grouped))
	for nodeID, paths := range grouped {
		result = append(result, ResolvedSeed{
			ChangedPaths: append([]string{}, paths...), NodeID: nodeID,
		})
		identities[nodeID] = struct{}{}
	}
	sort.Slice(result, func(i, j int) bool { return result[i].NodeID < result[j].NodeID })
	return result, identities
}

func withinNestedBoundary(value string, boundaries []gopackagegraph.NestedModule) bool {
	for _, boundary := range boundaries {
		if pathWithin(value, boundary.Directory) {
			return true
		}
	}
	return false
}

func directoryForPath(value string) string {
	directory := path.Dir(value)
	if directory == "" {
		return "."
	}
	return directory
}
