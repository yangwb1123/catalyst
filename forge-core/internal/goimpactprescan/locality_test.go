package goimpactprescan

import (
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"sort"
	"strconv"
	"strings"
	"testing"
)

func TestSourceImportsStayPureAndBounded(t *testing.T) {
	dir := sourceDir(t)
	for _, name := range sortedSourceNames() {
		got := sourceImports(t, filepath.Join(dir, name))
		want := expectedSourceImports()[name]
		if !reflect.DeepEqual(got, want) {
			t.Fatalf("%s imports = %v, want %v", name, got, want)
		}
	}
}

func TestCommandAndBuilderStayOnDecodeOnlyPath(t *testing.T) {
	dir := sourceDir(t)
	build := readSource(t, filepath.Join(dir, "build.go"))
	if !strings.Contains(build, "DecodeGraphObservation(") || strings.Contains(build, "Produce(") {
		t.Fatal("build must decode caller-supplied graph bytes without live capture")
	}
	command := readSource(t, filepath.Join(dir, "command.go"))
	if strings.Count(command, "os.") != 1 || !strings.Contains(command, "os.Open(source)") {
		t.Fatal("command may only use os.Open for explicit --input")
	}
	all := allNonTestSource(t, dir)
	for _, token := range forbiddenAmbientTokens() {
		if strings.Contains(all, token) {
			t.Fatalf("forbidden ambient access token %q present", token)
		}
	}
}

func sourceDir(t *testing.T) string {
	t.Helper()
	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller failed")
	}
	return filepath.Dir(file)
}

func sortedSourceNames() []string {
	names := make([]string, 0, len(expectedSourceImports()))
	for name := range expectedSourceImports() {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

func sourceImports(t *testing.T, path string) []string {
	t.Helper()
	file, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
	if err != nil {
		t.Fatal(err)
	}
	result := make([]string, 0, len(file.Imports))
	for _, item := range file.Imports {
		value, err := strconv.Unquote(item.Path.Value)
		if err != nil {
			t.Fatal(err)
		}
		result = append(result, value)
	}
	sort.Strings(result)
	return result
}

func readSource(t *testing.T, path string) string {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(raw)
}

func allNonTestSource(t *testing.T, dir string) string {
	t.Helper()
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatal(err)
	}
	parts := make([]string, 0, len(entries))
	for _, entry := range entries {
		name := entry.Name()
		if entry.IsDir() || !strings.HasSuffix(name, ".go") || strings.HasSuffix(name, "_test.go") {
			continue
		}
		parts = append(parts, readSource(t, filepath.Join(dir, name)))
	}
	sort.Strings(parts)
	return strings.Join(parts, "\n")
}

func forbiddenAmbientTokens() []string {
	return []string{
		"Produce(", "exec.Command(", "http.", "os.Getenv(", "sql.Open(",
		"gitworktreesource.", "time.Now(",
	}
}

func expectedSourceImports() map[string][]string {
	return map[string][]string{
		"build.go":      {"encoding/base64", "fmt", "forgeos/forge-core/internal/gopackagedependencyobservationproducer", "forgeos/forge-core/internal/gopackagegraph", "sort"},
		"canonical.go":  {"bytes", "crypto/sha256", "encoding/hex", "encoding/json", "fmt"},
		"clone.go":      {},
		"command.go":    {"errors", "flag", "fmt", "io", "os"},
		"decode.go":     {"bytes", "encoding/base64", "encoding/json", "fmt", "io", "unicode/utf8"},
		"graph.go":      {"fmt", "forgeos/forge-core/internal/gopackagegraph", "sort"},
		"json_shape.go": {"bytes", "encoding/json", "fmt", "io", "strconv", "unicode", "unicode/utf8"},
		"path.go":       {"path", "strings", "unicode", "unicode/utf8"},
		"seeds.go":      {"forgeos/forge-core/internal/gopackagegraph", "path", "sort", "strings"},
		"status.go":     {"forgeos/forge-core/internal/gopackagegraph", "sort"},
		"traverse.go":   {"fmt", "sort"},
		"types.go":      {},
	}
}
