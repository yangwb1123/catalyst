package gopackagegraph

import (
	"fmt"
	"sort"
)

type packageKey struct {
	directory string
	name      string
}

type dependencyKey struct {
	directory  string
	importPath string
	name       string
	role       string
}

func derivePackages(files []File, module Module) ([]Package, error) {
	grouped := make(map[packageKey]*Package)
	for _, file := range files {
		key := packageKey{directory: pathDirectory(file.Path), name: file.PackageName}
		item, exists := grouped[key]
		if !exists {
			item = &Package{
				CompileFiles: []string{}, Directory: key.directory,
				Name: key.name, TestFiles: []string{},
			}
			grouped[key] = item
		}
		if file.Role == "compile" {
			item.CompileFiles = append(item.CompileFiles, file.Path)
		} else {
			item.TestFiles = append(item.TestFiles, file.Path)
		}
	}
	if len(grouped) > maxPackages {
		return nil, fmt.Errorf("package nodes exceed %d", maxPackages)
	}
	result := make([]Package, 0, len(grouped))
	for _, item := range grouped {
		sort.Strings(item.CompileFiles)
		sort.Strings(item.TestFiles)
		if len(item.CompileFiles) != 0 {
			importPath := module.ModulePath
			if item.Directory != module.Directory {
				importPath += "/" + relativeDirectory(item.Directory, module.Directory)
			}
			if !canonicalImportPath(importPath) {
				return nil, fmt.Errorf("derived package import path is outside the bounded profile")
			}
			item.ImportPath = stringPointer(importPath)
		}
		result = append(result, *item)
	}
	sort.Slice(result, func(i, j int) bool {
		if result[i].Directory != result[j].Directory {
			return result[i].Directory < result[j].Directory
		}
		return result[i].Name < result[j].Name
	})
	return result, nil
}

func deriveDependencies(files []File, module Module, packages []Package) ([]Dependency, error) {
	grouped := make(map[dependencyKey][]string)
	for _, file := range files {
		for _, importPath := range file.Imports {
			key := dependencyKey{
				directory: pathDirectory(file.Path), importPath: importPath,
				name: file.PackageName, role: file.Role,
			}
			grouped[key] = append(grouped[key], file.Path)
		}
	}
	if len(grouped) > maxEdges {
		return nil, fmt.Errorf("dependency edges exceed %d", maxEdges)
	}
	resolver := newImportResolver(module, packages)
	result := make([]Dependency, 0, len(grouped))
	for key, paths := range grouped {
		sort.Strings(paths)
		resolution, detail, targetDirectory, targetName, err := resolver.resolve(key.importPath)
		if err != nil {
			return nil, err
		}
		result = append(result, Dependency{
			FromDirectory: key.directory, FromPackageName: key.name,
			ImportPath: key.importPath, Relation: "depends_on", Resolution: resolution,
			ResolutionDetail: detail, Role: key.role, SourcePaths: compactStrings(paths),
			TargetDirectory: targetDirectory, TargetPackageName: targetName,
		})
	}
	sort.Slice(result, func(i, j int) bool {
		left, right := result[i], result[j]
		if left.FromDirectory != right.FromDirectory {
			return left.FromDirectory < right.FromDirectory
		}
		if left.FromPackageName != right.FromPackageName {
			return left.FromPackageName < right.FromPackageName
		}
		if roleRank(left.Role) != roleRank(right.Role) {
			return roleRank(left.Role) < roleRank(right.Role)
		}
		return left.ImportPath < right.ImportPath
	})
	return result, nil
}

func roleRank(value string) int {
	if value == "compile" {
		return 0
	}
	return 1
}

func compactStrings(values []string) []string {
	if len(values) == 0 {
		return []string{}
	}
	result := values[:1]
	for _, value := range values[1:] {
		if value != result[len(result)-1] {
			result = append(result, value)
		}
	}
	return result
}

func stringPointer(value string) *string { return &value }
