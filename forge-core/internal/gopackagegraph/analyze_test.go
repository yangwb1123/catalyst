package gopackagegraph

import (
	"bytes"
	"context"
	"sort"
	"strconv"
	"strings"
	"testing"
)

func TestAnalyzeDerivesLexicalUnionGraphAndDiagnostics(t *testing.T) {
	contents := graphFixtureContents()
	entries := fixtureEntries(contents)
	entries = append(entries,
		fixtureSourceEntry("linked.go", "symlink", []byte("root.go")),
		fixtureSourceEntry("nested/go.mod", "symlink", []byte("elsewhere")),
		fixtureSourceEntry("nested/ignored.go", "regular", []byte("package ignored\n")),
		fixtureSourceEntry("removed.go", "deleted", nil),
		fixtureSourceEntry("too-large.go", "regular", bytes.Repeat([]byte{'x'}, int(limits.GoFileBytes+1))),
	)
	sort.Slice(entries, func(i, j int) bool { return entries[i].Path < entries[j].Path })
	plan, err := Prepare(".", entries)
	if err != nil {
		t.Fatal(err)
	}
	analysis, err := Analyze(
		context.Background(), plan, fixtureRegular("go.mod", contents["go.mod"]),
		fixtureReadFiles(plan, contents),
	)
	if err != nil {
		t.Fatal(err)
	}
	observation, err := analysis.Observation(123, fixtureProducer(), fixtureSource())
	if err != nil {
		t.Fatal(err)
	}
	assertFixtureModuleAndCoverage(t, observation)
	assertFixtureDiagnostics(t, observation.Diagnostics)
	assertFixturePackages(t, observation.Packages)
	assertFixtureResolutions(t, observation.Dependencies)
}

func graphFixtureContents() map[string][]byte {
	return map[string][]byte{
		"go.mod":         []byte("module example.com/acme // selected module\n"),
		"root.go":        []byte("package root\nimport (\n \"C\"\n \"fmt\"\n \"github.com/lib/x\"\n \"example.com/acme/local\"\n \"example.com/acme/missing\"\n \"example.com/acme/amb\"\n \"example.com/acme/nested/x\"\n \"./bad\"\n)\n"),
		"root_test.go":   []byte("package root_test\nimport \"example.com/acme\"\n"),
		"local/local.go": []byte("package local\nimport \"fmt\"\n"),
		"amb/one.go":     []byte("package one\n"),
		"amb/two.go":     []byte("package two\n"),
		"bad.go":         []byte("package broken\nimport (\n\"fmt\"\n"),
		"invalid.go":     {0xff, 0xfe},
		"unsupported.go": []byte("package unsupported\n// \u202e\n"),
		"too-many.go":    []byte(importLimitSource()),
	}
}

func importLimitSource() string {
	var value strings.Builder
	value.WriteString("package many\nimport (\n")
	for index := 0; index <= maxImportsPerFile; index++ {
		value.WriteString("_ \"p")
		value.WriteString(strconv.Itoa(index))
		value.WriteString("\"\n")
	}
	value.WriteString(")\n")
	return value.String()
}

func fixtureEntries(contents map[string][]byte) []SourceEntry {
	result := make([]SourceEntry, 0, len(contents))
	for path, content := range contents {
		result = append(result, fixtureSourceEntry(path, "regular", content))
	}
	sort.Slice(result, func(i, j int) bool { return result[i].Path < result[j].Path })
	return result
}

func fixtureSourceEntry(path, kind string, content []byte) SourceEntry {
	entry := SourceEntry{Bytes: int64(len(content)), Kind: kind, Path: path}
	if kind != "deleted" {
		digest := sha256Bytes(content)
		entry.ContentSHA256 = &digest
	}
	return entry
}

func fixtureRegular(path string, content []byte) RegularFile {
	return RegularFile{Content: append([]byte(nil), content...), Path: path, SHA256: sha256Bytes(content)}
}

func fixtureReadFiles(plan *Plan, contents map[string][]byte) []RegularFile {
	result := make([]RegularFile, 0, len(plan.GoFilePaths()))
	for _, path := range plan.GoFilePaths() {
		result = append(result, fixtureRegular(path, contents[path]))
	}
	return result
}

func fixtureProducer() Producer {
	return Producer{
		ParametersSHA256: strings.Repeat("a", 64), ProducerID: "producer",
		ProducerType: "tool", ProducerVersion: "v1", RunID: "run-1",
	}
}

func fixtureSource() Source {
	return Source{SourceRevision: "git-sha1:" + strings.Repeat("b", 40), SourceTreeSHA256: strings.Repeat("c", 64)}
}

func assertFixtureModuleAndCoverage(t *testing.T, value Observation) {
	t.Helper()
	if value.Module.ModulePath != "example.com/acme" || len(value.Module.NestedModules) != 1 ||
		value.Module.NestedModules[0].Kind != "symlink" {
		t.Fatalf("module = %#v", value.Module)
	}
	coverage := value.Coverage
	if coverage.GoEntriesExcludedByNestedModule != 1 || coverage.GoEntriesExcludedNonregular != 2 ||
		coverage.RegularGoFilesSelected != 10 || coverage.RegularGoFilesParsed != 5 ||
		coverage.RegularGoFilesWithDiagnostics != 5 || coverage.GoEntriesInSelectedSubtree != 13 {
		t.Fatalf("coverage = %#v", coverage)
	}
}

func assertFixtureDiagnostics(t *testing.T, values []Diagnostic) {
	t.Helper()
	want := map[string]string{
		"bad.go": "go_file_parse_error", "invalid.go": "go_file_invalid_utf8",
		"too-large.go":   "go_file_exceeds_parser_limit",
		"too-many.go":    "go_file_import_limit_exceeded",
		"unsupported.go": "go_file_unsupported_text",
	}
	if len(values) != len(want) {
		t.Fatalf("diagnostics = %#v", values)
	}
	for _, value := range values {
		if want[value.Path] != value.Code {
			t.Fatalf("unexpected diagnostic %#v", value)
		}
	}
}

func assertFixturePackages(t *testing.T, values []Package) {
	t.Helper()
	byKey := make(map[packageKey]Package)
	for _, value := range values {
		byKey[packageKey{directory: value.Directory, name: value.Name}] = value
	}
	if byKey[packageKey{directory: ".", name: "root"}].ImportPath == nil {
		t.Fatal("compile-bearing root package has nil import path")
	}
	if byKey[packageKey{directory: ".", name: "root_test"}].ImportPath != nil {
		t.Fatal("test-only external package claimed an import path")
	}
	if len(byKey) != 5 {
		t.Fatalf("packages = %#v", values)
	}
}

func assertFixtureResolutions(t *testing.T, values []Dependency) {
	t.Helper()
	want := map[string]string{
		"C": "cgo_pseudo", "fmt": "stdlib_candidate",
		"github.com/lib/x": "external_candidate", "example.com/acme/local": "local",
		"example.com/acme/missing":  "unresolved_local",
		"example.com/acme/amb":      "ambiguous_local",
		"example.com/acme/nested/x": "nested_module_boundary", "./bad": "unsupported",
	}
	for _, value := range values {
		if expected, exists := want[value.ImportPath]; exists && value.Resolution != expected {
			t.Fatalf("dependency %#v, want resolution %q", value, expected)
		}
		delete(want, value.ImportPath)
	}
	if len(want) != 0 {
		t.Fatalf("missing dependency resolutions %#v", want)
	}
}
