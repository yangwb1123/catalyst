package gopackagegraph

import (
	"fmt"
	"reflect"
	"sort"
	"strings"
)

var diagnosticCodes = map[string]struct{}{
	"go_file_exceeds_parser_limit":  {},
	"go_file_import_limit_exceeded": {},
	"go_file_invalid_utf8":          {},
	"go_file_parse_error":           {},
	"go_file_unsupported_text":      {},
}

func ValidateObservation(
	value Observation,
	moduleDirectory string,
	entries []SourceEntry,
) error {
	plan, err := Prepare(moduleDirectory, entries)
	if err != nil {
		return err
	}
	if value.APIVersion != APIVersion || value.Canonicalization != Canonicalization ||
		value.ProfileID != ProfileID || value.ObservedAtUnixMS < 0 {
		return fmt.Errorf("graph identity or observation time is invalid")
	}
	if !validProducer(value.Producer) || !validSource(value.Source) {
		return fmt.Errorf("graph producer or source binding is invalid")
	}
	if err := validateObservedModule(value.Module, plan); err != nil {
		return err
	}
	if err := validateObservedFiles(value.Files, plan); err != nil {
		return err
	}
	if err := validateObservedDiagnostics(value.Diagnostics, plan); err != nil {
		return err
	}
	if err := validateObservedPartition(value, plan); err != nil {
		return err
	}
	expectedPackages, err := derivePackages(value.Files, value.Module)
	if err != nil || !reflect.DeepEqual(value.Packages, expectedPackages) {
		return fmt.Errorf("package nodes do not exactly derive from observed files")
	}
	expectedDependencies, err := deriveDependencies(value.Files, value.Module, expectedPackages)
	if err != nil || !reflect.DeepEqual(value.Dependencies, expectedDependencies) {
		return fmt.Errorf("dependency edges do not exactly derive from observed files")
	}
	return nil
}

func validateObservedModule(value Module, plan *Plan) error {
	expected := Module{
		Directory: plan.moduleDirectory, GoModBytes: plan.goMod.bytes,
		GoModContentSHA256: plan.goMod.sha256, GoModPath: plan.goMod.path,
		ModulePath: value.ModulePath, NestedModules: cloneNestedModules(plan.nestedModules),
	}
	if !canonicalImportPath(value.ModulePath) || !reflect.DeepEqual(value, expected) {
		return fmt.Errorf("observed module does not match selected source module")
	}
	return nil
}

func validateObservedFiles(values []File, plan *Plan) error {
	if values == nil || len(values) > limits.GoFiles {
		return fmt.Errorf("observed files are absent or exceed limits")
	}
	selected := selectedFileIndex(plan.selected)
	previous, occurrences := "", 0
	for index, value := range values {
		if index > 0 && previous >= value.Path {
			return fmt.Errorf("observed files must be strictly path sorted")
		}
		expected, exists := selected[value.Path]
		if !exists || expected.bytes > limits.GoFileBytes || value.Bytes != expected.bytes ||
			value.ContentSHA256 != expected.sha256 {
			return fmt.Errorf("observed file %q does not match selected source", value.Path)
		}
		if !validPackageIdentifier(value.PackageName) || !validText(value.PackageName) ||
			value.Role != roleForPath(value.Path) {
			return fmt.Errorf("observed file %q has invalid package or role", value.Path)
		}
		if !sortedUniqueText(value.Imports, maxImportsPerFile) {
			return fmt.Errorf("observed file %q has invalid imports", value.Path)
		}
		if len(value.Imports) > maxImportOccurrences-occurrences {
			return fmt.Errorf("observed import occurrences exceed %d", maxImportOccurrences)
		}
		occurrences += len(value.Imports)
		previous = value.Path
	}
	return nil
}

func validateObservedDiagnostics(values []Diagnostic, plan *Plan) error {
	if values == nil || len(values) > maxDiagnostics {
		return fmt.Errorf("observed diagnostics are absent or exceed limits")
	}
	selected := selectedFileIndex(plan.selected)
	previous := ""
	for index, value := range values {
		expected, exists := selected[value.Path]
		_, validCode := diagnosticCodes[value.Code]
		if !exists || !validCode || index > 0 && previous >= value.Path {
			return fmt.Errorf("diagnostic %q is invalid, duplicate, or unsorted", value.Path)
		}
		oversized := expected.bytes > limits.GoFileBytes
		if oversized != (value.Code == "go_file_exceeds_parser_limit") {
			return fmt.Errorf("diagnostic %q does not match parser size classification", value.Path)
		}
		previous = value.Path
	}
	return nil
}

func validateObservedPartition(value Observation, plan *Plan) error {
	paths := make(map[string]struct{}, len(value.Files)+len(value.Diagnostics))
	for _, file := range value.Files {
		paths[file.Path] = struct{}{}
	}
	for _, diagnostic := range value.Diagnostics {
		if _, exists := paths[diagnostic.Path]; exists {
			return fmt.Errorf("selected source path %q has multiple outcomes", diagnostic.Path)
		}
		paths[diagnostic.Path] = struct{}{}
	}
	if len(paths) != len(plan.selected) {
		return fmt.Errorf("observed files and diagnostics do not partition selected source")
	}
	expected := plan.coverage
	expected.RegularGoFilesParsed = int64(len(value.Files))
	expected.RegularGoFilesWithDiagnostics = int64(len(value.Diagnostics))
	if value.Coverage != expected {
		return fmt.Errorf("observed coverage does not exactly derive from source selection")
	}
	return nil
}

func selectedFileIndex(values []selectedGoFile) map[string]selectedGoFile {
	result := make(map[string]selectedGoFile, len(values))
	for _, value := range values {
		result[value.path] = value
	}
	return result
}

func sortedUniqueText(values []string, limit int) bool {
	if values == nil || len(values) > limit || !sort.StringsAreSorted(values) {
		return false
	}
	for index, value := range values {
		if !validText(value) || index > 0 && values[index-1] == value {
			return false
		}
	}
	return true
}

func roleForPath(value string) string {
	if strings.HasSuffix(value, "_test.go") {
		return "test"
	}
	return "compile"
}
