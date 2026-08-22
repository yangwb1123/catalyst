package prompt

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestBuild_IncludesHeaderCardAndContext(t *testing.T) {
	got := Build("reviewer", "reviewer", "balanced", "opus",
		"# Agent: reviewer\nfresh-context, 只判不写", []string{"ADRs:\n- ADR 0001"})
	for _, want := range []string{`"reviewer" agent`, "phase=reviewer", "tier=opus",
		"## Role card", "只判不写", "## Project context", "ADR 0001"} {
		if !strings.Contains(got, want) {
			t.Errorf("prompt missing %q", want)
		}
	}
}

func TestBuild_EmptyContextOmitsSection(t *testing.T) {
	if got := Build("x", "x", "m", "sonnet", "card", nil); strings.Contains(got, "## Project context") {
		t.Errorf("empty context must omit the section; got: %s", got)
	}
}

// testRepoRoot finds the checkout containing this package so tests that bind to
// the real ADR and constraint corpus do not depend on one developer's path.
func testRepoRoot(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatalf("get working directory: %v", err)
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "forge-core", "go.mod")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatalf("ForgeOS repo root not found above %q", dir)
		}
		dir = parent
	}
}

// Gather reads the real repo: retrieved ADR titles and the ALWAYS-injected hard
// constraints must both surface so a real agent prompt carries actual project
// ground truth, not just a role. A non-empty query keeps the ADR lane populated.
func TestGather_RealRepoHasADRsAndConstraints(t *testing.T) {
	joined := strings.Join(Gather(testRepoRoot(t), "go core stack"), "\n")
	if !strings.Contains(joined, "ADR 0002") {
		t.Errorf("expected the Go-stack ADR in context; got: %.200s", joined)
	}
	if !strings.Contains(joined, "500") {
		t.Errorf("expected hard constraints (<=500 lines); got: %.300s", joined)
	}
}

// The Context Engine must inject hard constraints + RETRIEVED ADRs + memory. This
// asserts the retrieval lane (a query matching the Go-stack ADR ranks it in) AND,
// critically, that the first 6 NON-NEGOTIABLE hard constraints are STILL injected
// (never filtered out by retrieval). The ADR corpus is larger than top-K, so the
// query-specific assertion also prevents a relevant decision from being displaced.
func TestGather_RetrievesADRsAndAlwaysKeepsHardConstraints(t *testing.T) {
	ctx := Gather(testRepoRoot(t), "stack polyglot go")
	joined := strings.Join(ctx, "\n")
	// Retrieval lane: the query terms match an ADR title, so an ADR is selected.
	if !strings.Contains(joined, "ADR 0002") {
		t.Errorf("query 'stack polyglot go' should retrieve the Go-stack ADR; got: %.300s", joined)
	}
	// Hard-constraint lane: the leading AGENTS.md bullets are non-negotiable and
	// must ALWAYS be present. Pin the 1st (500-line cap), 3rd (dependency-
	// direction), and 6th (no-hardcoded-secret) — spanning the injection window's
	// head/middle/tail, so inserting a bullet that pushes the 6th hard gate out of
	// the leadingBullets(…, 6) window breaks THIS test instead of silently dropping
	// a real gate from every agent's injected ground truth.
	for _, want := range []string{"500", "依赖方向", "secret"} {
		if !strings.Contains(joined, want) {
			t.Errorf("hard constraint %q must always inject (never retrieval-filtered); got: %.400s", want, joined)
		}
	}
}

// Even a degenerate (empty) query must still inject the non-negotiable hard
// constraints — only the retrieved ADR lane goes quiet. This pins the invariant
// that hard constraints never depend on a usable query.
func TestGather_EmptyQueryStillInjectsHardConstraints(t *testing.T) {
	joined := strings.Join(Gather(testRepoRoot(t), ""), "\n")
	if !strings.Contains(joined, "500") {
		t.Errorf("hard constraints must inject even with an empty query; got: %.300s", joined)
	}
}

func TestGather_MissingRepoDegradesQuietly(t *testing.T) {
	if ctx := Gather("/nonexistent-xyz-forge", "anything"); len(ctx) != 0 {
		t.Errorf("missing repo should yield no context, got %v", ctx)
	}
}

// The task lane: Gather must inject the .agent/ROADMAP.md body so a real agent
// knows WHAT to build — the gap that left an agent with only role+constraints and
// no concrete task. Verified host-agnostically in a temp repo.
func TestGather_InjectsRoadmapTask(t *testing.T) {
	dir := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dir, ".agent"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, ".agent", "ROADMAP.md"),
		[]byte("# Roadmap\n\nimplement the widget multiplier"), 0o644); err != nil {
		t.Fatal(err)
	}
	joined := strings.Join(Gather(dir, "implementer implementer"), "\n")
	if !strings.Contains(joined, "Current task") {
		t.Errorf("Gather must inject the ROADMAP task lane; got: %.200s", joined)
	}
	if !strings.Contains(joined, "implement the widget multiplier") {
		t.Errorf("task lane must carry the ROADMAP body; got: %.200s", joined)
	}
}

// A repo with no ROADMAP degrades cleanly: no task lane, no error (a discover-stage
// project may have no roadmap yet).
func TestGather_MissingRoadmapOmitsTask(t *testing.T) {
	dir := t.TempDir()
	if strings.Contains(strings.Join(Gather(dir, "q"), "\n"), "Current task") {
		t.Error("a repo with no ROADMAP must not inject a task lane")
	}
}

// A long roadmap is capped to taskCap runes (+ marker) so it cannot blow the
// context window — the same bounding guarantee as the ADR lane.
func TestCurrentTask_CapsLongRoadmap(t *testing.T) {
	dir := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dir, ".agent"), 0o755); err != nil {
		t.Fatal(err)
	}
	big := strings.Repeat("x", taskCap+500)
	if err := os.WriteFile(filepath.Join(dir, ".agent", "ROADMAP.md"), []byte(big), 0o644); err != nil {
		t.Fatal(err)
	}
	got := currentTask(dir)
	if n := len([]rune(got)); n > taskCap+40 {
		t.Errorf("currentTask must cap near taskCap runes; got %d", n)
	}
	if !strings.Contains(got, "truncated") {
		t.Error("a capped roadmap must carry the truncation marker")
	}
}
