package orchestrator

import (
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

func (c *countingExec) Execute(_ asset.Phase, _ string) error {
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
