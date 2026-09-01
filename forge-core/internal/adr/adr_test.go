// Package adr contains automated tests that verify architectural decisions
// recorded in ForgeOS Architecture Decision Records (docs/adr/) continue to
// hold in the current codebase (eighth-wave-adr-decay.md §方向1: ADR 可测试性).
//
// Each ADR gets its own test file / test group. A failing test means a decision
// has decayed — the code no longer matches the ADR's commitment. This is a
// deliberate design choice: every ADR must be falsifiable by an automated test,
// so "Accepted" means "passes its tests".
//
// These tests themselves use only the Go standard library plus forge-core's
// internal packages; the reviewed App Server SQLite closure is checked below.
package adr

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/doctor"
)

// repoRoot attempts to find the ForgeOS repo root by looking for
// forge-core/go.mod.
func repoRoot(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Skip("cannot determine working directory")
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "forge-core", "go.mod")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Skip("not inside ForgeOS repo (forge-core/go.mod not found in any parent)")
		}
		dir = parent
	}
}

// ───────────────────────────────────────────────────────────────────────────
// ADR-0001: v0–v1 Ride Claude Code, v2 自研运行时
// ───────────────────────────────────────────────────────────────────────────

// TestADR0001_ForgeCoreExists checks the primary decision of ADR-0001:
// forge-core is a self-hosted Go runtime, not a Claude Code wrapper.
func TestADR0001_ForgeCoreExists(t *testing.T) {
	root := repoRoot(t)
	cmd := exec.Command("go", "build", "-o", os.DevNull, "./cmd/forge")
	cmd.Dir = filepath.Join(root, "forge-core")
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("ADR-0001 violation: forge-core must compile as a Go binary: %v\n%s", err, out)
	}
}

// TestForgeCoreDependencyPolicy keeps the App Server's sole external module
// exact and confined. ADR-0001 is superseded and ADR-0002 permits procured
// infrastructure; neither records the former all-module zero-dependency claim.
func TestForgeCoreDependencyPolicy(t *testing.T) {
	root := repoRoot(t)
	if err := doctor.CheckModuleDependencyPolicy(filepath.Join(root, "forge-core")); err != nil {
		t.Fatal(err)
	}
}

func TestForgeCoreDependencyPolicyCatchesUnexpectedModule(t *testing.T) {
	root := repoRoot(t)
	forgeDir := filepath.Join(root, "forge-core")
	goMod, err := os.ReadFile(filepath.Join(forgeDir, "go.mod"))
	if err != nil {
		t.Fatal(err)
	}
	goSum, err := os.ReadFile(filepath.Join(forgeDir, "go.sum"))
	if err != nil {
		t.Fatal(err)
	}
	goMod = append(goMod, []byte("require golang.org/x/net v0.20.0\n")...)
	if err := doctor.CheckModuleManifests(goMod, goSum); err == nil {
		t.Fatal("unexpected module escaped the exact dependency policy")
	}
}

func TestForgeCoreDependencyPolicyCatchesChecksumDrift(t *testing.T) {
	root := repoRoot(t)
	forgeDir := filepath.Join(root, "forge-core")
	goMod, err := os.ReadFile(filepath.Join(forgeDir, "go.mod"))
	if err != nil {
		t.Fatal(err)
	}
	goSum, err := os.ReadFile(filepath.Join(forgeDir, "go.sum"))
	if err != nil {
		t.Fatal(err)
	}
	goSum = append(goSum, []byte("unexpected checksum\n")...)
	if err := doctor.CheckModuleManifests(goMod, goSum); err == nil {
		t.Fatal("checksum drift escaped the exact dependency policy")
	}
}

// ───────────────────────────────────────────────────────────────────────────
// ADR-0002: Go-核心 Polyglot 栈，分期引入
// ───────────────────────────────────────────────────────────────────────────

// TestADR0002_ForgeCoreIsGo checks the primary decision: forge-core = Go.
func TestADR0002_ForgeCoreIsGo(t *testing.T) {
	root := repoRoot(t)
	forgeDir := filepath.Join(root, "forge-core")
	entries, err := os.ReadDir(forgeDir)
	if err != nil {
		t.Fatalf("read forge-core dir: %v", err)
	}
	// Must have Go source files (cmd/ or internal/).
	hasGo := false
	for _, e := range entries {
		if e.IsDir() && (e.Name() == "cmd" || e.Name() == "internal") {
			hasGo = true
			break
		}
	}
	if !hasGo {
		t.Error("ADR-0002 violation: forge-core has no cmd/ or internal/ directories")
	}
}

// TestADR0002_PolyglotNotStarted checks that the still-deferred Python and TS
// runtimes have not been introduced yet.
//
// Deliberately t.Logf, never t.Error: their appearance means legitimate v3
// progress (a milestone marker), not a decayed decision — the ONE case in
// this file's four "soft" ADR-decay tests where a failing assertion would be
// actively wrong, not just weak.
func TestADR0002_PolyglotNotStarted(t *testing.T) {
	root := repoRoot(t)
	expectedAbsent := []string{"forge-ai", "forge-web"}
	for _, name := range expectedAbsent {
		path := filepath.Join(root, name)
		if fi, err := os.Stat(path); err == nil && fi.IsDir() {
			t.Logf("ADR-0002: %s/ detected — polyglot stage advancing (expected for v3, not v2)", name)
		}
	}
}

func TestADR0006_RustRuntimeSliceExists(t *testing.T) {
	root := repoRoot(t)
	required := []string{
		"forge-runtime/Cargo.toml",
		"docs/adr/0006-pi-inspired-agent-runtime-boundary.md",
	}
	for _, relative := range required {
		if _, err := os.Stat(filepath.Join(root, relative)); err != nil {
			t.Errorf("ADR-0006 artifact missing: %s: %v", relative, err)
		}
	}
}

func TestADR0007_ConversationHubSliceExists(t *testing.T) {
	root := repoRoot(t)
	required := []string{
		"docs/adr/0007-local-first-conversation-hub.md",
		"forge-runtime/crates/infrastructure/src/sqlite_hub/schema.rs",
		"forge-runtime/crates/interfaces/tests/cli_hub.rs",
	}
	for _, relative := range required {
		if _, err := os.Stat(filepath.Join(root, relative)); err != nil {
			t.Errorf("ADR-0007 artifact missing: %s: %v", relative, err)
		}
	}
}

// TestADR0002_HarnessIsNodeJS checks that the harness is still Node.js
// (the transitional state documented in ADR-0002). Deliberately t.Log, never
// t.Error: harness losing its .mjs files means the Go-binary consolidation
// ADR-0002 itself calls "未来" (future work) has landed — a milestone, not
// a decayed decision.
func TestADR0002_HarnessIsNodeJS(t *testing.T) {
	root := repoRoot(t)
	harnessDir := filepath.Join(root, "harness")
	entries, err := os.ReadDir(harnessDir)
	if err != nil {
		t.Skipf("harness dir not found: %v", err)
	}
	hasMJS := false
	for _, e := range entries {
		if strings.HasSuffix(e.Name(), ".mjs") {
			hasMJS = true
			break
		}
	}
	if !hasMJS {
		t.Log("ADR-0002: harness no longer has .mjs files — Go-binary consolidation may be complete")
	}
}

// ───────────────────────────────────────────────────────────────────────────
// ADR-0004: REVIEW 阶段 AI-SDLC 深度评审集成
// ───────────────────────────────────────────────────────────────────────────

// TestADR0004_ReviewWorkflowExists checks that the review.yml workflow was created.
func TestADR0004_ReviewWorkflowExists(t *testing.T) {
	root := repoRoot(t)
	path := filepath.Join(root, ".agent", "workflows", "review.yml")
	if _, err := os.Stat(path); err != nil {
		t.Errorf("ADR-0004 violation: review.yml workflow not found at %s", path)
	}
}

// TestADR0004_ReviewAgentCardsExist checks that the four reviewer agent cards exist.
func TestADR0004_ReviewAgentCardsExist(t *testing.T) {
	root := repoRoot(t)
	agents := []string{"security-engineer", "distributed-engineer", "performance-engineer", "cto"}
	for _, name := range agents {
		path := filepath.Join(root, ".agent", "agents", name+".md")
		if _, err := os.Stat(path); err != nil {
			t.Errorf("ADR-0004: reviewer agent card %s.md not found", name)
		}
	}
}

// TestADR0004_RoleCheckFileExists checks the review.yml's role_check.md exists
// (referenced by the workflow). Deliberately t.Logf, never t.Error: the file
// is genuinely optional (its own log message says so) — nothing in ADR-0004
// requires it, so a missing file is not a decayed decision.
func TestADR0004_RoleCheckFileExists(t *testing.T) {
	root := repoRoot(t)
	path := filepath.Join(root, ".ai", "role_checks", "01-general.md")
	if _, err := os.Stat(path); err != nil {
		t.Logf("ADR-0004: role check file not found at %s (optional). err: %v", path, err)
	}
}

// ───────────────────────────────────────────────────────────────────────────
// Cross-ADR compliance
// ───────────────────────────────────────────────────────────────────────────

// TestCrossADR_HarnessNotInGo ensures harness scripts are NOT inside forge-core.
// This enforces the boundary between forge-core (Go runtime) and harness
// (Node.js governance gates).
func TestCrossADR_HarnessNotInForgeCore(t *testing.T) {
	root := repoRoot(t)
	forgeCore := filepath.Join(root, "forge-core")
	// Walk the tree looking for .mjs files inside forge-core — they should not be there.
	err := filepath.WalkDir(forgeCore, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() && strings.HasSuffix(d.Name(), ".mjs") {
			t.Errorf("Cross-ADR: found Node.js .mjs file inside forge-core/: %s", path)
		}
		return nil
	})
	if err != nil {
		t.Fatalf("walk forge-core: %v", err)
	}
}
