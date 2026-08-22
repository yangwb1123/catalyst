package goimpactprescan

import (
	"encoding/json"
	"strings"

	"forgeos/forge-core/internal/gopackagegraph"
)

const graphDigestDomainForTest = "forgeos.governance.local-go-package-dependency-graph-observation.v1"

func richObservation() gopackagegraph.Observation {
	value := gopackagegraph.Observation{
		APIVersion: gopackagegraph.APIVersion, Canonicalization: Canonicalization,
		Coverage: gopackagegraph.Coverage{
			GoEntriesExcludedByNestedModule: 1, GoEntriesExcludedNonregular: 1,
			GoEntriesInSelectedSubtree: 13, RegularGoFilesParsed: 10,
			RegularGoFilesSelected: 11, RegularGoFilesWithDiagnostics: 1,
		},
		Dependencies: richDependencies(), Diagnostics: []gopackagegraph.Diagnostic{{
			Code: "go_file_parse_error", Path: "service/broken/bad.go",
		}},
		Files: richFiles(), Module: richModule(), ObservedAtUnixMS: 1_786_320_000_123,
		Packages: richPackages(), Producer: gopackagegraph.Producer{
			ParametersSHA256: strings.Repeat("a", 64),
			ProducerID:       "forgeos.local-go-package-dependency-graph-observer",
			ProducerType:     "tool", ProducerVersion: "v1", RunID: "impact-fixture-001",
		},
		ProfileID: gopackagegraph.ProfileID, Source: gopackagegraph.Source{
			SourceRevision:   "git-sha1:2222222222222222222222222222222222222222",
			SourceTreeSHA256: strings.Repeat("b", 64),
		},
	}
	return value
}

func richModule() gopackagegraph.Module {
	return gopackagegraph.Module{
		Directory: "service", GoModBytes: 36,
		GoModContentSHA256: strings.Repeat("c", 64), GoModPath: "service/go.mod",
		ModulePath: "example.com/service", NestedModules: []gopackagegraph.NestedModule{{
			Directory: "service/nested", GoModPath: "service/nested/go.mod", Kind: "regular",
		}},
	}
}

func richFiles() []gopackagegraph.File {
	return []gopackagegraph.File{
		graphFile("service/a/a.go", "a", "compile", "./relative", "example.com/service/b", "example.com/service/c", "example.com/service/missing", "example.com/service/nested/x"),
		graphFile("service/b/b.go", "b", "compile", "example.com/service/d"),
		graphFile("service/c/c.go", "c", "compile", "example.com/service/d"),
		graphFile("service/d/d.go", "d", "compile"),
		graphFile("service/d/d_extra.go", "d", "compile"),
		graphFile("service/e/e.go", "e", "compile", "example.com/service/b", "example.com/service/c"),
		graphFile("service/f/f.go", "f", "compile", "example.com/service/d", "example.com/service/g"),
		graphFile("service/g/g.go", "g", "compile", "example.com/service/f"),
		graphFile("service/x/external_test.go", "x_test", "test", "example.com/service/d"),
		graphFile("service/z/z.go", "z", "compile"),
	}
}

func graphFile(path, name, role string, imports ...string) gopackagegraph.File {
	return gopackagegraph.File{
		Bytes: 10, ContentSHA256: strings.Repeat("d", 64),
		Imports: append([]string{}, imports...), PackageName: name, Path: path, Role: role,
	}
}

func richPackages() []gopackagegraph.Package {
	return []gopackagegraph.Package{
		graphPackage("service/a", "a", []string{"service/a/a.go"}, nil),
		graphPackage("service/b", "b", []string{"service/b/b.go"}, nil),
		graphPackage("service/c", "c", []string{"service/c/c.go"}, nil),
		graphPackage("service/d", "d", []string{"service/d/d.go", "service/d/d_extra.go"}, nil),
		graphPackage("service/e", "e", []string{"service/e/e.go"}, nil),
		graphPackage("service/f", "f", []string{"service/f/f.go"}, nil),
		graphPackage("service/g", "g", []string{"service/g/g.go"}, nil),
		graphPackage("service/x", "x_test", nil, []string{"service/x/external_test.go"}),
		graphPackage("service/z", "z", []string{"service/z/z.go"}, nil),
	}
}

func graphPackage(
	directory, name string,
	compileFiles, testFiles []string,
) gopackagegraph.Package {
	compileFiles = nonnilStrings(compileFiles)
	testFiles = nonnilStrings(testFiles)
	var importPath *string
	if len(compileFiles) != 0 {
		value := "example.com/service/" + strings.TrimPrefix(directory, "service/")
		importPath = &value
	}
	return gopackagegraph.Package{
		CompileFiles: compileFiles, Directory: directory,
		ImportPath: importPath, Name: name, TestFiles: testFiles,
	}
}

func nonnilStrings(values []string) []string {
	if values == nil {
		return []string{}
	}
	return append([]string{}, values...)
}

func richDependencies() []gopackagegraph.Dependency {
	return []gopackagegraph.Dependency{
		unsupportedDependency(),
		localDependency("service/a", "a", "example.com/service/b", "compile", "service/a/a.go", "service/b", "b"),
		localDependency("service/a", "a", "example.com/service/c", "compile", "service/a/a.go", "service/c", "c"),
		unresolvedDependency(), nestedDependency(),
		localDependency("service/b", "b", "example.com/service/d", "compile", "service/b/b.go", "service/d", "d"),
		localDependency("service/c", "c", "example.com/service/d", "compile", "service/c/c.go", "service/d", "d"),
		localDependency("service/e", "e", "example.com/service/b", "compile", "service/e/e.go", "service/b", "b"),
		localDependency("service/e", "e", "example.com/service/c", "compile", "service/e/e.go", "service/c", "c"),
		localDependency("service/f", "f", "example.com/service/d", "compile", "service/f/f.go", "service/d", "d"),
		localDependency("service/f", "f", "example.com/service/g", "compile", "service/f/f.go", "service/g", "g"),
		localDependency("service/g", "g", "example.com/service/f", "compile", "service/g/g.go", "service/f", "f"),
		localDependency("service/x", "x_test", "example.com/service/d", "test", "service/x/external_test.go", "service/d", "d"),
	}
}

func localDependency(
	fromDirectory, fromName, importPath, role, sourcePath, targetDirectory, targetName string,
) gopackagegraph.Dependency {
	return gopackagegraph.Dependency{
		FromDirectory: fromDirectory, FromPackageName: fromName, ImportPath: importPath,
		Relation: "depends_on", Resolution: "local", ResolutionDetail: nil,
		Role: role, SourcePaths: []string{sourcePath},
		TargetDirectory: &targetDirectory, TargetPackageName: &targetName,
	}
}

func unsupportedDependency() gopackagegraph.Dependency {
	detail := "noncanonical_import_path"
	return gopackagegraph.Dependency{
		FromDirectory: "service/a", FromPackageName: "a", ImportPath: "./relative",
		Relation: "depends_on", Resolution: "unsupported", ResolutionDetail: &detail,
		Role: "compile", SourcePaths: []string{"service/a/a.go"},
	}
}

func unresolvedDependency() gopackagegraph.Dependency {
	detail, target := "no_compile_package", "service/missing"
	return gopackagegraph.Dependency{
		FromDirectory: "service/a", FromPackageName: "a",
		ImportPath: "example.com/service/missing", Relation: "depends_on",
		Resolution: "unresolved_local", ResolutionDetail: &detail,
		Role: "compile", SourcePaths: []string{"service/a/a.go"}, TargetDirectory: &target,
	}
}

func nestedDependency() gopackagegraph.Dependency {
	detail, target := "nested_module_boundary", "service/nested/x"
	return gopackagegraph.Dependency{
		FromDirectory: "service/a", FromPackageName: "a",
		ImportPath: "example.com/service/nested/x", Relation: "depends_on",
		Resolution: "nested_module_boundary", ResolutionDetail: &detail,
		Role: "compile", SourcePaths: []string{"service/a/a.go"}, TargetDirectory: &target,
	}
}

func completeObservation() gopackagegraph.Observation {
	value := richObservation()
	value.Files[0].Imports = []string{"example.com/service/b", "example.com/service/c"}
	local := make([]gopackagegraph.Dependency, 0)
	for _, dependency := range value.Dependencies {
		if dependency.Resolution == "local" {
			local = append(local, dependency)
		}
	}
	value.Dependencies, value.Diagnostics = local, []gopackagegraph.Diagnostic{}
	value.Coverage = gopackagegraph.Coverage{
		GoEntriesInSelectedSubtree: 10, RegularGoFilesParsed: 10,
		RegularGoFilesSelected: 10, RegularGoFilesWithDiagnostics: 0,
	}
	return value
}

func marshalGraph(value gopackagegraph.Observation) []byte {
	encoded, err := json.Marshal(value)
	if err != nil {
		panic(err)
	}
	return encoded
}

func graphDigest(raw []byte) string {
	return domainDigest(graphDigestDomainForTest, raw)
}
