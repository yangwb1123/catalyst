package main

import (
	"flag"
	"testing"
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
	sink := b.feed(func(_, _ string, usd float64) { forwarded = append(forwarded, usd) })

	costs := []float64{0.05, 0.18, 0.0044035, 0.12}
	var want float64
	for _, c := range costs {
		sink("implementer", "opus", c)
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

	feed("p1", "opus", 0.19)
	if b.exhausted() {
		t.Fatalf("0.19 < cap 0.20 must NOT be exhausted (spent=%v)", b.spent)
	}
	feed("p2", "opus", 0.01) // -> 0.20, exactly the cap
	if !b.exhausted() {
		t.Fatalf("0.20 >= cap 0.20 must be exhausted (spent=%v)", b.spent)
	}
	feed("p3", "opus", 0.50) // well over: still exhausted
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
	feed("implementer", "opus", 0.18)
	if b.exhausted() {
		t.Fatalf("iteration 1 alone (0.18) must not trip the 0.30 cap (spent=%v)", b.spent)
	}
	// Iteration 2 reuses the SAME accumulator (no reset): another 0.18 -> 0.36 total.
	feed("implementer", "opus", 0.18)
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
	b.feed(nil)("p", "opus", 0.10)
	if !f() {
		t.Error("puller must read true once spend reaches the cap")
	}
}

// ⑤ newRunBudget: empty flag -> unset no-op accumulator (cap 0, nil puller); a valid
// number -> that cap; a malformed or negative value -> a hard error (fail-closed: a budget
// the operator set must never be silently dropped).
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

	// Malformed / negative / non-finite = fail-closed error.
	for _, bad := range []string{"abc", "-1", "1.2.3", "NaN", "Inf"} {
		if _, err := newRunBudget(bad); err == nil {
			t.Errorf("newRunBudget(%q) must fail closed, not silently drop the cap", bad)
		}
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
	sink := b.feed(func(_, _ string, _ float64) {})
	observe := observeFor(true, sink, func(string) string { return "opus" }, nil, nil, nil, nil, nil)
	if observe == nil {
		t.Fatal("observeFor must return a live sink when the cost concern is active")
	}

	observe("implementer", realClaudeJSON) // 0.0544035 — under the 0.10 cap alone
	if b.exhausted() {
		t.Fatalf("one claude phase (0.0544035) must not trip the 0.10 cap (spent=%v)", b.spent)
	}
	observe("implementer", realClaudeJSON) // -> 0.108807, over the cap
	if !b.exhausted() {
		t.Fatalf("two claude phases (~0.1089) must trip the 0.10 cap (spent=%v)", b.spent)
	}
	// A non-claude / non-cost output must NOT move the total (no fabricated cost).
	before := b.spent
	observe("implementer", "plain echo output, no JSON envelope")
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

// approx compares two dollar figures within a tiny epsilon (float sums are not exact).
func approx(a, b float64) bool {
	d := a - b
	if d < 0 {
		d = -d
	}
	return d < 1e-9
}
