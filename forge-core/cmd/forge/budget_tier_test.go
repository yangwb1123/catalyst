package main

import (
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/routing"
)

// budget_tier_test.go pins PR6 — the budget-aware near-budget DOWN-TIER, the cmd/forge
// post-filter that realizes routing's budget_guard on the agent-phase tier. The pure tier
// math (the 0.80 gate, the opusFloor exemption, the Haiku clamp) is proven in
// internal/routing/routing_test.go (TestBudgetAdjustTier); HERE we prove the WIRING: that
// the ONE shared resolver feeds all three tier consumers identically (the drift-guard), that
// the spend ratio is read at SPAWN time (not frozen at engine-build), and that an unbudgeted
// run is byte-for-byte unchanged. Each test drives the REAL executor buildRunEngine wires.

// tierConsumers resolves all THREE per-phase tier consumers off ONE executor built by
// buildRunEngine at the given spend ratio, returning what each independently reports:
//   - model: the token `claude --model` receives (ce.Build argv),
//   - prompt: the tier the prompt STATES (the -p prompt, ce.Build's last argv element),
//   - stamp: the model the cost is ATTRIBUTED to (driven through the live Observe sink).
//
// The cost stamp is captured by passing a recording costSink into buildRunEngine and firing
// Observe with a real claude envelope — the exact production path, no parallel executor.
func tierConsumers(t *testing.T, wf asset.Workflow, p asset.Phase, ratio float64) (model, prompt, stamp string) {
	t.Helper()
	o := runOpts{root: repoRoot(), mode: "balanced", executor: "command", agentCmd: "claude"}
	b := &runBudget{cap: 1.00}
	b.seed(int64(ratio * 1e6)) // ratio of a $1.00 cap -> SpendRatio() == ratio (seed, no feed)

	var stamped string
	recordSink := func(_, model string, _ float64, _ time.Duration) { stamped = model }
	eng := buildRunEngine(wf, o, func(string) {}, recordSink,
		func(string) gate.Result { return gate.Result{} }, mode.Policy{}, b)
	ce, ok := eng.Exec.(orchestrator.CommandExecutor)
	if !ok {
		t.Fatalf("buildRunEngine must wire a CommandExecutor for claude, got %T", eng.Exec)
	}

	argv := ce.Build(p, "balanced")
	ce.Observe(p.Name, realClaudeJSON, 0) // drives the cost sink -> records the stamped model
	return modelArg(t, argv), promptTier(t, argv), stamped
}

// modelArg extracts the token after "--model" in a built argv (the tier handed to claude).
func modelArg(t *testing.T, argv []string) string {
	t.Helper()
	for i, a := range argv {
		if a == "--model" && i+1 < len(argv) {
			return argv[i+1]
		}
	}
	t.Fatalf("argv carries no --model token: %v", argv)
	return ""
}

// promptTier extracts the "tier=<X>" the prompt states (the LAST argv element is the -p prompt).
func promptTier(t *testing.T, argv []string) string {
	t.Helper()
	prompt := argv[len(argv)-1]
	const marker = "tier="
	i := strings.Index(prompt, marker)
	if i < 0 {
		t.Fatalf("prompt states no tier=: %.200s", prompt)
	}
	rest := prompt[i+len(marker):]
	if end := strings.IndexAny(rest, " ).\n"); end >= 0 {
		return rest[:end]
	}
	return rest
}

// ── ① DRIFT-GUARD (the highest-value test): the three tier consumers AGREE ──────────────
//
// For a near-budget phase, `claude --model`, the prompt's stated tier, and the cost stamp
// must resolve to the IDENTICAL budget-adjusted tier. A single shared resolver makes drift
// impossible; this test would FAIL the instant any consumer recomputed PhaseTier on its own
// (run model X, prompt says Y, cost charged Z) — the exact three-way drift the design forbids.
func TestBudgetTier_DriftGuard_ThreeConsumersAgree(t *testing.T) {
	// A non-floor phase pinned to opus via model_tier, so near budget it DOWN-tiers opus->sonnet:
	// a non-trivial adjustment (not a no-op) that all three consumers must reflect alike.
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}

	const ratio = 0.85 // near-budget band
	want := routing.BudgetAdjustTier(orchestrator.PhaseTier(phase, "balanced"), phase.Agent, ratio)
	if want != routing.Sonnet {
		t.Fatalf("precondition: opus implementer near budget must adjust to sonnet; got %q", want)
	}

	model, prompt, stamp := tierConsumers(t, wf, phase, ratio)
	if model != want || prompt != want || stamp != want {
		t.Errorf("THREE-WAY DRIFT: --model=%q, prompt tier=%q, cost stamp=%q — all must equal the "+
			"shared adjusted tier %q (one resolver, no drift)", model, prompt, stamp, want)
	}
}

// The drift-guard must also hold for an opusFloor (judgement) role: near budget the reviewer
// is EXEMPT, so all three consumers must report opus UNCHANGED — proving the exemption is
// applied once in the shared resolver, not forgotten by one consumer.
func TestBudgetTier_DriftGuard_FloorAgentExemptEverywhere(t *testing.T) {
	phase := asset.Phase{Name: "review", Agent: "reviewer"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}

	model, prompt, stamp := tierConsumers(t, wf, phase, 0.95) // deep in near-budget band
	if model != routing.Opus || prompt != routing.Opus || stamp != routing.Opus {
		t.Errorf("reviewer is opusFloor-exempt near budget; all consumers must stay opus, got "+
			"--model=%q prompt=%q stamp=%q", model, prompt, stamp)
	}
}

// ── ② RATIO READ AT SPAWN TIME, not engine-build time ───────────────────────────────────
//
// The resolver reads SpendRatio via a PULLER, so spend that accumulates BETWEEN spawns is
// reflected: a phase resolved while the budget is in-budget gets the full tier; the SAME
// executor resolving the SAME phase after spend crosses into the near-budget band gets the
// down-tiered one. This models two sequential phases where the first pushes the run near
// budget — the second must see the higher ratio, proving the read is per-spawn not cached.
func TestBudgetTier_RatioReadAtSpawnNotCached(t *testing.T) {
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}

	o := runOpts{root: repoRoot(), mode: "balanced", executor: "command", agentCmd: "claude"}
	b := &runBudget{cap: 1.00} // starts empty -> ratio 0
	eng := buildRunEngine(wf, o, func(string) {}, func(string, string, float64, time.Duration) {},
		func(string) gate.Result { return gate.Result{} }, mode.Policy{}, b)
	ce := eng.Exec.(orchestrator.CommandExecutor)

	// First spawn: in budget (ratio 0) -> the full opus tier, no down-tier.
	if got := modelArg(t, ce.Build(phase, "balanced")); got != routing.Opus {
		t.Fatalf("first spawn in-budget must route opus; got %q", got)
	}
	// The first phase bills, pushing the run into the near-budget band (0.85 of $1.00).
	b.feed(nil)(phase.Name, routing.Opus, 0.85, 0)
	// Second spawn of the SAME executor: the puller now reads 0.85 -> down-tier opus->sonnet.
	// If the ratio were cached at engine-build (0), this would still read opus — the bug.
	if got := modelArg(t, ce.Build(phase, "balanced")); got != routing.Sonnet {
		t.Errorf("second spawn after spend crossed into near-budget must down-tier to sonnet "+
			"(ratio read at SPAWN, not cached at build); got %q", got)
	}
}

// ── ③ BACK-COMPAT: no budget -> byte-identical, all three consumers ──────────────────────
//
// With no --run-budget-usd (cap 0 -> SpendRatio 0 -> ratio < 0.80), BudgetAdjustTier returns
// the routed tier verbatim: `--model`, the prompt, and the cost stamp must be EXACTLY the
// un-adjusted PhaseTier, proving an unbudgeted run is unchanged by PR6 end to end. Even an
// unbudgeted run still TALLIES spend, so we also spend heavily to prove ratio stays 0.
func TestBudgetTier_NoBudgetByteIdenticalAllConsumers(t *testing.T) {
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}
	want := orchestrator.PhaseTier(phase, "balanced") // the un-adjusted routed tier (opus here)

	o := runOpts{root: repoRoot(), mode: "balanced", executor: "command", agentCmd: "claude"}
	b := &runBudget{} // cap 0: unset, the back-compat hinge
	var stamped string
	eng := buildRunEngine(wf, o, func(string) {}, func(_, m string, _ float64, _ time.Duration) { stamped = m },
		func(string) gate.Result { return gate.Result{} }, mode.Policy{}, b)
	ce := eng.Exec.(orchestrator.CommandExecutor)

	b.feed(nil)(phase.Name, routing.Opus, 9.99, 0) // unbudgeted spend -> ratio stays 0, no adjustment
	argv := ce.Build(phase, "balanced")
	ce.Observe(phase.Name, realClaudeJSON, 0)
	if m, p := modelArg(t, argv), promptTier(t, argv); m != want || p != want || stamped != want {
		t.Errorf("unbudgeted run must be byte-identical (no down-tier); want %q, got --model=%q prompt=%q stamp=%q",
			want, m, p, stamped)
	}
}

// ── ④ HONEST DOWN-TIER LOG: a real adjustment is logged, a no-op is silent ───────────────
//
// When the resolver actually lowers the tier it must LOG the down-tier (the ratio + both
// tiers + the trade-off), so the operator sees why a cheaper model ran. When nothing changes
// (in budget, or a floor agent), it must log NOTHING — an unbudgeted/exempt run's log is
// byte-for-byte unchanged. This pins the honesty contract on the log itself.
func TestBudgetTier_DownTierLogsHonestly(t *testing.T) {
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}

	// (a) Near budget, non-floor -> a down-tier IS logged with ratio and both tiers.
	// nil cards = cold start: history adds only its own line, never a tier change (PR2).
	var logs []string
	tierOf := phaseTierResolver("balanced", func() float64 { return 0.85 }, nil, func(s string) { logs = append(logs, s) })
	if got := tierOf(phase); got != routing.Sonnet {
		t.Fatalf("near-budget opus implementer must resolve to sonnet; got %q", got)
	}
	joined := strings.Join(logs, "\n")
	for _, want := range []string{"near budget", "0.85", "opus", "sonnet", "downtiering"} {
		if !strings.Contains(joined, want) {
			t.Errorf("down-tier log must name %q (honest trade-off); log=%q", want, joined)
		}
	}

	// (b) In budget -> the SAME phase emits NO DOWN-TIER line (the PR6 invariant). A history
	// observability line (PR2) may be present — it never down-tiers — so we assert specifically
	// that no `downtiering` line appears, not that the log is wholly empty.
	var quiet []string
	inBudget := phaseTierResolver("balanced", func() float64 { return 0.50 }, nil, func(s string) { quiet = append(quiet, s) })
	if got := inBudget(phase); got != routing.Opus || downTierLines(quiet) != 0 {
		t.Errorf("in-budget resolve must be opus and emit NO down-tier line; got tier=%q, down-tier lines=%d (%v)", inBudget(phase), downTierLines(quiet), quiet)
	}

	// (c) Floor agent near budget -> exempt, so again NO down-tier line happens (the PR6
	// invariant). As in (b) a PR2 history line is allowed; only a `downtiering` line is forbidden.
	var floorLog []string
	floor := phaseTierResolver("balanced", func() float64 { return 0.95 }, nil, func(s string) { floorLog = append(floorLog, s) })
	if got := floor(asset.Phase{Name: "review", Agent: "reviewer"}); got != routing.Opus || downTierLines(floorLog) != 0 {
		t.Errorf("floor agent near budget must stay opus and emit NO down-tier line; got tier=%q, down-tier lines=%d", got, downTierLines(floorLog))
	}
}

// downTierLines counts the resolver's DOWN-TIER log lines (the `downtiering` marker), so a
// test can pin the PR6 down-tier-silence invariant independently of the PR2 history line that
// the same resolver also emits (history never down-tiers; the two concerns must not conflate).
func downTierLines(logs []string) int {
	n := 0
	for _, l := range logs {
		if strings.Contains(l, "downtiering") {
			n++
		}
	}
	return n
}

// ── ⑤ phaseTierByName agrees with the Phase-keyed resolver (the cost-path face) ──────────
//
// The cost Observe seam resolves the tier by phase NAME (phaseTierByName); it must return the
// SAME tier the Phase-keyed tierOf does for the matching phase, and "" for an unknown name
// (omitempty drops it). This is the structural guarantee behind the drift-guard's stamp arm.
func TestBudgetTier_PhaseTierByNameMatchesResolver(t *testing.T) {
	phase := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: "opus"}
	wf := asset.Workflow{Phases: []asset.Phase{phase}}
	tierOf := phaseTierResolver("balanced", func() float64 { return 0.85 }, nil, nil)
	byName := phaseTierByName(wf, tierOf)

	if a, b := byName(phase.Name), tierOf(phase); a != b {
		t.Errorf("phaseTierByName(%q)=%q must equal tierOf(phase)=%q (name- and Phase-keyed agree)", phase.Name, a, b)
	}
	if got := byName("no-such-phase"); got != "" {
		t.Errorf("unknown phase name must yield \"\" (omitempty drops it); got %q", got)
	}
}
