package graphsnapshot

import (
	"fmt"
	"sort"
	"strings"

	"forgeos/forge-core/internal/gopackagegraph"
)

func (value *projector) packageLocators(item gopackagegraph.Package) ([]SourceLocator, error) {
	result := make([]SourceLocator, 0, len(item.CompileFiles)+len(item.TestFiles))
	for _, path := range item.CompileFiles {
		locator, err := value.fileLocator(path, "compile")
		if err != nil {
			return nil, err
		}
		result = append(result, locator)
	}
	for _, path := range item.TestFiles {
		locator, err := value.fileLocator(path, "test")
		if err != nil {
			return nil, err
		}
		result = append(result, locator)
	}
	return normalizeLocators(result)
}

func (value *projector) testSourceLocators(item gopackagegraph.Package) ([]SourceLocator, error) {
	result := make([]SourceLocator, 0, len(item.TestFiles))
	for _, path := range item.TestFiles {
		locator, err := value.fileLocator(path, "test")
		if err != nil {
			return nil, err
		}
		result = append(result, locator)
	}
	return normalizeLocators(result)
}

func (value *projector) dependencyLocators(item gopackagegraph.Dependency) ([]SourceLocator, error) {
	result := make([]SourceLocator, 0, len(item.SourcePaths))
	for _, path := range item.SourcePaths {
		locator, err := value.fileLocator(path, item.Role)
		if err != nil {
			return nil, err
		}
		result = append(result, locator)
	}
	return normalizeLocators(result)
}

func (value *projector) fileLocator(path, role string) (SourceLocator, error) {
	file, exists := value.files[path]
	if !exists || file.Role != role {
		return SourceLocator{}, fmt.Errorf("source locator does not resolve to the exact graph file")
	}
	digest := file.ContentSHA256
	return SourceLocator{
		ContentSHA256: &digest, Path: file.Path, Role: role, SourceID: value.source.SourceID,
	}, nil
}

func normalizeLocators(values []SourceLocator) ([]SourceLocator, error) {
	if len(values) == 0 || len(values) > maxLocators {
		return nil, fmt.Errorf("source locator count is outside bounds")
	}
	result := cloneLocators(values)
	sort.Slice(result, func(i, j int) bool { return locatorLess(result[i], result[j]) })
	for index, item := range result {
		if !validBoundedText(item.Path) || index > 0 && !locatorLess(result[index-1], item) {
			return nil, fmt.Errorf("source locators are invalid or duplicate")
		}
	}
	return result, nil
}

func locatorLess(left, right SourceLocator) bool {
	if left.Role != right.Role {
		return left.Role < right.Role
	}
	if left.Path != right.Path {
		return left.Path < right.Path
	}
	if left.ContentSHA256 == nil || right.ContentSHA256 == nil {
		return left.ContentSHA256 == nil && right.ContentSHA256 != nil
	}
	return *left.ContentSHA256 < *right.ContentSHA256
}

func cloneLocators(values []SourceLocator) []SourceLocator {
	result := make([]SourceLocator, len(values))
	for index, value := range values {
		result[index] = value
		result[index].ContentSHA256 = stringCopy(value.ContentSHA256)
	}
	return result
}

func diagnosticRole(path string) string {
	if strings.HasSuffix(path, "_test.go") {
		return "test"
	}
	return "compile"
}
