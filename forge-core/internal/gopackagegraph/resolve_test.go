package gopackagegraph

import (
	"fmt"
	"strings"
	"testing"
)

func TestDerivedPackageAndTargetPathsRemainBounded(t *testing.T) {
	module := Module{Directory: ".", ModulePath: strings.Repeat("m", maxTextScalars)}
	files := []File{{PackageName: "p", Path: "p/p.go", Role: "compile"}}
	if _, err := derivePackages(files, module); err == nil {
		t.Fatal("oversized derived package import path accepted")
	}
	module = Module{Directory: strings.Repeat("d", maxTextScalars-6), ModulePath: "m"}
	files = []File{{
		Imports: []string{"m/descendant"}, PackageName: "p",
		Path: "source.go", Role: "compile",
	}}
	if _, err := deriveDependencies(files, module, nil); err == nil {
		t.Fatal("oversized derived target directory accepted")
	}

	resolver := newImportResolver(Module{Directory: ".", ModulePath: "m"}, nil)
	resolution, detail, target, name, err := resolver.resolve("m/C:/x")
	if err != nil || resolution != "unsupported" || detail == nil ||
		*detail != "noncanonical_import_path" || target != nil || name != nil {
		t.Fatalf("drive-prefixed import = %q, %v, %v, %v, %v", resolution, detail, target, name, err)
	}
}

func TestImportResolverIndexesHighCardinalityPackages(t *testing.T) {
	packages := make([]Package, maxPackages)
	for index := range packages {
		packages[index] = Package{
			CompileFiles: []string{"x.go"}, Directory: fmt.Sprintf("p%05d", index),
			Name: "p",
		}
	}
	resolver := newImportResolver(Module{Directory: ".", ModulePath: "example.com/m"}, packages)
	resolution, _, target, name, err := resolver.resolve("example.com/m/p16383")
	if err != nil || resolution != "local" || target == nil || *target != "p16383" ||
		name == nil || *name != "p" {
		t.Fatalf("indexed resolution = %q, %v, %v, %v, %v", resolution, target, name, err, packages)
	}
}

func TestPackageIdentifierProfileIsStableASCII(t *testing.T) {
	for _, value := range []string{"package_name", "_test", "P2"} {
		if !validPackageIdentifier(value) {
			t.Fatalf("ASCII identifier %q rejected", value)
		}
	}
	for _, value := range []string{"for", "π", string(rune(0x105c0)), "2bad", "has-hyphen"} {
		if validPackageIdentifier(value) {
			t.Fatalf("out-of-profile identifier %q accepted", value)
		}
	}
}

func TestUnicodePackageClauseProducesStableUnsupportedDiagnostic(t *testing.T) {
	content := []byte("package 世界\n")
	digest := sha256Bytes(content)
	file, _, code, err := parseGoFile(
		selectedGoFile{bytes: int64(len(content)), path: "unicode.go", sha256: digest},
		RegularFile{Content: content, Path: "unicode.go", SHA256: digest},
	)
	if err != nil || code != "go_file_unsupported_text" || file.Path != "" {
		t.Fatalf("unicode package parse = %#v, %q, %v", file, code, err)
	}
}

func BenchmarkDeriveDependenciesHighCardinality(b *testing.B) {
	module := Module{Directory: ".", ModulePath: "example.com/m"}
	packages := benchmarkPackages()
	files := benchmarkDependencyFiles()
	b.ResetTimer()
	for iteration := 0; iteration < b.N; iteration++ {
		if _, err := deriveDependencies(files, module, packages); err != nil {
			b.Fatal(err)
		}
	}
}

func benchmarkPackages() []Package {
	values := make([]Package, maxPackages)
	for index := range values {
		values[index] = Package{
			CompileFiles: []string{"x.go"}, Directory: fmt.Sprintf("p%05d", index),
			Name: "p",
		}
	}
	return values
}

func benchmarkDependencyFiles() []File {
	values := make([]File, maxImportOccurrences/maxImportsPerFile)
	for fileIndex := range values {
		imports := make([]string, maxImportsPerFile)
		for importIndex := range imports {
			sequence := fileIndex*maxImportsPerFile + importIndex
			imports[importIndex] = fmt.Sprintf("example.com/m/missing%05d", sequence)
		}
		values[fileIndex] = File{
			Imports: imports, PackageName: "p", Path: fmt.Sprintf("f%03d.go", fileIndex),
			Role: "compile",
		}
	}
	return values
}
