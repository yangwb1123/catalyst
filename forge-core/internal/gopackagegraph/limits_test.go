package gopackagegraph

import (
	"context"
	"fmt"
	"sort"
	"strconv"
	"strings"
	"testing"
)

var canonicalImportPathAccepted = []string{
	"fmt", "crypto/x509", "example.com/m", "a/b-c_d~e", "a/c++",
}

var canonicalImportPathRejected = []string{
	"", "/a", "a/", "a//b", "-flag", "./relative", "../up",
	"a/.", "a/..", "a/.hidden", "a/b.", `a\b`, "a!b", `a"b`, "a:b", "a@b",
	"@response", "a/CON", "a/com1.txt", "a/x~1", "例子.com/x", "a..b",
	"a b", "a\tb",
}

func TestPrepareRejectsInvalidModuleAndSelectionLimits(t *testing.T) {
	hash := strings.Repeat("a", 64)
	tests := []struct {
		name      string
		directory string
		entries   []SourceEntry
		want      string
	}{
		{name: "unsafe directory", directory: "../mod", entries: []SourceEntry{}, want: "module_directory"},
		{name: "own go mod symlink", directory: ".", entries: []SourceEntry{
			{Bytes: 1, ContentSHA256: &hash, Kind: "symlink", Path: "go.mod"},
		}, want: "current regular"},
		{name: "unsorted", directory: ".", entries: []SourceEntry{
			{Bytes: 1, ContentSHA256: &hash, Kind: "regular", Path: "z.go"},
			{Bytes: 1, ContentSHA256: &hash, Kind: "regular", Path: "go.mod"},
		}, want: "strictly sorted"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			_, err := Prepare(test.directory, test.entries)
			if err == nil || !strings.Contains(err.Error(), test.want) {
				t.Fatalf("error = %v, want %q", err, test.want)
			}
		})
	}
}

func TestPrepareEnforcesAggregateFileAndNestedLimits(t *testing.T) {
	t.Run("aggregate parser bytes", func(t *testing.T) {
		entries := boundedSyntheticEntries(17, 0)
		_, err := Prepare(".", entries)
		if err == nil || !strings.Contains(err.Error(), "aggregate Go parser input") {
			t.Fatalf("error = %v", err)
		}
	})
	t.Run("selected files", func(t *testing.T) {
		entries := boundedSyntheticEntries(limits.GoFiles+1, -limits.GoFileBytes)
		_, err := Prepare(".", entries)
		if err == nil || !strings.Contains(err.Error(), "selected regular Go files") {
			t.Fatalf("error = %v", err)
		}
	})
	t.Run("nested modules", func(t *testing.T) {
		entries := []SourceEntry{syntheticEntry("go.mod", 1)}
		for index := 0; index <= maxNestedModules; index++ {
			entries = append(entries, syntheticEntry(fmt.Sprintf("n%04d/go.mod", index), 1))
		}
		sort.Slice(entries, func(i, j int) bool { return entries[i].Path < entries[j].Path })
		_, err := Prepare(".", entries)
		if err == nil || !strings.Contains(err.Error(), "nested modules") {
			t.Fatalf("error = %v", err)
		}
	})
}

func TestAnalyzeRejectsImportOccurrenceLimitAndCancellation(t *testing.T) {
	contents := map[string][]byte{"go.mod": []byte("module example.com/limit\n")}
	entries := []SourceEntry{fixtureSourceEntry("go.mod", "regular", contents["go.mod"])}
	for index := 0; index < 65; index++ {
		filePath := fmt.Sprintf("p%02d/x.go", index)
		contents[filePath] = []byte(exactImportCountSource(maxImportsPerFile))
		entries = append(entries, fixtureSourceEntry(filePath, "regular", contents[filePath]))
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Path < entries[j].Path })
	plan, err := Prepare(".", entries)
	if err != nil {
		t.Fatal(err)
	}
	_, err = Analyze(context.Background(), plan, fixtureRegular("go.mod", contents["go.mod"]), fixtureReadFiles(plan, contents))
	if err == nil || !strings.Contains(err.Error(), "import occurrences") {
		t.Fatalf("occurrence error = %v", err)
	}
	canceled, cancel := context.WithCancel(context.Background())
	cancel()
	_, err = Analyze(canceled, plan, fixtureRegular("go.mod", contents["go.mod"]), fixtureReadFiles(plan, contents))
	if err == nil || !strings.Contains(err.Error(), "context canceled") {
		t.Fatalf("cancellation error = %v", err)
	}
}

func TestAnalyzeDeduplicatesRepeatedImportsBeforeLimits(t *testing.T) {
	var source strings.Builder
	source.WriteString("package repeated\nimport (\n")
	for index := 0; index <= maxImportsPerFile; index++ {
		source.WriteString("_ \"fmt\"\n")
	}
	source.WriteString(")\n")
	contents := map[string][]byte{
		"go.mod": []byte("module example.com/repeated\n"), "x.go": []byte(source.String()),
	}
	plan, err := Prepare(".", fixtureEntries(contents))
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
	observation, err := analysis.Observation(1, fixtureProducer(), fixtureSource())
	if err != nil || len(observation.Files) != 1 ||
		len(observation.Files[0].Imports) != 1 || observation.Files[0].Imports[0] != "fmt" {
		t.Fatalf("repeated imports = %#v, %v", observation.Files, err)
	}
}

func TestCanonicalImportPathMatrix(t *testing.T) {
	for _, value := range canonicalImportPathAccepted {
		if !canonicalImportPath(value) {
			t.Errorf("canonicalImportPath(%q) = false", value)
		}
	}
	for _, value := range canonicalImportPathRejected {
		if canonicalImportPath(value) {
			t.Errorf("canonicalImportPath(%q) = true", value)
		}
	}
}

func TestAnalyzeClassifiesNoncanonicalImports(t *testing.T) {
	contents := map[string][]byte{
		"go.mod": []byte("module example.com/m\n"),
		"x.go":   []byte("package p\nimport (\n\"./bad\"\n\"a..b\"\n)\n"),
	}
	plan, err := Prepare(".", fixtureEntries(contents))
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
	if len(analysis.dependencies) != 2 {
		t.Fatalf("dependencies = %#v", analysis.dependencies)
	}
	for _, dependency := range analysis.dependencies {
		if dependency.Resolution != "unsupported" || dependency.ResolutionDetail == nil ||
			*dependency.ResolutionDetail != "noncanonical_import_path" ||
			dependency.TargetDirectory != nil || dependency.TargetPackageName != nil {
			t.Errorf("noncanonical dependency = %#v", dependency)
		}
	}
}

func TestAnalyzeRejectsNoncanonicalModuleDirectives(t *testing.T) {
	for _, value := range []string{"a!b", "a:b", "a..b"} {
		t.Run(fmt.Sprintf("%q", value), func(t *testing.T) {
			contents := map[string][]byte{
				"go.mod": []byte("module " + value + "\n"),
			}
			plan, err := Prepare(".", fixtureEntries(contents))
			if err != nil {
				t.Fatal(err)
			}
			_, err = Analyze(
				context.Background(), plan,
				fixtureRegular("go.mod", contents["go.mod"]), []RegularFile{},
			)
			if err == nil || !strings.Contains(err.Error(), "not canonical") {
				t.Fatalf("module directive path %q error = %v", value, err)
			}
		})
	}
}

func boundedSyntheticEntries(count int, byteAdjustment int64) []SourceEntry {
	entries := []SourceEntry{syntheticEntry("go.mod", 1)}
	for index := 0; index < count; index++ {
		entries = append(entries, syntheticEntry(
			fmt.Sprintf("p%05d.go", index), limits.GoFileBytes+byteAdjustment,
		))
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Path < entries[j].Path })
	return entries
}

func syntheticEntry(path string, size int64) SourceEntry {
	hash := strings.Repeat("a", 64)
	return SourceEntry{Bytes: size, ContentSHA256: &hash, Kind: "regular", Path: path}
}

func exactImportCountSource(count int) string {
	var value strings.Builder
	value.WriteString("package p\nimport (\n")
	for index := 0; index < count; index++ {
		value.WriteString("_ \"p")
		value.WriteString(strconv.Itoa(index))
		value.WriteString("\"\n")
	}
	value.WriteString(")\n")
	return value.String()
}
