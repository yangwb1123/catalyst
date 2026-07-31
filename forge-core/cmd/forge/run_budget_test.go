package main

import (
	"flag"
	"math"
	"testing"
	"time"
)

// run_budget_test.go pins the DOLLAR half of the run-level budget cap (cost.go's
// runBudget) — the cmd/forge side that owns ALL money arithmetic and yields the opaque
// bool the orchestrator consumes. The engine-side stop behavior (a pure-bool puller
// stopping RunFrom before the next spawn, the stash proof that nil = no cap) lives in
// internal/orchestrator/budget_test.go; here we prove the accumulator sums real cost,
// crosses its cap exactly, survives ACROSS evolve iterations (never reset), and that an
// unset / malformed flag behaves correctly (no-op back-compat / fail-closed parse).

// ① The accumulator SUMS every fed phase cost (the cumulative run total). feed wraps a
// downstream sink; driving it with several phase costs must leave spent == the sum, and
// must forward each call to the inner sink unchanged (so the trace still gets every event).
func TestRunBudget_FeedAccumulatesSumAndForwards(t *testing.T) {
	b := &runBudget{} // unset cap: pure accumulator
	var forwarded []float64
	sink := b.feed(func(_, _ string, usd float64, _ time.Duration) { forwarded = append(forwarded, usd) })

	costs := []float64{0.05, 0.18, 0.0044035, 0.12}
	var want float64
	for _, c := range costs {
		sink("implementer", "opus", c, 0)
		want += c
	}
	if !approx(b.spent, want) {
		t.Errorf("accumulated spent = %v, want sum %v", b.spent, want)
	}
	if len(forwarded) != len(costs) {
		t.Fatalf("feed must forward every call to the inner sink; forwarded %d of %d", len(forwarded), len(costs))
	}
	for i, c := range costs {
		if !approx(forwarded[i], c) {
			t.Errorf("forwarded[%d] = %v, want %v (feed must not mutate the cost)", i, forwarded[i], c)
		}
	}
}

// ② exhausted crosses EXACTLY at the cap: false strictly below, true at-or-above. A cap of
// 0.20 stays open at 0.19, trips at 0.20, stays tripped past it. This is the dollar
// comparison the engine never sees — only the bool it produces.
func TestRunBudget_ExhaustedCrossesAtCap(t *testing.T) {
	b := &runBudget{cap: 0.20}
	feed := b.feed(nil)

	feed("p1", "opus", 0.19, 0)
	if b.exhausted() {
		t.Fatalf("0.19 < cap 0.20 must NOT be exhausted (spent=%v)", b.spent)
	}
	feed("p2", "opus", 0.01, 0) // -> 0.20, exactly the cap
	if !b.exhausted() {
		t.Fatalf("0.20 >= cap 0.20 must be exhausted (spent=%v)", b.spent)
	}
	feed("p3", "opus", 0.50, 0) // well over: still exhausted
	if !b.exhausted() {
		t.Fatalf("past the cap must stay exhausted (spent=%v)", b.spent)
	}
}

// ③ CROSS-ITERATION accumulation (the evolve invariant): the run budget is created ONCE
// and reused for the whole loop, so spend from iteration 1 PLUS iteration 2 accumulates
// into the SAME total — it is NOT reset per iteration. Neither iteration alone trips a
// 0.30 cap (each spends 0.18), but their SUM (0.36) does. This models exactly how buildLoop
// threads one runBudget into the single reused Engine: the cap bounds the WHOLE run.
func TestRunBudget_AccumulatesAcrossIterations(t *testing.T) {
	b := &runBudget{cap: 0.30}
	feed := b.feed(nil)

	// Iteration 1: one billed phase at 0.18. Under the 0.30 cap on its own.
	feed("implementer", "opus", 0.18, 0)
	if b.exhausted() {
		t.Fatalf("iteration 1 alone (0.18) must not trip the 0.30 cap (spent=%v)", b.spent)
	}
	// Iteration 2 reuses the SAME accumulator (no reset): another 0.18 -> 0.36 total.
	feed("implementer", "opus", 0.18, 0)
	if !b.exhausted() {
		t.Fatalf("iter1+iter2 (0.36) must trip the 0.30 cap — the accumulator must NOT reset per iteration (spent=%v)", b.spent)
	}
	if !approx(b.spent, 0.36) {
		t.Errorf("cross-iteration total = %v, want 0.36 (sum of both iterations)", b.spent)
	}
}

// ④ BudgetExhaustedFunc returns the puller ONLY when a positive cap is set; an unset cap
// yields nil (the back-compat signal "no run-level budget" the engine reads as "never
// consult"). A set cap returns a working closure that reflects the live spend.
func TestRunBudget_ExhaustedFuncNilWhenUnset(t *testing.T) {
	if (&runBudget{}).BudgetExhaustedFunc() != nil {
		t.Error("an unset cap must yield a NIL puller (no run-level budget -> engine unchanged)")
	}
	b := &runBudget{cap: 0.10}
	f := b.BudgetExhaustedFunc()
	if f == nil {
		t.Fatal("a set cap must yield a non-nil puller")
	}
	if f() {
		t.Error("puller must read false before any spend")
	}
	b.feed(nil)("p", "opus", 0.10, 0)
	if !f() {
		t.Error("puller must read true once spend reaches the cap")
	}
}

// ⑤ newRunBudget: empty flag -> unset no-op accumulator (cap 0, nil puller); a valid
// number -> the canonical persisted micro-dollar cap; malformed, negative, sub-micro,
// or unrepresentably large values -> a hard error (fail-closed: a budget the operator
// set must never be silently dropped or change meaning on resume).
func TestNewRunBudget_ParsesAndFailsClosed(t *testing.T) {
	// Empty = unset.
	b, err := newRunBudget("")
	if err != nil {
		t.Fatalf("empty flag must be unset, not an error; got %v", err)
	}
	if b.cap != 0 || b.BudgetExhaustedFunc() != nil {
		t.Errorf("empty flag must yield an unset (cap 0, nil puller) accumulator; got cap=%v", b.cap)
	}

	// Valid number = that cap (whitespace tolerated).
	b, err = newRunBudget("  2.50 ")
	if err != nil {
		t.Fatalf("a valid dollar amount must parse; got %v", err)
	}
	if !approx(b.cap, 2.50) {
		t.Errorf("parsed cap = %v, want 2.50", b.cap)
	}

	// More precision than checkpoint v3 supports is rounded exactly once at
	// parse time, so first-run enforcement and resumed enforcement agree.
	b, err = newRunBudget("1.0000004")
	if err != nil {
		t.Fatalf("a representable high-precision cap must parse; got %v", err)
	}
	if b.cap != 1 || b.CapUsdMicros() != 1_000_000 {
		t.Errorf("canonical cap = %v (%d micro-USD), want 1.0 (1000000)", b.cap, b.CapUsdMicros())
	}

	// Malformed / negative / non-finite / non-persistable = fail-closed error.
	for _, bad := range []string{
		"abc", "-1", "1.2.3", "NaN", "Inf",
		"0.0000004",          // positive but rounds to the persisted "unset" sentinel
		"9223372036854.7758", // would overflow the checkpoint int64 micro-USD field
	} {
		if _, err := newRunBudget(bad); err == nil {
			t.Errorf("newRunBudget(%q) must fail closed, not silently drop the cap", bad)
		}
	}
}

func TestRunBudget_LargeCanonicalMicrosRemainStableAcrossResume(t *testing.T) {
	for _, input := range []string{
		"4503599627.360497", // immediately above float64's exact integer range
		"9223372036854",     // near the int64 micro-dollar persistence ceiling
	} {
		t.Run(input, func(t *testing.T) {
			configured, err := newRunBudget(input)
			if err != nil {
				t.Fatal(err)
			}
			wantCap := configured.CapUsdMicros()
			if wantCap <= 1<<51 {
				t.Fatalf("test cap %d does not exercise large integer precision", wantCap)
			}
			wantSpent := wantCap - 1
			for cycle := 0; cycle < 5; cycle++ {
				resumed := &runBudget{}
				if err := resumed.restore(wantCap, wantSpent); err != nil {
					t.Fatalf("restore cycle %d: %v", cycle, err)
				}
				if got := resumed.CapUsdMicros(); got != wantCap {
					t.Fatalf("cycle %d cap drifted to %d, want %d", cycle, got, wantCap)
				}
				if got := resumed.SpentUsdMicros(); got != wantSpent {
					t.Fatalf("cycle %d spend drifted to %d, want %d", cycle, got, wantSpent)
				}
			}
		})
	}
}

func TestRunBudget_UnrepresentableSpendSaturatesFailClosed(t *testing.T) {
	b, err := newRunBudget("1")
	if err != nil {
		t.Fatal(err)
	}
	b.feed(nil)("phase", "model", 1e308, 0)
	if got := b.SpentUsdMicros(); got != math.MaxInt64 {
		t.Fatalf("huge finite bill persisted as %d, want saturated MaxInt64", got)
	}
	if !b.exhausted() {
		t.Fatal("huge finite bill must exhaust a representable cap")
	}

	resumed := &runBudget{}
	if err := resumed.restore(b.CapUsdMicros(), b.SpentUsdMicros()); err != nil {
		t.Fatalf("restore saturated spend: %v", err)
	}
	if resumed.SpentUsdMicros() != math.MaxInt64 || !resumed.exhausted() {
		t.Fatalf("saturated spend did not remain fail-closed after resume: micros=%d exhausted=%v",
			resumed.SpentUsdMicros(), resumed.exhausted())
	}
}

// ⑥ END TO END through the claude cost parser: a real claude JSON envelope fed through the
// SAME observeFor cost path production uses, with the sink wrapped by feed, must land the
// parsed total_cost_usd in the run total — proving parse -> accumulate is connected, not
// just the arithmetic in isolation. Two claude phases accumulate to the sum of their costs.
func TestRunBudget_AccumulatesParsedClaudeCost(t *testing.T) {
	b := &runBudget{cap: 0.10} // realClaudeJSON bills 0.0544035; two phases exceed this
	// observeFor with only the cost concern live (isClaude=true, other ledgers nil); the
	// cost sink is wrapped by feed exactly as buildRunEngine wires it.
	sink := b.feed(func(_, _ string, _ float64, _ time.Duration) {})
	observe := observeFor(true, sink, func(string) string { return "opus" }, nil, nil, nil, nil, nil)
	if observe == nil {
		t.Fatal("observeFor must return a live sink when the cost concern is active")
	}

	observe("implementer", realClaudeJSON, 0) // 0.0544035 — under the 0.10 cap alone
	if b.exhausted() {
		t.Fatalf("one claude phase (0.0544035) must not trip the 0.10 cap (spent=%v)", b.spent)
	}
	observe("implementer", realClaudeJSON, 0) // -> 0.108807, over the cap
	if !b.exhausted() {
		t.Fatalf("two claude phases (~0.1089) must trip the 0.10 cap (spent=%v)", b.spent)
	}
	// A non-claude / non-cost output must NOT move the total (no fabricated cost).
	before := b.spent
	observe("implementer", "plain echo output, no JSON envelope", 0)
	if !approx(b.spent, before) {
		t.Errorf("a non-cost output must not change the run total; spent moved %v -> %v", before, b.spent)
	}
}

// ⑦ The flag is wired into usage and bindRunOpts as --run-budget-usd, DISTINCT from the
// per-call --agent-max-budget-usd — a quick guard that the new run-level flag is actually
// reachable and described (so the two budgets are not confused at the CLI).
func TestRunBudget_FlagIsBoundAndDistinct(t *testing.T) {
	var o runOpts
	fs := flag.NewFlagSet("test", flag.ContinueOnError)
	bindRunOpts(fs, &o)
	if fs.Lookup("run-budget-usd") == nil {
		t.Fatal("bindRunOpts must register --run-budget-usd")
	}
	if fs.Lookup("agent-max-budget-usd") == nil {
		t.Fatal("the per-call --agent-max-budget-usd must remain (the two are distinct bounds)")
	}
	if err := fs.Parse([]string{"--run-budget-usd", "3.00"}); err != nil {
		t.Fatalf("parsing --run-budget-usd must succeed; got %v", err)
	}
	if o.runBudgetUSD != "3.00" {
		t.Errorf("--run-budget-usd must bind into runOpts.runBudgetUSD; got %q", o.runBudgetUSD)
	}
}

// ⑧ seed <-> SpentUsdMicros is the persistence boundary (the only dollar<->micro crossing for
// the checkpoint). SpentUsdMicros reports the cumulative spend as rounded micro-dollars; seed sets
// the base from a persisted micro total; and a subsequent feed ACCUMULATES on top of the seed (not
// from 0). A zero/negative seed is a no-op — an unbudgeted or never-billed resume stays at 0.
func TestRunBudget_SeedAndSpentMicros(t *testing.T) {
	b := &runBudget{cap: 1.00}

	// A never-billed budget reports 0 micros, and the checkpoint omits the (omitempty) key.
	if got := b.SpentUsdMicros(); got != 0 {
		t.Errorf("unbilled SpentUsdMicros = %d, want 0", got)
	}
	// SpentUsdMicros mirrors costEmitter's usd*1e6 rounding: 0.054403 -> 54403 (jitter-free,
	// chosen to land on an exact integer micro so the round is unambiguous).
	b.feed(nil)("p", "opus", 0.054403, 0)
	if got := b.SpentUsdMicros(); got != 54403 {
		t.Errorf("SpentUsdMicros after 0.054403 = %d, want 54403 (rounded micro-dollars)", got)
	}

	// seed SETS the base from a persisted micro total (the resume path): 250000µ$ = $0.25.
	fresh := &runBudget{cap: 1.00}
	fresh.seed(250_000)
	if !approx(fresh.spent, 0.25) {
		t.Errorf("after seed(250000) spent = %v, want 0.25", fresh.spent)
	}
	if got := fresh.SpentUsdMicros(); got != 250_000 {
		t.Errorf("SpentUsdMicros after seed = %d, want 250000 (round-trips the seed)", got)
	}
	// A later feed accumulates ON TOP of the seed, not from zero: 0.25 + 0.10 = 0.35.
	fresh.feed(nil)("p", "opus", 0.10, 0)
	if !approx(fresh.spent, 0.35) {
		t.Errorf("feed after seed = %v, want 0.35 (accumulates on the seeded base)", fresh.spent)
	}

	// Zero / negative seed is a no-op (unbudgeted or fresh resume): spend stays untouched.
	noop := &runBudget{cap: 1.00}
	noop.seed(0)
	noop.seed(-5)
	if noop.spent != 0 {
		t.Errorf("seed(0)/seed(-5) must be a no-op; spent = %v, want 0", noop.spent)
	}
}

// ⑨ ★THE CROSS-RESUME CORRECTNESS TEST★ — the gap PR5 closes. A crash mid-evolve plus --resume
// builds a FRESH runBudget at spent=0. Without re-seeding from the checkpoint, the cost billed
// before the crash escapes the cap and the run overspends past it; seeding restores the pre-crash
// cumulative so the cap keeps metering the WHOLE run. We model the crash by persisting the spend
// into a Checkpoint (the real channel) and resuming through a brand-new budget.
//
// STASH PROOF (the gap is real, not hypothetical): the same scenario WITHOUT seed needs much more
// post-resume spend before the cap trips — i.e. it overspends. Seed vs no-seed is the whole fix.
func TestRunBudget_CrossResumeSeedEnforcesCap(t *testing.T) {
	const cap = 1.00
	const preCrash = 0.90 // billed in iterations 1..N before the crash

	// --- Pre-crash run: spend 0.90 under a $1.00 cap, then "crash". The loop checkpoints the
	// cumulative spend as micro-dollars (exactly what checkpointHook writes).
	pre := &runBudget{cap: cap}
	pre.feed(nil)("implementer", "opus", preCrash, 0)
	if pre.exhausted() {
		t.Fatalf("pre-crash 0.90 < cap 1.00 must not be exhausted (spent=%v)", pre.spent)
	}
	persistedMicros := pre.SpentUsdMicros() // 900000 — the value the checkpoint carries

	// --- FIXED behavior: resume SEEDS a fresh budget from the checkpoint. Now just $0.10 more
	// trips the $1.00 cap (0.90 + 0.10), because the pre-crash spend still counts.
	seeded := &runBudget{cap: cap}
	seeded.seed(persistedMicros) // <- the PR5 fix: resume re-seeds the cumulative
	seeded.feed(nil)("implementer", "opus", 0.10, 0)
	if !seeded.exhausted() {
		t.Fatalf("SEEDED resume: 0.90 (pre-crash) + 0.10 = 1.00 must trip the cap — the run-level "+
			"bound must survive --resume (spent=%v)", seeded.spent)
	}

	// --- STASH (the pre-PR bug): WITHOUT seed the resumed budget starts at $0. The SAME $0.10
	// does NOT trip the cap — the run sails past its real total and overspends. This is the gap.
	unseeded := &runBudget{cap: cap}
	// (no seed — the crashed-and-restarted budget the bug produced)
	unseeded.feed(nil)("implementer", "opus", 0.10, 0)
	if unseeded.exhausted() {
		t.Fatalf("STASH-PROOF unexpectedly tripped: without seed the cap should NOT trip at 0.10 "+
			"alone — that it doesn't is the overspend bug (spent=%v)", unseeded.spent)
	}
	// Concretely: the unseeded run keeps spending and only trips after ANOTHER ~0.90 — i.e. it
	// overspends by the entire pre-crash amount before the cap finally bites. That extra slack
	// IS the gap: total real spend reaches ~1.90 against a $1.00 cap.
	for !unseeded.exhausted() {
		unseeded.feed(nil)("implementer", "opus", 0.10, 0)
	}
	if unseeded.spent <= seeded.spent {
		t.Fatalf("the unseeded (buggy) run must overspend relative to the seeded run: unseeded total "+
			"%v should exceed the seeded cap-trip total %v (the pre-crash spend it ignored)", unseeded.spent, seeded.spent)
	}
	t.Logf("gap quantified: seeded trips at %.2f (correct), unseeded only at %.2f (overspent by ~%.2f)",
		seeded.spent, unseeded.spent, unseeded.spent-seeded.spent)
}

// ⑩ SpendRatio is the dimensionless spend/cap fraction the near-budget down-tier (PR6)
// consults — the ONLY place spent/cap is divided, the near-budget analogue of exhausted's
// at/over-cap bool. An unset (cap<=0) budget yields 0 (nothing to be "near" -> no adjustment,
// byte-identical to the unbudgeted path); a positive cap yields the live fraction; and after
// a seed (the resume path) the ratio reflects the SEEDED cumulative, so a resumed run near its
// cap is correctly seen as near-budget.
func TestRunBudget_SpendRatio(t *testing.T) {
	// Unset cap: ratio is 0 regardless of spend (an unbudgeted accumulator still tallies,
	// but with no cap there is no ratio -> 0, which sits below every down-tier gate).
	unset := &runBudget{} // cap 0
	if got := unset.SpendRatio(); got != 0 {
		t.Errorf("unset cap SpendRatio = %v, want 0 (no cap -> never near budget)", got)
	}
	unset.feed(nil)("p", "opus", 5.0, 0) // spend without a cap
	if got := unset.SpendRatio(); got != 0 {
		t.Errorf("unset cap stays 0 even after spend; got %v", got)
	}
	// A negative cap can never exist (newRunBudget rejects it) but SpendRatio must still be
	// total-safe: cap<=0 -> 0, no divide-by-zero.
	if got := (&runBudget{cap: 0, spent: 1}).SpendRatio(); got != 0 {
		t.Errorf("cap 0 with spend must be 0 (no divide-by-zero); got %v", got)
	}

	// Positive cap: the live fraction. 0.40 of a 1.00 cap = 0.40; 0.85 = near-budget band.
	b := &runBudget{cap: 1.00}
	b.feed(nil)("p", "opus", 0.40, 0)
	if got := b.SpendRatio(); !approx(got, 0.40) {
		t.Errorf("SpendRatio after 0.40/1.00 = %v, want 0.40", got)
	}
	b.feed(nil)("p", "opus", 0.45, 0) // -> 0.85 total
	if got := b.SpendRatio(); !approx(got, 0.85) {
		t.Errorf("SpendRatio after 0.85/1.00 = %v, want 0.85 (near-budget band)", got)
	}
	// Past the cap the ratio exceeds 1.0 (it is not clamped — PR4 hard-stop owns >=1.0).
	b.feed(nil)("p", "opus", 0.30, 0) // -> 1.15 total
	if got := b.SpendRatio(); !approx(got, 1.15) {
		t.Errorf("SpendRatio past the cap = %v, want 1.15 (unclamped)", got)
	}

	// After a SEED (the --resume path, PR5): the ratio reflects the seeded cumulative, so a
	// resumed run that was already near its cap is seen as near-budget immediately. 0.90 of
	// a 1.00 cap seeded from 900000µ$.
	seeded := &runBudget{cap: 1.00}
	seeded.seed(900_000) // $0.90
	if got := seeded.SpendRatio(); !approx(got, 0.90) {
		t.Errorf("SpendRatio after seed(900000)/cap 1.00 = %v, want 0.90 (resume sees near-budget)", got)
	}
}

// approx compares two dollar figures within a tiny epsilon (float sums are not exact).
func approx(a, b float64) bool {
	d := a - b
	if d < 0 {
		d = -d
	}
	return d < 1e-9
}
