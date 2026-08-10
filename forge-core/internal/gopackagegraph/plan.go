package gopackagegraph

import (
	"fmt"
	"path"
	"sort"
	"strings"
)

const maxSourceEntries = 65_536

type selectedGoFile struct {
	bytes  int64
	path   string
	sha256 string
}

type Plan struct {
	coverage        Coverage
	goFilePaths     []string
	goMod           selectedGoFile
	moduleDirectory string
	nestedModules   []NestedModule
	selected        []selectedGoFile
}

func Prepare(moduleDirectory string, entries []SourceEntry) (*Plan, error) {
	if err := ValidateModuleDirectory(moduleDirectory); err != nil {
		return nil, err
	}
	if entries == nil || len(entries) > maxSourceEntries {
		return nil, fmt.Errorf("source entries are absent or exceed %d", maxSourceEntries)
	}
	if err := validateSourceEntries(entries); err != nil {
		return nil, err
	}
	goModPath := joinDirectory(moduleDirectory, "go.mod")
	goMod, err := selectGoMod(entries, goModPath)
	if err != nil {
		return nil, err
	}
	nested, err := selectNestedModules(entries, moduleDirectory, goModPath)
	if err != nil {
		return nil, err
	}
	return selectGoFiles(entries, moduleDirectory, goMod, nested)
}

func (plan *Plan) GoModPath() string {
	if plan == nil {
		return ""
	}
	return plan.goMod.path
}

func (plan *Plan) GoFilePaths() []string {
	if plan == nil {
		return nil
	}
	return append([]string{}, plan.goFilePaths...)
}

func validateSourceEntries(entries []SourceEntry) error {
	previous := ""
	for index, entry := range entries {
		if !safeRepoPath(entry.Path) || index > 0 && previous >= entry.Path {
			return fmt.Errorf("source entries must have strictly sorted canonical paths")
		}
		if entry.Bytes < 0 || !validSourceEntryKind(entry.Kind) {
			return fmt.Errorf("source path %q has invalid kind or byte count", entry.Path)
		}
		if entry.Kind == "regular" && (entry.ContentSHA256 == nil || !validHash(*entry.ContentSHA256)) {
			return fmt.Errorf("regular source path %q lacks a canonical digest", entry.Path)
		}
		previous = entry.Path
	}
	return nil
}

func validSourceEntryKind(value string) bool {
	return value == "regular" || value == "symlink" || value == "deleted"
}

func selectGoMod(entries []SourceEntry, goModPath string) (selectedGoFile, error) {
	index := sort.Search(len(entries), func(index int) bool { return entries[index].Path >= goModPath })
	if index == len(entries) || entries[index].Path != goModPath || entries[index].Kind != "regular" ||
		entries[index].ContentSHA256 == nil {
		return selectedGoFile{}, fmt.Errorf("selected go.mod %q must be a current regular source entry", goModPath)
	}
	entry := entries[index]
	if entry.Bytes < 1 || entry.Bytes > limits.GoModBytes {
		return selectedGoFile{}, fmt.Errorf("selected go.mod %q exceeds bounded size", goModPath)
	}
	return selectedGoFile{bytes: entry.Bytes, path: entry.Path, sha256: *entry.ContentSHA256}, nil
}

func selectNestedModules(
	entries []SourceEntry,
	moduleDirectory, ownGoModPath string,
) ([]NestedModule, error) {
	result := make([]NestedModule, 0)
	for _, entry := range entries {
		if entry.Path == ownGoModPath || path.Base(entry.Path) != "go.mod" ||
			!pathWithin(entry.Path, moduleDirectory) ||
			entry.Kind != "regular" && entry.Kind != "symlink" {
			continue
		}
		result = append(result, NestedModule{
			Directory: pathDirectory(entry.Path), GoModPath: entry.Path, Kind: entry.Kind,
		})
	}
	if len(result) > maxNestedModules {
		return nil, fmt.Errorf("nested modules exceed %d", maxNestedModules)
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].Directory != result[j].Directory {
			return result[i].Directory < result[j].Directory
		}
		return result[i].GoModPath < result[j].GoModPath
	})
	return result, nil
}

func selectGoFiles(
	entries []SourceEntry,
	moduleDirectory string,
	goMod selectedGoFile,
	nested []NestedModule,
) (*Plan, error) {
	nestedSet := make(map[string]struct{}, len(nested))
	for _, item := range nested {
		nestedSet[item.Directory] = struct{}{}
	}
	selected, readPaths := make([]selectedGoFile, 0), make([]string, 0)
	coverage := Coverage{}
	var parserBytes int64
	for _, entry := range entries {
		if !pathWithin(entry.Path, moduleDirectory) || !strings.HasSuffix(entry.Path, ".go") {
			continue
		}
		coverage.GoEntriesInSelectedSubtree++
		if withinNestedModule(entry.Path, nestedSet) {
			coverage.GoEntriesExcludedByNestedModule++
			continue
		}
		if entry.Kind != "regular" || entry.ContentSHA256 == nil {
			coverage.GoEntriesExcludedNonregular++
			continue
		}
		item := selectedGoFile{bytes: entry.Bytes, path: entry.Path, sha256: *entry.ContentSHA256}
		selected = append(selected, item)
		if entry.Bytes <= limits.GoFileBytes {
			if entry.Bytes > limits.AggregateParserBytes-parserBytes {
				return nil, fmt.Errorf("aggregate Go parser input exceeds %d bytes", limits.AggregateParserBytes)
			}
			parserBytes += entry.Bytes
			readPaths = append(readPaths, entry.Path)
		}
	}
	if len(selected) > limits.GoFiles {
		return nil, fmt.Errorf("selected regular Go files exceed %d", limits.GoFiles)
	}
	coverage.RegularGoFilesSelected = int64(len(selected))
	return &Plan{
		coverage: coverage, goFilePaths: readPaths, goMod: goMod,
		moduleDirectory: moduleDirectory, nestedModules: cloneNestedModules(nested),
		selected: selected,
	}, nil
}

func withinNestedModule(filePath string, nested map[string]struct{}) bool {
	for directory := pathDirectory(filePath); directory != "."; directory = pathDirectory(directory) {
		if _, exists := nested[directory]; exists {
			return true
		}
	}
	return false
}

func validHash(value string) bool {
	if len(value) != 64 {
		return false
	}
	for _, character := range value {
		if character < '0' || character > '9' {
			if character < 'a' || character > 'f' {
				return false
			}
		}
	}
	return true
}
