package projectsnapshot

import (
	"fmt"
	"reflect"
)

func validateCoverage(value Coverage, manifest SourceManifest) error {
	wantCounts := deriveCounts(manifest)
	if value.APIVersion != coverageVersion || value.Canonicalization != canonicalization ||
		value.SourceManifestSHA256 != manifest.SourceManifestSHA256 || value.Counts != wantCounts {
		return fmt.Errorf("project source coverage bindings or counts drifted")
	}
	expected, err := buildCoverage(manifest.SourceManifestSHA256, wantCounts)
	if err != nil || !reflect.DeepEqual(value, expected) {
		return fmt.Errorf("project source coverage surfaces or digest drifted")
	}
	return nil
}

func deriveCounts(manifest SourceManifest) CoverageCounts {
	value := CoverageCounts{
		IgnoredPathCount: manifest.IgnoredPathCount, UniverseCount: manifest.UniverseCount,
	}
	for _, entry := range manifest.Entries {
		if entry.Tracking == "tracked" {
			value.TrackedCount++
		} else {
			value.UntrackedCount++
		}
		if entry.Kind == "regular" {
			value.IncludedRegularCount++
		} else {
			value.TrackedAbsentCount++
		}
	}
	for _, exclusion := range manifest.Excluded {
		if exclusion.Tracking == "tracked" {
			value.TrackedCount++
		} else {
			value.UntrackedCount++
		}
		switch exclusion.Reason {
		case "control_path":
			value.ExcludedControlCount++
		case "sensitive_path":
			value.ExcludedSensitiveCount++
		case "symlink_leaf":
			value.ExcludedSymlinkCount++
		}
	}
	return value
}
