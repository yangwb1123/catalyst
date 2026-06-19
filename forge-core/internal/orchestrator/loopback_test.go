package orchestrator

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
)

// This file isolates the DIRECTED GATE LOOP-BACK (on_fail) tests. They share the
// in-package helpers from orchestrator_test.go (recorder, contains, containsLine,
// allOK), and live in their own file so the orchestrator test suite stays within
// the harness's max_file_lines=500 structural cap.

// loopBackWorkflow mirrors build.yml's loop-back shape: planner(agent) →
// implementer(agent) → harness-gates(GATE, on_fail loop_back to implementer) →
// qa(agent). The harness gate's on_fail TARGET is implementer (index 1), NOT
// planner (index 0), so a loop-back re-runs implementer but must NOT re-run
// planner — that asymmetry is what proves the jump is DIRECTED to the named phase
// rather than a whole-workflow replay. qa is an AGENT phase here (no gates) so it
// surfaces in recorder.executed: its presence proves the run PROCEEDED past the
// recovered gate (a directed loop-back that healed), its absence an abort.
const loopBackWorkflow = `{
  "stage": "build",
  "phases": [
    {"name": "planner", "agent": "planner", "readonly": true, "required_gates": []},
    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
    {"name": "harness-gates", "agent": "harness", "readonly": true,
     "required_gates": ["lint", "test"],
     "on_fail": {"action": "loop_back", "target_phase": "implementer"}},
    {"name": "qa", "agent": "qa", "readonly": true, "required_gates": []}
  ],
  "stop_condition": {"type": "conjunction", "all_of": [], "anti_pattern": "round_count"}
}`

func loadLoopBack(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(loopBackWorkflow))
	if err != nil {
		t.Fatalf("load loop-back fixture: %v", err)
	}
	return wf
}

// flakyGate fails the named gate the first failUntil times it is asked to run it,
// then PASSES it — the fake "fail-then-pass" gate that lets a test prove the
// engine retried via a directed loop-back rather than aborting. Every other gate
// always passes. calls counts how many times the flaky gate was attempted.
type flakyGate struct {
	name      string
	failUntil int
	calls     int
}

func (g *flakyGate) run(name string) gate.Result {
	if name != g.name {
		return gate.Result{Name: name, OK: true}
	}
	g.calls++
	ok := g.calls > g.failUntil
	return gate.Result{Name: name, OK: ok, Output: "flaky"}
}

// ★ THE proof of DIRECTED loop-back ★: the harness "test" gate fails twice then
// passes, and the harness phase declares on_fail loop_back to implementer. With a
// MaxLoopBack budget of 3, the engine must JUMP BACK to implementer (re-running
// implementer→harness) twice, then on the third harness attempt the gate passes
// and the run continues to qa — NOT abort, and NOT a whole-workflow replay
// (planner runs exactly once; implementer runs 1 + 2 loop-backs = 3 times).
func TestRun_DirectedLoopBackOnGateFail(t *testing.T) {
	wf := loadLoopBack(t)
	rec := &recorder{}
	fg := &flakyGate{name: "test", failUntil: 2}
	eng := Engine{Exec: rec.executor(), RunGate: fg.run, Log: rec.log, MaxLoopBack: 3}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run should recover via directed loop-back and complete; got %v", err)
	}
	// Executed agent phases, in order: planner once (NOT replayed — proves the
	// jump targets implementer, not phase 0), implementer three times (initial +
	// two loop-backs), then qa once after the gate finally passes.
	want := []string{"planner", "implementer", "implementer", "implementer", "qa"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("directed loop-back executed %v, want %v", rec.executed, want)
	}
	// The flaky gate was attempted three times (two FAILs + one PASS).
	if fg.calls != 3 {
		t.Errorf("flaky gate attempts = %d, want 3 (2 fail + 1 pass)", fg.calls)
	}
	// And each loop-back was logged as a directed jump to the target phase.
	if !containsLine(rec.logs, "loop-back 1/3 to implementer") || !containsLine(rec.logs, "loop-back 2/3 to implementer") {
		t.Errorf("each loop-back must log the directed jump to implementer; logs=%v", rec.logs)
	}
	// The completed run reports the stop condition (it did not abort).
	if !containsLine(rec.logs, "stop: condition declared") {
		t.Errorf("a recovered run must complete and report stop; logs=%v", rec.logs)
	}
}

// Budget exhaustion is FAIL-CLOSED: a gate that fails MORE times than the
// loop-back budget allows must, after the budget is spent, ABORT — never loop
// forever and never silently pass. Here the gate fails 5 times but the budget is
// only 2, so after 2 directed loop-backs (3 gate attempts) the still-red gate
// aborts; qa never runs.
func TestRun_LoopBackBudgetExhaustedAborts(t *testing.T) {
	wf := loadLoopBack(t)
	rec := &recorder{}
	fg := &flakyGate{name: "test", failUntil: 5}
	eng := Engine{Exec: rec.executor(), RunGate: fg.run, Log: rec.log, MaxLoopBack: 2}

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("budget exhaustion must abort (fail-closed), not loop forever or pass")
	}
	if !strings.Contains(err.Error(), "harness-gates") || !strings.Contains(err.Error(), "test") {
		t.Errorf("abort error should name the failing phase and gate; got %v", err)
	}
	// 1 initial attempt + 2 loop-backs = 3 gate attempts, then abort.
	if fg.calls != 3 {
		t.Errorf("flaky gate attempts = %d, want 3 (initial + 2 loop-backs, then abort)", fg.calls)
	}
	if contains(rec.executed, "qa") {
		t.Errorf("qa must NOT run after a budget-exhausted abort; executed=%v", rec.executed)
	}
	if !containsLine(rec.logs, "still red after 2/2 loop-backs") {
		t.Errorf("exhaustion must log the fail-closed reason; logs=%v", rec.logs)
	}
}

// BACK-COMPAT: a gate phase with NO on_fail must abort on the first red gate
// regardless of a non-zero MaxLoopBack — directed loop-back is opt-in on the asset
// side. Here the SAME flaky gate sits on a gate phase carrying no on_fail, so the
// first FAIL aborts and implementer is never re-run.
func TestRun_NoOnFailStillAbortsBackCompat(t *testing.T) {
	// A loop-back fixture with the on_fail stripped from the gate phase.
	const noOnFail = `{
	  "stage": "build",
	  "phases": [
	    {"name": "planner", "agent": "planner", "readonly": true, "required_gates": []},
	    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
	    {"name": "harness-gates", "agent": "harness", "readonly": true, "required_gates": ["test"]},
	    {"name": "qa", "agent": "qa", "readonly": true, "required_gates": ["test"]}
	  ],
	  "stop_condition": {"type": "conjunction", "all_of": [], "anti_pattern": "round_count"}
	}`
	wf, err := asset.LoadWorkflowJSON([]byte(noOnFail))
	if err != nil {
		t.Fatalf("load no-on_fail fixture: %v", err)
	}
	rec := &recorder{}
	fg := &flakyGate{name: "test", failUntil: 2} // would pass on the 3rd attempt IF it looped
	eng := Engine{Exec: rec.executor(), RunGate: fg.run, Log: rec.log, MaxLoopBack: 3}

	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("a gate phase with no on_fail must abort on the first red gate (back-compat)")
	}
	if fg.calls != 1 {
		t.Errorf("no on_fail must attempt the gate exactly once (no loop-back); got %d", fg.calls)
	}
	if contains(rec.executed, "qa") {
		t.Errorf("qa must not run after a no-on_fail abort; executed=%v", rec.executed)
	}
	// implementer ran once (initial) and was NOT re-run.
	if got := strings.Count(strings.Join(rec.executed, ","), "implementer"); got != 1 {
		t.Errorf("implementer must run once (no loop-back) with no on_fail; ran %d times", got)
	}
	if containsLine(rec.logs, "loop-back") {
		t.Errorf("no on_fail must not log any loop-back; logs=%v", rec.logs)
	}
}

// Fail-closed on a zero budget: even WITH on_fail, the default MaxLoopBack of 0
// means zero loop-backs are permitted, so the first red gate aborts — the engine
// side of the opt-in. This pins that a workflow declaring on_fail is still
// byte-for-byte the legacy abort until the operator raises the budget.
func TestRun_OnFailWithZeroBudgetAbortsBackCompat(t *testing.T) {
	wf := loadLoopBack(t)
	rec := &recorder{}
	fg := &flakyGate{name: "test", failUntil: 1}
	eng := Engine{Exec: rec.executor(), RunGate: fg.run, Log: rec.log} // MaxLoopBack defaults to 0

	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("on_fail with a zero loop-back budget must still abort on the first red gate")
	}
	if fg.calls != 1 {
		t.Errorf("zero budget must attempt the gate once then abort; got %d", fg.calls)
	}
	if contains(rec.executed, "qa") {
		t.Errorf("qa must not run when the zero-budget run aborts; executed=%v", rec.executed)
	}
}

// An on_fail whose target_phase names a non-existent phase is unresolvable, so the
// engine cannot jump — it must abort (fail-closed), not silently pass or panic.
func TestRun_LoopBackUnresolvableTargetAborts(t *testing.T) {
	const badTarget = `{
	  "stage": "build",
	  "phases": [
	    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
	    {"name": "harness-gates", "agent": "harness", "readonly": true, "required_gates": ["test"],
	     "on_fail": {"action": "loop_back", "target_phase": "nonexistent"}}
	  ],
	  "stop_condition": {"type": "external", "all_of": []}
	}`
	wf, err := asset.LoadWorkflowJSON([]byte(badTarget))
	if err != nil {
		t.Fatalf("load bad-target fixture: %v", err)
	}
	rec := &recorder{}
	fg := &flakyGate{name: "test", failUntil: 1}
	eng := Engine{Exec: rec.executor(), RunGate: fg.run, Log: rec.log, MaxLoopBack: 3}

	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("an unresolvable on_fail target must abort (fail-closed)")
	}
	if !containsLine(rec.logs, "not found") {
		t.Errorf("an unresolvable target must log the honest reason; logs=%v", rec.logs)
	}
}
