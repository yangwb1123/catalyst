package store

import (
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

func TestWorkspaceProductionImportBoundary(t *testing.T) {
	boundaries := map[string][]string{
		"../domain": {
			"errors", "fmt", "path", "sort", "strings", "unicode", "unicode/utf8",
			"forgeos/forge-core/internal/platformcorecontract",
		},
		"../application": {
			"bytes", "context", "crypto/rand", "encoding/json", "errors", "fmt", "strings", "time",
			"forgeos/forge-core/internal/platformcorecontract",
			"forgeos/forge-core/internal/workspace/domain",
		},
		".": {
			"context", "errors", "fmt", "forgeos/forge-core/internal/controlstore",
			"forgeos/forge-core/internal/platformcorecontract",
			"forgeos/forge-core/internal/workspace/application",
		},
	}
	for directory, allowed := range boundaries {
		t.Run(filepath.Base(directory), func(t *testing.T) {
			checkProductionImports(t, directory, allowed)
		})
	}
}

func checkProductionImports(t *testing.T, directory string, allowed []string) {
	t.Helper()
	entries, err := os.ReadDir(directory)
	if err != nil {
		t.Fatal(err)
	}
	accepted := make(map[string]bool, len(allowed))
	for _, value := range allowed {
		accepted[value] = true
	}
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".go") ||
			strings.HasSuffix(entry.Name(), "_test.go") {
			continue
		}
		checkFileImports(t, filepath.Join(directory, entry.Name()), accepted)
	}
}

func checkFileImports(t *testing.T, path string, allowed map[string]bool) {
	t.Helper()
	parsed, err := parser.ParseFile(token.NewFileSet(), path, nil, parser.ImportsOnly)
	if err != nil {
		t.Fatal(err)
	}
	for _, spec := range parsed.Imports {
		value, err := strconv.Unquote(spec.Path.Value)
		if err != nil {
			t.Fatalf("%s has invalid import: %v", path, err)
		}
		if !allowed[value] {
			t.Errorf("%s imports %q outside the Workspace boundary", path, value)
		}
	}
}
