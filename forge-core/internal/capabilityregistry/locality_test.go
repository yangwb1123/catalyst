package capabilityregistry

import (
	"go/parser"
	"go/token"
	"os"
	"strconv"
	"strings"
	"testing"
)

func TestProductionSurfaceHasNoAmbientOrExecutionDependency(t *testing.T) {
	entries, err := os.ReadDir(".")
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		name := entry.Name()
		if entry.IsDir() || !strings.HasSuffix(name, ".go") || strings.HasSuffix(name, "_test.go") {
			continue
		}
		raw, err := os.ReadFile(name)
		if err != nil {
			t.Fatal(err)
		}
		for _, forbidden := range []string{
			"os.Getenv(", "os.ReadDir(", "exec.Command(", "http.", "sql.Open(",
			"time.Now(", "capabilitygrantcontract", "internal/asset",
		} {
			if strings.Contains(string(raw), forbidden) {
				t.Fatalf("%s contains forbidden ambient/authority token %q", name, forbidden)
			}
		}
		for _, imported := range parsedImports(t, name) {
			if strings.HasPrefix(imported, "forgeos/") {
				t.Fatalf("%s imports internal runtime package %q", name, imported)
			}
			if imported == "os/exec" || imported == "net" || imported == "net/http" ||
				imported == "database/sql" || imported == "time" {
				t.Fatalf("%s imports ambient package %q", name, imported)
			}
		}
	}
}

func TestCommandOSAccessIsOnlyExplicitInputOpen(t *testing.T) {
	raw, err := os.ReadFile("command.go")
	if err != nil {
		t.Fatal(err)
	}
	text := string(raw)
	if strings.Count(text, "os.") != 1 || !strings.Contains(text, "os.Open(source)") {
		t.Fatal("command may only open the caller-declared input source")
	}
}

func parsedImports(t *testing.T, path string) []string {
	t.Helper()
	parsed, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
	if err != nil {
		t.Fatal(err)
	}
	result := make([]string, 0, len(parsed.Imports))
	for _, item := range parsed.Imports {
		value, err := strconv.Unquote(item.Path.Value)
		if err != nil {
			t.Fatal(err)
		}
		result = append(result, value)
	}
	return result
}
