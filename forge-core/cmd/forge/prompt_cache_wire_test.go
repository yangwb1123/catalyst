package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/prompt"
)

// promptText returns the -p prompt argv element (always last — claudeArgv appends "-p" then
// the built prompt) so a test can inspect what an agent phase would actually receive.
func promptText(t *testing.T, argv []string) string {
	t.Helper()
	if len(argv) == 0 {
		t.Fatal("empty argv")
	}
	return argv[len(argv)-1]
}

// writeRepoFile writes content to a repo-relative path under root, mkdir-p'ing parents.
func writeRepoFile(t *testing.T, root, rel, content string) {
	t.Helper()
	p := filepath.Join(root, rel)
	if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

// BACK-COMPAT (PR2) — a nil ContextCache must make buildPrompt byte-for-byte identical to the
// pre-cache path (which read the invariant lanes straight through prompt.Gather). This pins
// that the cache parameter is purely additive: every existing nil-cache caller is unchanged.
func TestBuildPrompt_NilCacheByteIdenticalToGatherPath(t *testing.T) {
	root := t.TempDir()
	writeRepoFile(t, root, ".agent/ROADMAP.md", "# Roadmap\n\nbuild the thing")
	writeRepoFile(t, root, ".agent/AGENTS.md", "# AGENTS\n- rule one\n- rule two")
	writeRepoFile(t, root, "docs/adr/0001-a.md", "# ADR 0001: alpha")
	p := asset.Phase{Name: "implementer", Agent: "implementer"}

	nilCache := buildPrompt(root, p, "balanced", unbudgetedTier("balanced"), nil, nil, nil, nil)
	withCache := buildPrompt(root, p, "balanced", unbudgetedTier("balanced"), prompt.NewContextCache(), nil, nil, nil)

	// A nil cache and a fresh cache must produce the IDENTICAL prompt — the cache only changes
	// WHERE the invariant inputs come from (disk vs memo), never the resulting bytes.
	if nilCache != withCache {
		t.Errorf("nil-cache and cached buildPrompt must be byte-identical:\n nil=%q\ncached=%q", nilCache, withCache)
	}
	// Sanity: the prompt actually carries the ground truth (not a degenerate empty compare).
	if !strings.Contains(nilCache, "build the thing") || !strings.Contains(nilCache, "ADR 0001") {
		t.Errorf("prompt must carry ROADMAP + ADR ground truth; got: %.400s", nilCache)
	}
}

// RUN MULTI-PHASE HIT (PR2) — one cache shared across phases of a run serves the INVARIANT
// lanes (ADRs + AGENTS) identically to each phase, while the ROADMAP lane stays per-phase
// fresh. Two different phases (different query) built with the SAME cache must carry the
// byte-identical ADR/AGENTS blocks (the memo is reused), proving the per-run cache is live.
func TestBuildPrompt_SharedCacheReusesInvariantLanesAcrossPhases(t *testing.T) {
	root := t.TempDir()
	writeRepoFile(t, root, ".agent/ROADMAP.md", "# Roadmap\n\nthe current task")
	writeRepoFile(t, root, ".agent/AGENTS.md", "# AGENTS\n- 单文件 ≤ 500 行\n- 依赖方向向内")
	writeRepoFile(t, root, "docs/adr/0001-a.md", "# ADR 0001: alpha decision")
	writeRepoFile(t, root, "docs/adr/0002-b.md", "# ADR 0002: beta decision")

	cache := prompt.NewContextCache()
	implPrompt := buildPrompt(root, asset.Phase{Name: "implementer", Agent: "implementer"}, "balanced", unbudgetedTier("balanced"), cache, nil, nil, nil)
	revPrompt := buildPrompt(root, asset.Phase{Name: "reviewer", Agent: "reviewer"}, "balanced", unbudgetedTier("balanced"), cache, nil, nil, nil)

	// The AGENTS constraints block is identical across phases (cached, invariant).
	const agentsMarker = "Engineering constraints (hard, non-negotiable):"
	if implBlk, revBlk := blockAfter(implPrompt, agentsMarker), blockAfter(revPrompt, agentsMarker); implBlk == "" || implBlk != revBlk {
		t.Errorf("AGENTS lane must be reused (identical) across phases via the shared cache:\n impl=%q\n rev=%q", implBlk, revBlk)
	}
	// Both phases see the same ROADMAP body (it is re-read each call, but unchanged here).
	for _, pr := range []string{implPrompt, revPrompt} {
		if !strings.Contains(pr, "the current task") {
			t.Errorf("each phase prompt must carry the ROADMAP task; got: %.300s", pr)
		}
	}
}

// ★ EVOLVE CROSS-ITERATION ROADMAP NEVER STALE (PR2, the load-bearing wiring test) ★
// End-to-end through the REAL buildRunEngine: it owns the per-run ContextCache, shared across
// every phase AND (for evolve) across iterations of the same Engine. We drive the engine's
// CommandExecutor.Build once (iteration 1), then REWRITE .agent/ROADMAP.md (simulating an
// implementer ticking a [x] / advancing the board), then Build AGAIN on the SAME engine
// (iteration 2). The second prompt MUST carry the rewritten body — proving the per-run cache
// (created in buildRunEngine, reused across iterations) NEVER serves a stale ROADMAP, because
// the ROADMAP lane is re-read every call and the cache holds no field for it.
func TestBuildRunEngine_EvolveCrossIterationRoadmapNeverStale(t *testing.T) {
	root := t.TempDir()
	writeRepoFile(t, root, ".agent/ROADMAP.md", "# Roadmap\n\nITERATION-ONE implement the alpha feature")
	writeRepoFile(t, root, ".agent/AGENTS.md", "# AGENTS\n- 单文件 ≤ 500 行")
	writeRepoFile(t, root, "docs/adr/0001-a.md", "# ADR 0001: alpha")

	phase := asset.Phase{Name: "implementer", Agent: "implementer"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}
	o := runOpts{root: root, mode: "balanced", executor: "command", agentCmd: "claude"}
	b := &runBudget{} // unbudgeted: ratio 0, no down-tier — isolates the cache path

	eng, _, _ := buildRunEngine(wf, o, func(string) {}, func(string, string, float64, time.Duration) {},
		nil, mode.Policy{}, b, "", nil)
	ce, ok := eng.Exec.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("buildRunEngine must wire a CommandExecutor; got %T", eng.Exec)
	}

	// Iteration 1: build the prompt — this lazily builds the per-run cache's invariant lanes.
	first := promptText(t, ce.Build(phase, "balanced"))
	if !strings.Contains(first, "ITERATION-ONE implement the alpha feature") {
		t.Fatalf("iteration 1 prompt must carry ROADMAP v1; got: %.300s", first)
	}

	// The implementer advances the board: .agent/ROADMAP.md is REWRITTEN (commit e6d2e13's
	// print-mode agent really edits this file). evolve loops back into the SAME engine.
	writeRepoFile(t, root, ".agent/ROADMAP.md", "# Roadmap\n\nITERATION-TWO implement the beta feature")

	// Iteration 2: SAME engine, SAME cache. The ROADMAP must be the rewritten body.
	second := promptText(t, ce.Build(phase, "balanced"))
	if !strings.Contains(second, "ITERATION-TWO implement the beta feature") {
		t.Errorf("★ STALE ROADMAP ★ — iteration 2 must read the rewritten v2 body through the shared "+
			"per-run cache; the ROADMAP must NEVER be cached. got: %.300s", second)
	}
	if strings.Contains(second, "ITERATION-ONE") {
		t.Errorf("iteration 2 still shows the stale v1 ROADMAP — the task lane was wrongly cached; got: %.300s", second)
	}
	// And the INVARIANT lane (AGENTS) IS reused identically across the two iterations.
	const agentsMarker = "Engineering constraints (hard, non-negotiable):"
	if b1, b2 := blockAfter(first, agentsMarker), blockAfter(second, agentsMarker); b1 == "" || b1 != b2 {
		t.Errorf("AGENTS lane must be reused (identical) across evolve iterations:\n iter1=%q\n iter2=%q", b1, b2)
	}
}

// blockAfter returns the text from the line carrying marker through the end of that prompt
// context lane (up to the next blank-line lane separator "\n\n"), or "" when the marker is
// absent. Used to isolate ONE context lane (e.g. the AGENTS block) for a cross-call compare.
func blockAfter(s, marker string) string {
	i := strings.Index(s, marker)
	if i < 0 {
		return ""
	}
	rest := s[i:]
	if end := strings.Index(rest, "\n\n"); end >= 0 {
		return rest[:end]
	}
	return rest
}
