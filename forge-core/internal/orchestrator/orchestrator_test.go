package orchestrator

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
)

// fixtureWorkflow is a self-contained workflow mirroring build.yml's shape: an
// agent phase, a gate phase carrying required gates, then a trailing agent
// phase. Built inline so this suite depends on no external file or sibling
// agent's output.
const fixtureWorkflow = `{
  "stage": "build",
  "phases": [
    {"name": "planner", "agent": "planner", "readonly": true, "required_gates": []},
    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
    {"name": "harness-gates", "agent": "harness", "readonly": true,
     "required_gates": ["lint", "test", "build"]},
    {"name": "qa", "agent": "qa", "readonly": true, "required_gates": ["test"]}
  ],
  "stop_condition": {"type": "conjunction", "all_of": [{"metric": "roadmap_completion", "operator": "==", "threshold": 100}, {"metric": "gates_status", "operator": "==", "value": "green"}], "anti_pattern": "round_count"}
}`

func loadFixture(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(fixtureWorkflow))
	if err != nil {
		t.Fatalf("load fixture: %v", err)
	}
	return wf
}

// recorder captures log lines and which phases an executor actually ran.
type recorder struct {
	logs     []string
	executed []string
}

func (r *recorder) log(s string) { r.logs = append(r.logs, s) }

func (r *recorder) executor() AgentExecutor {
	return execFunc(func(p asset.Phase, _ string) error {
		r.executed = append(r.executed, p.Name)
		return nil
	})
}

// execFunc adapts a function to the AgentExecutor interface for tests.
type execFunc func(asset.Phase, string) error

func (f execFunc) Execute(p asset.Phase, mode string) error { return f(p, mode) }

// allOK is a RunGate that passes every gate.
func allOK(name string) gate.Result { return gate.Result{Name: name, OK: true} }

func TestRun_AllGatesOK(t *testing.T) {
	wf := loadFixture(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run returned error on all-OK gates: %v", err)
	}

	// Only the two non-gate phases run the executor; gate phases do not.
	want := []string{"planner", "implementer"}
	if strings.Join(rec.executed, ",") != strings.Join(want, ",") {
		t.Errorf("executed = %v, want %v", rec.executed, want)
	}
	if !containsLine(rec.logs, "stop: condition declared") {
		t.Errorf("expected stop condition to be reported; logs=%v", rec.logs)
	}
	// P11 honesty: the stale "not evaluated live (scaffold)" claim is gone;
	// convergence IS evaluated live by converge.
	if containsLine(rec.logs, "not evaluated live") || containsLine(rec.logs, "scaffold") {
		t.Errorf("stop report must not claim it is unevaluated scaffold; logs=%v", rec.logs)
	}
	if !containsLine(rec.logs, "evaluated live by converge") {
		t.Errorf("stop report should state it is evaluated live; logs=%v", rec.logs)
	}
}

func TestRun_GateFailureStops(t *testing.T) {
	wf := loadFixture(t)
	rec := &recorder{}
	// Fail exactly the "test" gate; "lint" and "build" pass.
	failTest := func(name string) gate.Result {
		return gate.Result{Name: name, OK: name != "test", Output: "boom"}
	}
	eng := Engine{Exec: rec.executor(), RunGate: failTest, Log: rec.log}

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("Run should return an error when a required gate is not OK")
	}
	if !strings.Contains(err.Error(), "harness-gates") || !strings.Contains(err.Error(), "test") {
		t.Errorf("error should name the phase and gate; got %v", err)
	}

	// Enforcement: it must stop AT the gate phase. The trailing qa phase, which
	// would have run the executor, must never execute.
	if contains(rec.executed, "qa") {
		t.Errorf("qa ran after a failed gate; executed=%v", rec.executed)
	}
	// And the stop condition must NOT be reported on an aborted run.
	if containsLine(rec.logs, "stop: condition declared") {
		t.Errorf("stop should not be reported on abort; logs=%v", rec.logs)
	}
}

func TestRun_NilGateRunnerFailsClosed(t *testing.T) {
	wf := loadFixture(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: nil, Log: rec.log}

	if err := eng.Run(wf, "balanced"); err == nil {
		t.Error("a nil gate runner must fail closed, not pass silently")
	}
}

// P22: a workflow that reaches an agent phase with no executor must fail closed
// with an error — never a nil-pointer panic.
func TestRun_NilExecutorFailsClosed(t *testing.T) {
	wf := loadFixture(t)
	eng := Engine{Exec: nil, RunGate: allOK}
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("nil executor must not panic; got %v", r)
		}
	}()
	if err := eng.Run(wf, "balanced"); err == nil {
		t.Error("a nil agent executor must return an error, not pass silently")
	}
}

func TestDryRunExecutor_LogsTier(t *testing.T) {
	rec := &recorder{}
	exec := DryRunExecutor{Log: rec.log}
	p := asset.Phase{Name: "reviewer", Agent: "reviewer"}
	if err := exec.Execute(p, "explorer"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	// reviewer is an Opus-floor agent even under explorer mode.
	if !containsLine(rec.logs, "phase reviewer -> agent reviewer (tier opus)") {
		t.Errorf("log = %v, want reviewer routed to opus", rec.logs)
	}
}

func contains(ss []string, want string) bool {
	for _, s := range ss {
		if s == want {
			return true
		}
	}
	return false
}

func containsLine(ss []string, substr string) bool {
	for _, s := range ss {
		if strings.Contains(s, substr) {
			return true
		}
	}
	return false
}
