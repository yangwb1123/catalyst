package domain

import "sort"

func validateSnapshotBindings(
	values []SnapshotBinding, minimum, maximum int,
) (map[string]string, error) {
	if len(values) < minimum || len(values) > maximum {
		return nil, invalidDomain("snapshot binding count is outside its bound", nil)
	}
	projects := make(map[string]string, len(values))
	snapshots := make(map[string]struct{}, len(values))
	for _, value := range values {
		if err := validateEntityID(value.ProjectID, "project", "snapshot project_id"); err != nil {
			return nil, err
		}
		if err := validateEntityID(
			value.ProjectSnapshotID, "project_snapshot", "project_snapshot_id",
		); err != nil {
			return nil, err
		}
		if _, exists := projects[value.ProjectID]; exists {
			return nil, invalidDomain("snapshot bindings contain a duplicate Project", nil)
		}
		if _, exists := snapshots[value.ProjectSnapshotID]; exists {
			return nil, invalidDomain("snapshot bindings reuse one ProjectSnapshot", nil)
		}
		projects[value.ProjectID] = value.ProjectSnapshotID
		snapshots[value.ProjectSnapshotID] = struct{}{}
	}
	return projects, nil
}

func sameProjectIDs(values []string, bindings map[string]string) bool {
	if len(values) != len(bindings) {
		return false
	}
	for _, value := range values {
		if _, exists := bindings[value]; !exists {
			return false
		}
	}
	return true
}

func sameSnapshotBindings(left, right map[string]string) bool {
	if len(left) != len(right) {
		return false
	}
	for projectID, snapshotID := range left {
		if right[projectID] != snapshotID {
			return false
		}
	}
	return true
}

// CompareSnapshotBindings compares supplied identifiers without observing a project.
func CompareSnapshotBindings(
	bound, current []SnapshotBinding,
) (SnapshotDrift, error) {
	boundMap, err := validateSnapshotBindings(bound, 1, maxTargetProjects)
	if err != nil {
		return SnapshotDrift{}, err
	}
	currentMap, err := validateSnapshotBindings(current, 0, maxComparedSnapshots)
	if err != nil {
		return SnapshotDrift{}, err
	}
	result := SnapshotDrift{}
	for projectID, snapshotID := range boundMap {
		currentSnapshot, exists := currentMap[projectID]
		if !exists {
			result.MissingProjectIDs = append(result.MissingProjectIDs, projectID)
		} else if currentSnapshot != snapshotID {
			result.ChangedProjectIDs = append(result.ChangedProjectIDs, projectID)
		}
	}
	for projectID := range currentMap {
		if _, exists := boundMap[projectID]; !exists {
			result.UnexpectedProjectIDs = append(result.UnexpectedProjectIDs, projectID)
		}
	}
	sort.Strings(result.MissingProjectIDs)
	sort.Strings(result.ChangedProjectIDs)
	sort.Strings(result.UnexpectedProjectIDs)
	return result, nil
}
