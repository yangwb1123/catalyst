package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/routing"
)

// history_wire_test.go pins PR2 — wiring the learning loop's READ side into the real run.
// PR1's wind-down WRITES scorecards.json; PR2 reads it back and threads it through the SHARED
// tier resolver as the decision-chain's final step (routing.HistoryTiebreak) for OBSERVABILITY
// ONLY. The load-bearing invariant under test: history is LOGGED but NEVER changes a tier — v1's
// candidate set is a single claude-only model per band, so HistoryTiebreak passes the tier
// through (the genuine multi-candidate shoot-out is v3's cross-vendor pool). These tests prove
// (a) the read-back is observable, (b) it is a decision no-op (tier unchanged), (c) a cold start
// / malformed file / unmapped agent all degrade honestly, and (d) PR6's drift-guard still holds
// end-to-end because history touches no tier.

// scorecardsAt writes a scorecards.json fixture at <root>/.agent/routing/scorecards.json (the
// path buildRunEngine's LoadScorecards reads) and returns root, so a test can drive the REAL
// read-back wiring rather than calling LoadScorecards by hand.
func scorecardsAt(t *testing.T, body string) (root string) {
	t.Helper()
	root = t.TempDir()
	dir := filepath.Join(root, ".agent", "routing")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatalf("mkdir scorecards dir: %v", err)
	}
	if err := os.WriteFile(filepath.Join(dir, "scorecards.json"), []byte(body), 0o644); err != nil {
		t.Fatalf("write scorecards.json: %v", err)
	}
	return root
}

// ── ① HISTORY OBSERVABILITY + SINGLE-CANDIDATE PASSTHROUGH (the headline) ─────────────────
//
// A qualifying scorecard (samples >= historyMinSamples) for the resolved (tier, task_type) must
// make HistoryTiebreak's reason OBSERVABLE in the log ("picked ... by quality"), while the
// resolver's RETURNED tier stays the budget-adjusted tier unchanged — proving the v1 single
// candidate is a decision no-op (observability, not routing).
func TestHistory_QualifyingScorecardLoggedButTierUnchanged(t *testing.T) {
	// In-budget sonnet implementer (task_type "implementation"); a qualifying scorecard exists.
	phase := asset.Phase{Name: "implementer", Agent: "implementer"} // PhaseTier => sonnet (impl floor)
	cards := []routing.Scorecard{
		{Model: routing.Sonnet, TaskType: "implementation", QualityScore: 0.91, Samples: 50, UpdatedAt: "2026-06-18T10:00:00Z"},
	}
	want := orchestrator.PhaseTier(phase, "balanced") // sonnet — the tier history must NOT change
	if want != routing.Sonnet {
		t.Fatalf("precondition: implementer routes to sonnet; got %q", want)
	}

	var logs []string
	tierOf := phaseTierResolver("balanced", func() float64 { return 0.50 }, cards, func(s string) { logs = append(logs, s) })
	got := tierOf(phase)

	if got != want {
		t.Errorf("history must NOT change the tier (single-candidate passthrough); got %q, want %q", got, want)
	}
	joined := strings.Join(logs, "\n")
	for _, sub := range []string{"history:", "picked sonnet by quality 0.91", "50 samples", "task=implementation", "observability only"} {
		if !strings.Contains(joined, sub) {
			t.Errorf("history log must surface %q (observable read-back); log=%q", sub, joined)
		}
	}
	// And no down-tier happened (in budget): the history line is the ONLY new line.
	if downTierLines(logs) != 0 {
		t.Errorf("in-budget resolve must not down-tier; down-tier lines=%d (%v)", downTierLines(logs), logs)
	}
}

// ── ② COLD START: no scorecards -> passthrough, tier unchanged, honest "no scorecard" line ──
func TestHistory_ColdStartPassthroughLogsNoScorecard(t *testing.T) {
	phase := asset.Phase{Name: "implementer", Agent: "implementer"}
	want := orchestrator.PhaseTier(phase, "balanced")

	var logs []string
	// nil cards models LoadScorecards's (nil,nil) cold start.
	tierOf := phaseTierResolver("balanced", func() float64 { return 0.50 }, nil, func(s string) { logs = append(logs, s) })
	if got := tierOf(phase); got != want {
		t.Errorf("cold start must pass the tier through unchanged; got %q, want %q", got, want)
	}
	joined := strings.Join(logs, "\n")
	if !strings.Contains(joined, "no scorecard -> tier_default") {
		t.Errorf("cold start must log the honest no-scorecard reason; log=%q", joined)
	}
}

// ── ③ UNMAPPED AGENT (harness/gate phase) -> history is SKIPPED, no line ──────────────────
//
// A phase whose agent has no task_type mapping (taskTypeForAgent ok=false) owns no scorecard
// row, so logPhaseHistory must SKIP it — exactly as the wind-down producer skips it — rather
// than fabricate a history line under a task_type it has no business owning.
func TestHistory_UnmappedAgentSkipsHistory(t *testing.T) {
	phase := asset.Phase{Name: "gate", Agent: "harness"} // not in agentTaskType
	if _, ok := taskTypeForAgent(phase.Agent); ok {
		t.Fatalf("precondition: %q must be unmapped", phase.Agent)
	}
	cards := []routing.Scorecard{
		{Model: routing.Sonnet, TaskType: "implementation", QualityScore: 0.91, Samples: 50, UpdatedAt: "x"},
	}
	var logs []string
	tierOf := phaseTierResolver("balanced", func() float64 { return 0.50 }, cards, func(s string) { logs = append(logs, s) })
	tierOf(phase)
	for _, l := range logs {
		if strings.Contains(l, "history:") {
			t.Errorf("unmapped agent must emit NO history line; got %q", l)
		}
	}
}

// ── ④ LOAD FAILURE: malformed scorecards.json -> WARNING + run continues + tier passthrough ──
//
// buildRunEngine must be FAIL-LOUD-AND-CONTINUE on a corrupt scorecards.json: log a WARNING,
// fall back to empty history, and STILL build a working engine whose tier is byte-identical to
// the cold-start path (history is enrichment, not correctness — a bad scorecard must not abort
// or re-color a run).
func TestHistory_MalformedScorecardWarnsAndContinues(t *testing.T) {
	root := scorecardsAt(t, "{ this is not valid json")
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}

	o := runOpts{root: root, mode: "balanced", executor: "command", agentCmd: "claude"}
	b := &runBudget{} // unbudgeted -> ratio 0, no down-tier; isolates the history path
	var logs []string
	eng, _, _ := buildRunEngine(wf, o, func(s string) { logs = append(logs, s) }, func(string, string, float64, time.Duration) {},
		func(string) gate.Result { return gate.Result{} }, mode.Policy{}, b)

	ce, ok := eng.Exec.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("buildRunEngine must still wire a CommandExecutor despite a bad scorecard; got %T", eng.Exec)
	}
	// The WARNING must be loud (names the unreadable scorecard).
	if !strings.Contains(strings.Join(logs, "\n"), "WARNING scorecards unreadable") {
		t.Errorf("malformed scorecards must warn loudly; logs=%q", logs)
	}
	// And the tier is the cold-start tier (opus here, unbudgeted): the bad file changed nothing.
	want := orchestrator.PhaseTier(phase, "balanced")
	if got := modelArg(t, ce.Build(phase, "balanced")); got != want {
		t.Errorf("malformed scorecard must leave the tier at the cold-start value %q; got %q", want, got)
	}
}

// ── ⑤ PR6 DRIFT-GUARD STILL HOLDS with a real scorecard loaded (history touches no tier) ──
//
// The highest-value PR2 regression: even when a QUALIFYING scorecard is loaded through the REAL
// buildRunEngine path, the three tier consumers (--model, prompt, cost stamp) must STILL resolve
// the IDENTICAL budget-adjusted tier. History only logs; it must never nudge one consumer's tier
// and silently re-introduce the drift PR6 killed.
func TestHistory_DriftGuardHoldsWithScorecardLoaded(t *testing.T) {
	// A near-budget opus implementer that down-tiers opus->sonnet, WITH a scorecard for BOTH the
	// pre- and post-adjust tiers, so history has something to say at the resolved band.
	root := scorecardsAt(t, `[
	  {"model":"sonnet","task_type":"implementation","quality_score":0.88,"samples":40,"updated_at":"2026-06-18T10:00:00Z"},
	  {"model":"opus","task_type":"implementation","quality_score":0.95,"samples":40,"updated_at":"2026-06-18T11:00:00Z"}
	]`)
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}

	const ratio = 0.85 // near-budget band -> opus down-tiers to sonnet
	want := routing.BudgetAdjustTier(orchestrator.PhaseTier(phase, "balanced"), phase.Agent, ratio)
	if want != routing.Sonnet {
		t.Fatalf("precondition: opus implementer near budget adjusts to sonnet; got %q", want)
	}

	o := runOpts{root: root, mode: "balanced", executor: "command", agentCmd: "claude"}
	b := &runBudget{cap: 1.00}
	b.seed(int64(ratio * 1e6))
	var stamped string
	eng, _, _ := buildRunEngine(wf, o, func(string) {}, func(_, m string, _ float64, _ time.Duration) { stamped = m },
		func(string) gate.Result { return gate.Result{} }, mode.Policy{}, b)
	ce := eng.Exec.(orchestrator.CommandExecutor)

	argv := ce.Build(phase, "balanced")
	ce.Observe(phase.Name, realClaudeJSON, 0)
	model, prompt := modelArg(t, argv), promptTier(t, argv)
	if model != want || prompt != want || stamped != want {
		t.Errorf("DRIFT with a scorecard loaded: --model=%q prompt=%q stamp=%q — all must equal the "+
			"adjusted tier %q (history must not move any consumer's tier)", model, prompt, stamped, want)
	}
}

// ── ⑥ BACK-COMPAT: buildRunEngine with NO scorecards -> tier byte-identical to no-history ──
//
// A run with no scorecards.json (the overwhelming v1 case) must resolve EXACTLY the tier it did
// before PR2 wired history in. We compare the engine's `--model` against the un-adjusted
// PhaseTier for an unbudgeted run: cold-start history adds at most a log line, never a tier.
func TestHistory_NoScorecardByteIdenticalTier(t *testing.T) {
	root := t.TempDir() // no .agent/routing/scorecards.json -> LoadScorecards (nil,nil)
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}
	want := orchestrator.PhaseTier(phase, "balanced") // opus, unbudgeted

	o := runOpts{root: root, mode: "balanced", executor: "command", agentCmd: "claude"}
	b := &runBudget{} // unset cap -> ratio 0
	eng, _, _ := buildRunEngine(wf, o, func(string) {}, func(string, string, float64, time.Duration) {},
		func(string) gate.Result { return gate.Result{} }, mode.Policy{}, b)
	ce := eng.Exec.(orchestrator.CommandExecutor)

	if got := modelArg(t, ce.Build(phase, "balanced")); got != want {
		t.Errorf("no-scorecard run must be byte-identical in tier; got %q, want %q", got, want)
	}
}
