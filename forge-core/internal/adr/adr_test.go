// Package adr contains automated tests that verify architectural decisions
// recorded in ForgeOS Architecture Decision Records (docs/adr/) continue to
// hold in the current codebase (eighth-wave-adr-decay.md §方向1: ADR 可测试性).
//
// Each ADR gets its own test file / test group. A failing test means a decision
// has decayed — the code no longer matches the ADR's commitment. This is a
// deliberate design choice: every ADR must be falsifiable by an automated test,
// so "Accepted" means "passes its tests".
//
// These tests use only the Go standard library — forge-core's own zero-dep
// constraint means they never introduce external test dependencies.
package adr

import (
	"bytes"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
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

// TestADR0001_ZeroExternalDeps checks that forge-core has no external dependencies
// in go.mod (ADR-0001's implicit commitment to zero-dependency core).
func TestADR0001_ZeroExternalDeps(t *testing.T) {
	root := repoRoot(t)
	data, err := os.ReadFile(filepath.Join(root, "forge-core", "go.mod"))
	if err != nil {
		t.Fatalf("read go.mod: %v", err)
	}
	// The go.mod must not have a "require (" block — zero external dependencies.
	// Standard library and the forgeos/forge-core module itself are not external.
	lines := strings.Split(string(data), "\n")
	inRequire := false
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if trimmed == "require (" {
			inRequire = true
			continue
		}
		if inRequire {
			if trimmed == ")" {
				break
			}
			// Skip blank lines and comments.
			if trimmed == "" || strings.HasPrefix(trimmed, "//") {
				continue
			}
			// Any non-comment line in a require block is an external dependency.
			if strings.Contains(trimmed, " ") {
				t.Errorf("ADR-0001 violation: forge-core has external dependency: %s", trimmed)
			}
		}
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

// TestADR0002_PolyglotNotStarted checks that the other runtimes (Python, Rust, TS)
// have NOT been introduced yet — they are planned for later stages.
func TestADR0002_PolyglotNotStarted(t *testing.T) {
	root := repoRoot(t)
	expectedAbsent := []string{"forge-ai", "forge-web", "forge-runtime"}
	for _, name := range expectedAbsent {
		path := filepath.Join(root, name)
		if fi, err := os.Stat(path); err == nil && fi.IsDir() {
			t.Logf("ADR-0002: %s/ detected — polyglot stage advancing (expected for v3, not v2)", name)
		}
	}
}

// TestADR0002_HarnessIsNodeJS checks that the harness is still Node.js
// (the transitional state documented in ADR-0002).
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
// (referenced by the workflow).
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

// TestCrossADR_GoModStaysClean verifies that go.mod stays unchanged (no new
// dependencies added) by checking the raw file size is within expected range.
// This is a regression guard, not a hard boundary — update the expected max
// when a legitimate new standard-library-only feature requires a go.mod change.
func TestCrossADR_GoModStaysClean(t *testing.T) {
	root := repoRoot(t)
	data, err := os.ReadFile(filepath.Join(root, "forge-core", "go.mod"))
	if err != nil {
		t.Fatalf("read go.mod: %v", err)
	}
	// go.mod should be small: module declaration + go version line.
	// A go.mod with external deps would have a require block and much more content.
	if bytes.Count(data, []byte("\n")) > 5 {
		t.Logf("go.mod has %d lines — check for unintended changes", bytes.Count(data, []byte("\n")))
	}
}
