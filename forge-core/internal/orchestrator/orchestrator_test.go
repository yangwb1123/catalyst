package orchestrator

import (
	"errors"
	"strings"
	"testing"
	"time"

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

// OnPhase is the per-phase checkpoint hook: it must fire once per COMPLETED AGENT
// phase with that phase's workflow index — never for a gate phase (gates re-run
// idempotently on resume, so re-validating one is cheap; the cost worth saving is a
// re-spawned billed agent phase) and never for a mode-skipped phase. A nil OnPhase
// (every other test) is a no-op, so the per-iteration-only path stays unchanged.
func TestRunFrom_OnPhaseFiresPerCompletedAgentPhase(t *testing.T) {
	wf := loadFixture(t)
	rec := &recorder{}
	var fired []int
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log,
		OnPhase: func(i int) { fired = append(fired, i) }}
	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run: %v", err)
	}
	// Expected indices = the workflow positions of the phases the executor actually ran
	// (the agent phases); gate phases never reach the OnPhase call site.
	var want []int
	for _, name := range rec.executed {
		for i, p := range wf.Phases {
			if p.Name == name {
				want = append(want, i)
			}
		}
	}
	if len(fired) == 0 {
		t.Fatal("OnPhase never fired — the per-phase checkpoint hook is dead")
	}
	if len(fired) != len(want) {
		t.Fatalf("OnPhase fired %d times %v, want %d (the agent phases %v at %v)", len(fired), fired, len(want), rec.executed, want)
	}
	for k := range want {
		if fired[k] != want[k] {
			t.Errorf("OnPhase[%d] = %d, want %d (agent phase %q); fired=%v want=%v", k, fired[k], want[k], rec.executed[k], fired, want)
		}
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

// seqExecutor is a fake AgentExecutor that returns a scripted sequence of errors
// (one per call), counting how many times it was invoked. A nil entry is a
// success; running past the end of the script also returns nil (success), which
// is how the "succeeds on the Nth attempt" cases are expressed. It is the retry
// counterpart to recorder.executor (which always succeeds).
type seqExecutor struct {
	errs  []error
	calls int
}

func (s *seqExecutor) Execute(_ asset.Phase, _ string) error {
	i := s.calls
	s.calls++
	if i < len(s.errs) {
		return s.errs[i]
	}
	return nil
}

// agentOnlyWorkflow is a single agent phase with no gates, so Run reaches the
// executor immediately and the retry path is the only thing under test.
const agentOnlyWorkflow = `{
  "stage": "build",
  "phases": [{"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []}],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

func loadAgentOnly(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(agentOnlyWorkflow))
	if err != nil {
		t.Fatalf("load agent-only fixture: %v", err)
	}
	return wf
}

func timeoutErr() *ExecError { return &ExecError{Phase: "implementer", Kind: KindTimeout} }

func overloadedErr() *ExecError { return &ExecError{Phase: "implementer", Kind: KindOverloaded} }

// fakeSleep records every backoff duration the engine requests WITHOUT sleeping, so the
// exponential overload schedule is asserted in microseconds. Returned closure is wired to
// Engine.Sleep.
type fakeSleep struct{ durs []time.Duration }

func (f *fakeSleep) sleep(d time.Duration) { f.durs = append(f.durs, d) }

// 529/overload end-to-end: MaxRetries=3, the fake Exec returns KindOverloaded twice then
// succeeds. The run must succeed, the executor must be called 3 times, and the injected Sleep
// must have captured the EXPONENTIAL backoff sequence (2s, 4s) — proving an overload is retried
// AFTER a growing backoff, not in a tight loop. This is the decisive proof the resilience path
// is wired and bounded by the real schedule (mirrors direction-three's stash-style "prove it").
func TestRunAgentPhase_OverloadBacksOffThenSucceeds(t *testing.T) {
	wf := loadAgentOnly(t)
	rec := &recorder{}
	fs := &fakeSleep{}
	exec := &seqExecutor{errs: []error{overloadedErr(), overloadedErr()}} // 3rd call: nil
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log, MaxRetries: 3, Sleep: fs.sleep}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run should succeed once the overload clears; got %v", err)
	}
	if exec.calls != 3 {
		t.Errorf("executor calls = %d, want 3 (1 initial + 2 backed-off retries)", exec.calls)
	}
	want := []time.Duration{overloadBackoff(0), overloadBackoff(1)} // 2s, 4s
	if len(fs.durs) != len(want) {
		t.Fatalf("backoff count = %d (%v), want %d (%v)", len(fs.durs), fs.durs, len(want), want)
	}
	for i := range want {
		if fs.durs[i] != want[i] {
			t.Errorf("backoff[%d] = %s, want %s (exponential)", i, fs.durs[i], want[i])
		}
	}
	if !containsLine(rec.logs, "overloaded, backing off 2s before retry 1/3") {
		t.Errorf("each overload retry must log the backoff; logs=%v", rec.logs)
	}
}

// BOUNDED: a backend that stays overloaded past MaxRetries must ABORT — the backoff is charged
// against the retry budget, never an unbounded wait. Executor called exactly MaxRetries+1 times,
// the final error is the overload, and Sleep fired exactly MaxRetries times (one per retry taken).
func TestRunAgentPhase_OverloadExhaustsBudgetAndAborts(t *testing.T) {
	wf := loadAgentOnly(t)
	fs := &fakeSleep{}
	exec := &seqExecutor{errs: []error{overloadedErr(), overloadedErr(), overloadedErr(), overloadedErr()}}
	eng := Engine{Exec: exec, RunGate: allOK, Log: func(string) {}, MaxRetries: 2, Sleep: fs.sleep}

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("a persistently overloaded backend must abort once the retry budget is spent")
	}
	if exec.calls != 3 {
		t.Errorf("executor calls = %d, want 3 (1 initial + 2 retries, then give up)", exec.calls)
	}
	if len(fs.durs) != 2 {
		t.Errorf("backoff count = %d, want 2 (one per retry taken — bounded by MaxRetries)", len(fs.durs))
	}
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindOverloaded {
		t.Errorf("exhausted overload retries must return the last overload error; got %v", err)
	}
}

// HONEST DISTINCTION: a KindTimeout retry must NOT back off — it already burned its deadline, so
// it is re-attempted immediately. With only timeouts in the script, the injected Sleep must
// capture ZERO durations, proving the backoff is overload-specific (not a blanket sleep on every
// retryable kind).
func TestRunAgentPhase_TimeoutRetryDoesNotBackOff(t *testing.T) {
	wf := loadAgentOnly(t)
	fs := &fakeSleep{}
	exec := &seqExecutor{errs: []error{timeoutErr(), timeoutErr()}} // 3rd call: nil
	eng := Engine{Exec: exec, RunGate: allOK, Log: func(string) {}, MaxRetries: 2, Sleep: fs.sleep}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("timeout retries should still succeed; got %v", err)
	}
	if len(fs.durs) != 0 {
		t.Errorf("a timeout retry must NOT sleep; Sleep was called %d times (%v)", len(fs.durs), fs.durs)
	}
}

// MaxRetries=2 + two retryable timeouts then success => Run succeeds and the
// executor was called exactly 3 times (1 initial + 2 retries, the third clean).
func TestRunAgentPhase_RetriesThenSucceeds(t *testing.T) {
	wf := loadAgentOnly(t)
	rec := &recorder{}
	exec := &seqExecutor{errs: []error{timeoutErr(), timeoutErr()}} // 3rd call: nil
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log, MaxRetries: 2}

	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run should succeed once a retry clears the timeout; got %v", err)
	}
	if exec.calls != 3 {
		t.Errorf("executor calls = %d, want 3 (1 initial + 2 retries)", exec.calls)
	}
	if !containsLine(rec.logs, "retryable timeout, retry 1/2") || !containsLine(rec.logs, "retryable timeout, retry 2/2") {
		t.Errorf("each retry must be logged with kind + n/N; logs=%v", rec.logs)
	}
}

// MaxRetries=2 + always-retryable => Run fails returning the LAST error after the
// budget is spent; executor called exactly 3 times (1 initial + 2 retries).
func TestRunAgentPhase_ExhaustsRetriesAndFails(t *testing.T) {
	wf := loadAgentOnly(t)
	rec := &recorder{}
	exec := &seqExecutor{errs: []error{timeoutErr(), timeoutErr(), timeoutErr(), timeoutErr()}}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log, MaxRetries: 2}

	err := eng.Run(wf, "balanced")
	if err == nil {
		t.Fatal("Run must fail when every attempt times out and the budget is exhausted")
	}
	if exec.calls != 3 {
		t.Errorf("executor calls = %d, want 3 (1 initial + 2 retries, then give up)", exec.calls)
	}
	var execErr *ExecError
	if !errors.As(err, &execErr) || execErr.Kind != KindTimeout {
		t.Errorf("exhausted retries must return the last (timeout) error; got %v", err)
	}
}

// A non-retryable KindConfig failure must abort on the FIRST attempt regardless
// of a non-zero MaxRetries — retrying a permanent fault only burns turns.
func TestRunAgentPhase_NonRetryableAbortsImmediately(t *testing.T) {
	wf := loadAgentOnly(t)
	rec := &recorder{}
	exec := &seqExecutor{errs: []error{&ExecError{Phase: "implementer", Kind: KindConfig}}}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log, MaxRetries: 2}

	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("a KindConfig fault is permanent and must abort, not pass")
	}
	if exec.calls != 1 {
		t.Errorf("executor calls = %d, want 1 (non-retryable: no retries)", exec.calls)
	}
	if containsLine(rec.logs, "retry") {
		t.Errorf("a non-retryable error must not log a retry; logs=%v", rec.logs)
	}
}

// MaxRetries=0 (the default) + a retryable error must still abort on the first
// error and call the executor exactly once — the back-compat guarantee.
func TestRunAgentPhase_DefaultNoRetryIsBackCompat(t *testing.T) {
	wf := loadAgentOnly(t)
	rec := &recorder{}
	exec := &seqExecutor{errs: []error{timeoutErr(), timeoutErr()}}
	eng := Engine{Exec: exec, RunGate: allOK, Log: rec.log} // MaxRetries defaults to 0

	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("with MaxRetries=0 the first error must abort the run")
	}
	if exec.calls != 1 {
		t.Errorf("executor calls = %d, want 1 (MaxRetries=0 == no retries)", exec.calls)
	}
}

// OnGateResult must fire once per gate with its OBJECTIVE verdict — "ok" for a
// pass, "N/A" for an unbacked check, "FAILED" for a real failure — so cmd/forge
// can feed those results into a later phase's prompt. This drives all three states:
// the fixture's harness-gates phase runs lint/test/build, scripted here as
// pass / N/A / fail respectively, and the run aborts at the failing build gate.
func TestRunGates_OnGateResultReportsEachVerdict(t *testing.T) {
	wf := loadFixture(t)
	rec := &recorder{}
	// lint -> pass, test -> N/A (tri-state Status), build -> fail.
	triState := func(name string) gate.Result {
		switch name {
		case "test":
			return gate.Result{Name: name, Status: gate.StatusNA, Output: "no tool in this repo"}
		case "build":
			return gate.Result{Name: name, OK: false, Output: "boom"}
		default:
			return gate.Result{Name: name, OK: true}
		}
	}
	var got []string
	eng := Engine{
		Exec: rec.executor(), RunGate: triState, Log: rec.log,
		OnGateResult: func(name, status string) { got = append(got, name+"="+status) },
	}

	if err := eng.Run(wf, "balanced"); err == nil {
		t.Fatal("Run should abort on the failing build gate")
	}
	// Order follows the phase's required_gates; the run aborts AT build, so the
	// trailing qa gate never reports. All three verdict strings must be exact.
	want := []string{"lint=ok", "test=N/A", "build=FAILED"}
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Errorf("OnGateResult calls = %v, want %v", got, want)
	}
}

// A nil OnGateResult must be a silent no-op: a run that does not wire the callback
// behaves exactly as before the field existed (back-compat) and never panics.
func TestRunGates_NilOnGateResultIsBackCompat(t *testing.T) {
	wf := loadFixture(t)
	rec := &recorder{}
	eng := Engine{Exec: rec.executor(), RunGate: allOK, Log: rec.log} // OnGateResult nil
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("a nil OnGateResult must not panic; got %v", r)
		}
	}()
	if err := eng.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run with a nil OnGateResult should still pass on all-OK gates; got %v", err)
	}
}

// requiredWhenKey reduces a verbatim fragment to its trailing identifier so the
// orchestrator can match it against the reviewer dimension.
func TestRequiredWhenKey(t *testing.T) {
	cases := map[string]string{
		"../policies/modes.yml#workflow_depth.reviewer": "reviewer",
		"modes.yml#reviewer":                            "reviewer",
		"reviewer":                                      "reviewer",
		"":                                              "",
		"a.b.c":                                         "c",
	}
	for in, want := range cases {
		if got := requiredWhenKey(in); got != want {
			t.Errorf("requiredWhenKey(%q) = %q, want %q", in, got, want)
		}
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
