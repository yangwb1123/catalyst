package orchestrator

import (
	"context"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

// This file isolates the per-run AGENT-CALL BUDGET (MaxAgentCalls / checkAgentBudget)
// tests. They share the in-package helpers from orchestrator_test.go (recorder,
// allOK, containsLine) and loopback_test.go (loopBackWorkflow, flakyGate), and live
// in their own file so each test suite stays within the harness's max_file_lines cap.
//
// The budget is the PAIRED PREREQUISITE to the recursion guard: recursion bounds
// DEPTH, this bounds the TOTAL count of agent-phase executions in one run — INCLUDING
// loop-back re-runs (each a real spawn). These tests pin: a positive ceiling refuses
// the over-budget spawn fail-closed (and the executor is never called for it),
// MaxAgentCalls=0 is unbounded back-compat, and a loop-back re-run is charged.

// countingExec is a fake AgentExecutor that always succeeds and counts how many
// times it was invoked — the spawn meter. The assertion "calls == N" then proves
// EXACTLY how many agent phases were actually spawned, so a refused (over-budget)
// phase is provable by its ABSENCE from the count.
type countingExec struct{ calls int }

func (c *countingExec) Execute(_ context.Context, _ asset.Phase, _ string) error {
	c.calls++
	return nil
}

// threeAgentWorkflow is three back-to-back AGENT phases with no gates, so RunFrom
// reaches the executor three times in a clean run — the minimal shape for proving a
// budget of 2 stops the 3rd spawn.
const threeAgentWorkflow = `{
  "stage": "build",
  "phases": [
    {"name": "planner", "agent": "planner", "readonly": true, "required_gates": []},
    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
    {"name": "qa", "agent": "qa", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

func loadThreeAgent(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(threeAgentWorkflow))
	if err != nil {
		t.Fatalf("load three-agent fixture: %v", err)
	}
	return wf
}

// ① Budget overrun fails closed and does NOT spawn the over-budget phase.
// MaxAgentCalls=2 against a 3-agent-phase workflow: planner + implementer charge the
// budget and run, but the 3rd (qa) is refused at checkAgentBudget BEFORE spawning.
// The proof is twofold — Run returns an error AND the counting executor was called
// EXACTLY 2 times (the 3rd phase never reached Execute).
func TestCheckAgentBudget_OverrunFailsClosedNoSpawn(t *testing.T) {
	wf := loadThreeAgent(t)
	rec := &recorder{}
	exec := &countingExec{}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log, MaxAgentCalls: 2}

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("Run must fail closed once the agent-call budget is exceeded")
	}
	if exec.calls != 2 {
		t.Errorf("executor calls = %d, want 2 (budget=2: the 3rd phase must NOT spawn)", exec.calls)
	}
	if !strings.Contains(err.Error(), "budget") {
		t.Errorf("error should name the agent-call budget; got %v", err)
	}
	if !containsLine(rec.logs, "agent-call budget exhausted") {
		t.Errorf("the refusal must be logged honestly; logs=%v", rec.logs)
	}
	// And it must abort: never report the stop condition on an over-budget run.
	if containsLine(rec.logs, "stop: condition declared") {
		t.Errorf("stop must not be reported on a budget-aborted run; logs=%v", rec.logs)
	}
}

// ② MaxAgentCalls=0 is UNBOUNDED — the back-compat guarantee. Every agent phase
// runs, no budget error, and the executor is called exactly once per phase.
func TestCheckAgentBudget_ZeroIsUnboundedBackCompat(t *testing.T) {
	wf := loadThreeAgent(t)
	rec := &recorder{}
	exec := &countingExec{}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log} // MaxAgentCalls defaults to 0

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("MaxAgentCalls=0 must impose no ceiling; got %v", err)
	}
	if exec.calls != 3 {
		t.Errorf("executor calls = %d, want 3 (one per agent phase, unbounded)", exec.calls)
	}
	if containsLine(rec.logs, "budget exhausted") {
		t.Errorf("an unbounded run must never log budget exhaustion; logs=%v", rec.logs)
	}
}

// ③ A LOOP-BACK re-run is CHARGED to the budget. Using loopBackWorkflow (planner →
// implementer → harness-gates[on_fail loop_back to implementer] → qa) with a flaky
// "test" gate that fails twice then passes and a MaxLoopBack of 3 (enough to recover):
// the clean path would spawn planner, implementer, then implementer twice more on the
// two loop-backs, then qa. With MaxAgentCalls=2 the budget is spent by planner(1) +
// implementer(2); when the gate first fails and the runtime jumps BACK to implementer,
// that loop-back re-run charges the budget (would be 3 > 2) and is REFUSED — proving
// loop-back re-runs count against the per-run total. The counting executor is called
// EXACTLY 2 times: the loop-back's re-spawn never happens, and the gate (failing at
// attempt 1) never reaches its later passing attempt.
func TestCheckAgentBudget_LoopBackRerunChargesBudget(t *testing.T) {
	wf := loadLoopBack(t)
	rec := &recorder{}
	exec := &countingExec{}
	fg := &flakyGate{name: "test", failUntil: 2}
	eng := Engine{Exec: exec, RunGate: fg.run, Log: rec.log, MaxLoopBack: 3, MaxAgentCalls: 2}

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("a loop-back re-run must consume the budget and, when it overruns, fail closed")
	}
	if exec.calls != 2 {
		t.Errorf("executor calls = %d, want 2 (planner + implementer; the loop-back re-spawn is over budget)", exec.calls)
	}
	if !strings.Contains(err.Error(), "budget") {
		t.Errorf("error should name the agent-call budget exhausted on the loop-back re-run; got %v", err)
	}
	// The loop-back was ATTEMPTED (the gate failed and the runtime jumped back), but
	// the budget refusal stopped the re-spawn: both events must be visible in the log.
	if !containsLine(rec.logs, "loop-back 1/3 to implementer") {
		t.Errorf("the directed loop-back must have been taken before the budget stopped the re-spawn; logs=%v", rec.logs)
	}
	if !containsLine(rec.logs, "agent-call budget exhausted") {
		t.Errorf("the over-budget loop-back re-run must be refused with an honest log; logs=%v", rec.logs)
	}
}

// ──────────────────────────────────────────────────────────────────────────────
// RUN-LEVEL BUDGET (BudgetExhausted puller / checkRunBudget) — the cumulative-resource
// sibling of the agent-call COUNT budget above. These pin the THIRD cost dimension: a
// caller-metered run-level cap the engine consults via an OPAQUE bool before each spawn.
//
// LAYERING: every test here drives the engine with a bare `func() bool` — NO dollars, NO
// model, NO vendor envelope ever crosses into the engine. That is the proof the engine
// stays unit-free: a plain bool is sufficient to make it stop, so it cannot be touching
// money. (The dollar arithmetic that PRODUCES the bool is cmd/forge's, tested there.)

// budgetGate is a fake run-level budget meter: exhausted flips true once `after` agent
// phases have been OBSERVED via tick (mimicking cmd/forge's accumulator crossing its cap
// after N billed phases). It returns ONLY a bool — the engine never sees how it decides.
type budgetGate struct {
	after int // become exhausted once this many phases have ticked
	seen  int // phases observed so far
}

func (b *budgetGate) exhausted() bool { return b.seen >= b.after }
func (b *budgetGate) tick()           { b.seen++ }

// ④ OVER BUDGET → RunFrom STOPS at the exhaustion point; later phases never spawn.
// A 3-agent workflow with a budget that is exhausted after the 2nd phase: planner and
// implementer run, but BEFORE qa is spawned the run-level guard sees the budget gone and
// stops fail-closed. Proof is threefold — Run returns the structured budget error, the
// counting executor was called EXACTLY 2 times (qa never reached Execute), and the honest
// "not a failure / budget" log fired. The executor TICKS the meter as it runs, so the
// budget crosses exactly as a real per-phase cost accumulator would.
func TestCheckRunBudget_OverBudgetStopsAndDoesNotSpawnLater(t *testing.T) {
	wf := loadThreeAgent(t)
	rec := &recorder{}
	bg := &budgetGate{after: 2}
	exec := &countingExec{}
	// Wrap the counting executor so each spawn also advances the budget meter — the
	// engine still sees only Execute()'s error and only the BudgetExhausted bool.
	tick := execFunc(func(ctx context.Context, p asset.Phase, m string) error {
		err := exec.Execute(context.Background(), p, m)
		bg.tick()
		return err
	})
	eng := Engine{Exec: tick, RunGate: allOK, Log: rec.log, BudgetExhausted: bg.exhausted}

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("Run must stop fail-closed once the run-level budget is exhausted")
	}
	if exec.calls != 2 {
		t.Errorf("executor calls = %d, want 2 (budget exhausted after 2: qa must NOT spawn)", exec.calls)
	}
	if !strings.Contains(err.Error(), "budget") {
		t.Errorf("error should name the run budget; got %v", err)
	}
	// The stop must read as a budget stop, not a crash, and name the completed count.
	if !containsLine(rec.logs, "run budget exhausted after 2 agent phase(s)") {
		t.Errorf("the run-level stop must be logged honestly with the completed count; logs=%v", rec.logs)
	}
	// And it aborts: the stop condition is never reported on a budget-stopped run.
	if containsLine(rec.logs, "stop: condition declared") {
		t.Errorf("stop must not be reported on a budget-stopped run; logs=%v", rec.logs)
	}
}

// ⑤ STASH (proves the GAP): the SAME workflow + SAME spend, but with NO BudgetExhausted
// puller wired (nil — the pre-PR4 world), runs ALL THREE phases to completion. This is the
// decisive before/after: without the run-level guard there is no cumulative cap, so a run
// burns through every phase no matter the total spend — exactly the gap PR4 closes.
func TestCheckRunBudget_NilPullerRunsAllPhases_StashProof(t *testing.T) {
	wf := loadThreeAgent(t)
	rec := &recorder{}
	exec := &countingExec{}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log} // BudgetExhausted nil: no cap

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("a nil run-budget puller must impose no cap; got %v", err)
	}
	if exec.calls != 3 {
		t.Errorf("executor calls = %d, want 3 (no run budget: every phase runs)", exec.calls)
	}
	if containsLine(rec.logs, "run budget exhausted") {
		t.Errorf("an unbudgeted run must never log a run-budget stop; logs=%v", rec.logs)
	}
}

// ⑥ UNDER BUDGET → all phases run. A puller that is NEVER exhausted (always false) must
// leave the run byte-for-byte identical to the no-cap path: all three phases spawn, no
// budget error, no budget log. This pins that a wired-but-unmet cap is inert.
func TestCheckRunBudget_UnderBudgetRunsAllPhases(t *testing.T) {
	wf := loadThreeAgent(t)
	rec := &recorder{}
	exec := &countingExec{}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log,
		BudgetExhausted: func() bool { return false }} // never exhausted

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("an unmet run budget must impose no ceiling; got %v", err)
	}
	if exec.calls != 3 {
		t.Errorf("executor calls = %d, want 3 (budget never exhausted)", exec.calls)
	}
	if containsLine(rec.logs, "run budget exhausted") {
		t.Errorf("an unmet budget must never log exhaustion; logs=%v", rec.logs)
	}
}

// ⑦ LAYERING: the engine stops on a CONSTANT `func() bool { return true }` — a pure bool
// carrying no dollar, model, or vendor data whatsoever. The very FIRST agent phase is
// refused (budget already gone before any spawn), proving the engine needs nothing but the
// bool to enforce a run-level cap: the dollar knowledge is entirely the caller's.
func TestCheckRunBudget_PureBoolDrivesEngine_NoDollars(t *testing.T) {
	wf := loadThreeAgent(t)
	rec := &recorder{}
	exec := &countingExec{}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log,
		BudgetExhausted: func() bool { return true }} // exhausted from the start

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("a constantly-exhausted budget must stop the run before the first spawn")
	}
	if exec.calls != 0 {
		t.Errorf("executor calls = %d, want 0 (budget exhausted before any spawn)", exec.calls)
	}
	if !containsLine(rec.logs, "run budget exhausted after 0 agent phase(s)") {
		t.Errorf("a pre-exhausted budget must stop at 0 completed phases; logs=%v", rec.logs)
	}
}
