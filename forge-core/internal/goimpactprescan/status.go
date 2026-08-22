package goimpactprescan

import (
	"sort"

	"forgeos/forge-core/internal/gopackagegraph"
)

func closureStatus(
	observation gopackagegraph.Observation,
	unresolved []UnresolvedSeed,
) (string, []string) {
	reasons := make(map[string]struct{})
	if len(unresolved) != 0 {
		reasons["changed_path_unresolved"] = struct{}{}
	}
	if len(observation.Diagnostics) != 0 {
		reasons["go_file_diagnostic_present"] = struct{}{}
	}
	for _, dependency := range observation.Dependencies {
		if reason := dependencyClosureReason(dependency.Resolution); reason != "" {
			reasons[reason] = struct{}{}
		}
	}
	result := sortedSetKeys(reasons)
	if len(result) == 0 {
		return Complete, []string{}
	}
	sort.Strings(result)
	return Unknown, result
}

func dependencyClosureReason(resolution string) string {
	switch resolution {
	case "ambiguous_local":
		return "ambiguous_local_dependency_present"
	case "nested_module_boundary":
		return "nested_module_boundary_dependency_present"
	case "unresolved_local":
		return "unresolved_local_dependency_present"
	case "unsupported":
		return "unsupported_import_dependency_present"
	default:
		return ""
	}
}
