package auditprojection

import (
	"go/ast"
	"go/parser"
	"go/token"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
)

// TestProjectionPackageHasNoOutboundOrExecutionBoundary keeps this package a
// deterministic contract seam. A future Audit Governance relay needs its own
// accepted decision, Hub-owned outbox, tenant source binding, credentials,
// receipt handling, and retry policy; it must not silently appear beside the
// pure projector.
func TestProjectionPackageHasNoOutboundOrExecutionBoundary(t *testing.T) {
	entries, err := os.ReadDir(".")
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		name := entry.Name()
		if entry.IsDir() || filepath.Ext(name) != ".go" || strings.HasSuffix(name, "_test.go") {
			continue
		}
		file, err := parser.ParseFile(token.NewFileSet(), name, nil, 0)
		if err != nil {
			t.Fatalf("parse %s: %v", name, err)
		}
		assertNoEffectImports(t, name, file)
		assertNoEffectEntryPoints(t, name, file)
	}
}

// TestForgeDoesNotDependOnUnapprovedEcosystemServices makes the revalidated
// integration boundary executable. Snaplink is intentionally absent from this
// denylist because it is the authenticated human-identity authority; the
// other named services need their own accepted consumer contracts.
func TestForgeDoesNotDependOnUnapprovedEcosystemServices(t *testing.T) {
	assertSourceOmits(t, filepath.Join("..", "..", "go.mod"), []string{
		"github.com/aero/aero-id",
		"github.com/aero/aero-im",
		"github.com/aero/aero-vault",
		"snaplink-audit-governance",
		"snaplink-console",
	})
	for _, name := range []string{"routes.go", "execution_routes.go"} {
		assertSourceOmits(t, filepath.Join("..", "appserver", name), []string{
			"/api/v1/devices",
			"/api/v1/tasks",
			"/api/v1/events",
			"AgentHub",
			"agent-hub",
		})
	}
}

func assertSourceOmits(t *testing.T, name string, forbidden []string) {
	t.Helper()
	content, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	for _, value := range forbidden {
		if strings.Contains(string(content), value) {
			t.Fatalf("%s contains forbidden unapproved ecosystem dependency %q", name, value)
		}
	}
}

func assertNoEffectImports(t *testing.T, name string, file *ast.File) {
	t.Helper()
	for _, spec := range file.Imports {
		path, err := strconv.Unquote(spec.Path.Value)
		if err != nil {
			t.Fatalf("unquote import in %s: %v", name, err)
		}
		switch path {
		case "database/sql", "net", "net/http", "net/url", "os/exec", "runtime", "syscall":
			t.Fatalf("%s imports %q; audit projection must remain offline", name, path)
		}
	}
}

func assertNoEffectEntryPoints(t *testing.T, name string, file *ast.File) {
	t.Helper()
	for _, declaration := range file.Decls {
		function, ok := declaration.(*ast.FuncDecl)
		if !ok {
			continue
		}
		switch function.Name.Name {
		case "Publish", "Send", "Enqueue", "Dispatch", "Execute", "Reserve", "Register", "Heartbeat":
			t.Fatalf("%s declares %s; audit projection must not perform effects", name, function.Name.Name)
		}
	}
}
