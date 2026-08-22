package graphsnapshot

import (
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
	"testing"
)

func TestProjectorProductionImportsStayPureAndDecodeOnly(t *testing.T) {
	entries, err := os.ReadDir(".")
	if err != nil {
		t.Fatal(err)
	}
	allowedInternal := map[string]bool{
		"forgeos/forge-core/internal/gopackagedependencyobservationproducer": true,
		"forgeos/forge-core/internal/gopackagegraph":                         true,
	}
	seenDecoder := 0
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".go") ||
			strings.HasSuffix(entry.Name(), "_test.go") {
			continue
		}
		raw, readErr := os.ReadFile(entry.Name())
		if readErr != nil {
			t.Fatal(readErr)
		}
		seenDecoder += strings.Count(string(raw), "DecodeGraphObservation(")
		if strings.Contains(string(raw), ".Produce(") {
			t.Fatalf("%s invokes live ADR-0053 capture", entry.Name())
		}
		for _, imported := range importsForTest(t, entry.Name()) {
			if strings.HasPrefix(imported, "forgeos/forge-core/internal/") && !allowedInternal[imported] {
				t.Fatalf("%s imports ambient internal package %q", entry.Name(), imported)
			}
			if ambientImport(imported) && !(entry.Name() == "command.go" && imported == "os") {
				t.Fatalf("pure projector file %s imports %q", entry.Name(), imported)
			}
		}
	}
	if seenDecoder != 1 {
		t.Fatalf("DecodeGraphObservation call count = %d", seenDecoder)
	}
}

func importsForTest(t *testing.T, path string) []string {
	t.Helper()
	parsed, err := parser.ParseFile(token.NewFileSet(), filepath.Clean(path), nil, parser.ImportsOnly)
	if err != nil {
		t.Fatal(err)
	}
	result := make([]string, 0, len(parsed.Imports))
	for _, imported := range parsed.Imports {
		value, unquoteErr := strconv.Unquote(imported.Path.Value)
		if unquoteErr != nil {
			t.Fatal(unquoteErr)
		}
		result = append(result, value)
	}
	sort.Strings(result)
	return result
}

func ambientImport(value string) bool {
	return value == "context" || value == "database/sql" || value == "net" ||
		value == "net/http" || value == "os" || value == "os/exec" ||
		value == "path/filepath" || value == "time"
}
