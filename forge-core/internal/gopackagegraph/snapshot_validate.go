package gopackagegraph

import (
	"fmt"
	"reflect"
	"sort"
	"strings"
)

// ValidateObservationSnapshot validates every fact that can be reconstructed
// from an observation alone. Source-manifest freshness remains the producer's
// responsibility and is deliberately not inferred here.
func ValidateObservationSnapshot(value Observation) error {
	if err := validateSnapshotIdentity(value); err != nil {
		return err
	}
	if err := validateSnapshotModule(value.Module); err != nil {
		return err
	}
	if err := validateSnapshotFiles(value.Files, value.Module); err != nil {
		return err
	}
	if err := validateSnapshotDiagnostics(value.Diagnostics, value.Module); err != nil {
		return err
	}
	if err := validateSnapshotCoverage(value); err != nil {
		return err
	}
	return validateSnapshotDerivations(value)
}

func validateSnapshotIdentity(value Observation) error {
	if value.APIVersion != APIVersion || value.Canonicalization != Canonicalization ||
		value.ProfileID != ProfileID || value.ObservedAtUnixMS < 0 {
		return fmt.Errorf("graph identity or observation time is invalid")
	}
	if !validProducer(value.Producer) || !validSource(value.Source) {
		return fmt.Errorf("graph producer or source binding is invalid")
	}
	return nil
}

func validateSnapshotModule(value Module) error {
	expectedGoMod := joinDirectory(value.Directory, "go.mod")
	if !safeDirectory(value.Directory) || !canonicalImportPath(value.ModulePath) ||
		value.GoModPath != expectedGoMod || value.GoModBytes < 1 ||
		value.GoModBytes > limits.GoModBytes || !validHash(value.GoModContentSHA256) {
		return fmt.Errorf("observed module is outside the bounded graph profile")
	}
	if value.NestedModules == nil || len(value.NestedModules) > maxNestedModules {
		return fmt.Errorf("observed nested modules are absent or exceed limits")
	}
	for index, item := range value.NestedModules {
		if err := validateSnapshotNestedModule(item, value.Directory); err != nil {
			return err
		}
		if index > 0 && !nestedModuleLess(value.NestedModules[index-1], item) {
			return fmt.Errorf("observed nested modules are not strictly sorted")
		}
	}
	return nil
}

func validateSnapshotNestedModule(value NestedModule, moduleDirectory string) error {
	if !safeDirectory(value.Directory) || value.Directory == moduleDirectory ||
		!pathWithin(value.Directory, moduleDirectory) ||
		value.GoModPath != joinDirectory(value.Directory, "go.mod") ||
		value.Kind != "regular" && value.Kind != "symlink" {
		return fmt.Errorf("observed nested module %q is invalid", value.Directory)
	}
	return nil
}

func nestedModuleLess(left, right NestedModule) bool {
	if left.Directory != right.Directory {
		return left.Directory < right.Directory
	}
	return left.GoModPath < right.GoModPath
}

func validateSnapshotFiles(values []File, module Module) error {
	if values == nil || len(values) > limits.GoFiles {
		return fmt.Errorf("observed files are absent or exceed limits")
	}
	previous, occurrences := "", 0
	var parserBytes int64
	for index, value := range values {
		if index > 0 && previous >= value.Path {
			return fmt.Errorf("observed files must be strictly path sorted")
		}
		if err := validateSnapshotFile(value, module); err != nil {
			return err
		}
		if len(value.Imports) > maxImportOccurrences-occurrences {
			return fmt.Errorf("observed import occurrences exceed %d", maxImportOccurrences)
		}
		if value.Bytes > limits.AggregateParserBytes-parserBytes {
			return fmt.Errorf("aggregate Go parser input exceeds %d bytes", limits.AggregateParserBytes)
		}
		occurrences += len(value.Imports)
		parserBytes += value.Bytes
		previous = value.Path
	}
	return nil
}

func validateSnapshotFile(value File, module Module) error {
	if !safeRepoPath(value.Path) || !strings.HasSuffix(value.Path, ".go") ||
		!pathWithin(value.Path, module.Directory) || withinSnapshotNested(value.Path, module) ||
		value.Bytes < 0 || value.Bytes > limits.GoFileBytes ||
		!validHash(value.ContentSHA256) || !validPackageIdentifier(value.PackageName) ||
		!validText(value.PackageName) || value.Role != roleForPath(value.Path) ||
		!sortedUniqueText(value.Imports, maxImportsPerFile) {
		return fmt.Errorf("observed file %q is invalid", value.Path)
	}
	return nil
}

func validateSnapshotDiagnostics(values []Diagnostic, module Module) error {
	if values == nil || len(values) > maxDiagnostics {
		return fmt.Errorf("observed diagnostics are absent or exceed limits")
	}
	previous := ""
	for index, value := range values {
		_, validCode := diagnosticCodes[value.Code]
		if !validCode || !safeRepoPath(value.Path) || !strings.HasSuffix(value.Path, ".go") ||
			!pathWithin(value.Path, module.Directory) || withinSnapshotNested(value.Path, module) ||
			index > 0 && previous >= value.Path {
			return fmt.Errorf("observed diagnostic %q is invalid or unsorted", value.Path)
		}
		previous = value.Path
	}
	return nil
}

func withinSnapshotNested(value string, module Module) bool {
	for _, boundary := range module.NestedModules {
		if pathWithin(value, boundary.Directory) {
			return true
		}
	}
	return false
}

func validateSnapshotCoverage(value Observation) error {
	coverage := value.Coverage
	counts := []int64{
		coverage.GoEntriesExcludedByNestedModule, coverage.GoEntriesExcludedNonregular,
		coverage.GoEntriesInSelectedSubtree, coverage.RegularGoFilesParsed,
		coverage.RegularGoFilesSelected, coverage.RegularGoFilesWithDiagnostics,
	}
	for _, count := range counts {
		if count < 0 || count > maxSourceEntries {
			return fmt.Errorf("observed coverage count is negative or exceeds source limits")
		}
	}
	selected := int64(len(value.Files) + len(value.Diagnostics))
	inside := selected + coverage.GoEntriesExcludedByNestedModule +
		coverage.GoEntriesExcludedNonregular
	if coverage.RegularGoFilesParsed != int64(len(value.Files)) ||
		coverage.RegularGoFilesWithDiagnostics != int64(len(value.Diagnostics)) ||
		coverage.RegularGoFilesSelected != selected || coverage.GoEntriesInSelectedSubtree != inside {
		return fmt.Errorf("observed coverage is internally inconsistent")
	}
	if selected > int64(limits.GoFiles) {
		return fmt.Errorf("observed coverage exceeds source or selected-file limits")
	}
	return validateSnapshotPartition(value.Files, value.Diagnostics)
}

func validateSnapshotPartition(files []File, diagnostics []Diagnostic) error {
	paths := make(map[string]struct{}, len(files)+len(diagnostics))
	for _, file := range files {
		paths[file.Path] = struct{}{}
	}
	for _, diagnostic := range diagnostics {
		if _, exists := paths[diagnostic.Path]; exists {
			return fmt.Errorf("observed path %q has multiple outcomes", diagnostic.Path)
		}
		paths[diagnostic.Path] = struct{}{}
	}
	return nil
}

func validateSnapshotDerivations(value Observation) error {
	packages, err := derivePackages(value.Files, value.Module)
	if err != nil || !reflect.DeepEqual(value.Packages, packages) {
		return fmt.Errorf("package nodes do not exactly derive from observed files")
	}
	dependencies, err := deriveDependencies(value.Files, value.Module, packages)
	if err != nil || !reflect.DeepEqual(value.Dependencies, dependencies) {
		return fmt.Errorf("dependency edges do not exactly derive from observed files")
	}
	if !sort.SliceIsSorted(value.Packages, func(i, j int) bool {
		return value.Packages[i].Directory < value.Packages[j].Directory ||
			value.Packages[i].Directory == value.Packages[j].Directory &&
				value.Packages[i].Name < value.Packages[j].Name
	}) {
		return fmt.Errorf("package nodes are not sorted")
	}
	return nil
}
