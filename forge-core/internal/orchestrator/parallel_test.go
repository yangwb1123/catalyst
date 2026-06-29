package orchestrator

import (
	"strings"
	"sync"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
)

// barrierExec PROVES a wave's phases run CONCURRENTLY (not serially): each phase, on
// entry, decrements a WaitGroup sized to the wave, then waits for ALL wave-mates to
// arrive (with a timeout). Under true concurrency every phase arrives and all proceed; a
// SERIAL execution strands the first phase at the barrier -> timeout -> timedOut=true, so
// the test fails fast rather than hanging.
type barrierExec struct {
	wg       *sync.WaitGroup
	mu       sync.Mutex
	executed []string
	timedOut bool
}

func (b *barrierExec) Execute(p asset.Phase, _ string) error {
	b.mu.Lock()
	b.executed = append(b.executed, p.Name)
	b.mu.Unlock()
	b.wg.Done()
	done := make(chan struct{})
	go func() { b.wg.Wait(); close(done) }()
	select {
	case <-done:
	case <-time.After(2 * time.Second):
		b.mu.Lock()
		b.timedOut = true
		b.mu.Unlock()
	}
	return nil
}

// safeRec is a thread-safe execution recorder for the dependency-order assertions.
type safeRec struct {
	mu       sync.Mutex
	executed []string
}

func (r *safeRec) Execute(p asset.Phase, _ string) error {
	r.mu.Lock()
	r.executed = append(r.executed, p.Name)
	r.mu.Unlock()
	return nil
}

func parallelWF(phases ...asset.Phase) asset.Workflow {
	return asset.Workflow{Stage: "discover", Phases: phases, Stop: externalStop()}
}

// A pure fan-out (no depends_on) runs ALL phases in ONE wave, CONCURRENTLY (the barrier
// proves overlap; -race proves the shared-state access is safe).
func TestRunParallel_FanOutRunsConcurrently(t *testing.T) {
	phases := []asset.Phase{ph("a"), ph("b"), ph("c")}
	var wg sync.WaitGroup
	wg.Add(len(phases))
	ex := &barrierExec{wg: &wg}
	eng := Engine{Exec: ex, RunGate: allOK}
	if err := eng.RunParallel(parallelWF(phases...), "balanced"); err != nil {
		t.Fatalf("RunParallel: %v", err)
	}
	if ex.timedOut {
		t.Fatal("phases did NOT overlap — a wave-mate never reached the barrier (serial, not parallel)")
	}
	if len(ex.executed) != 3 {
		t.Errorf("all 3 fan-out phases must run; got %v", ex.executed)
	}
}

// A diamond (a <- b,c <- d) respects WAVE ORDER: a before b/c, and b/c before d. Waves
// run sequentially (a barrier between them), so the executed list is wave-ordered.
func TestRunParallel_RespectsDependencyWaveOrder(t *testing.T) {
	rec := &safeRec{}
	eng := Engine{Exec: rec, RunGate: allOK}
	wf := parallelWF(ph("a"), ph("b", "a"), ph("c", "a"), ph("d", "b", "c"))
	if err := eng.RunParallel(wf, "balanced"); err != nil {
		t.Fatalf("RunParallel: %v", err)
	}
	pos := map[string]int{}
	for i, n := range rec.executed {
		pos[n] = i
	}
	if len(rec.executed) != 4 {
		t.Fatalf("all 4 phases must run; got %v", rec.executed)
	}
	// a is wave 0 (before everything); d is the last wave (after b and c).
	if !(pos["a"] < pos["b"] && pos["a"] < pos["c"] && pos["b"] < pos["d"] && pos["c"] < pos["d"]) {
		t.Errorf("dependency wave order violated; executed=%v", rec.executed)
	}
}

// A red GATE phase ABORTS the parallel run (no loop-back), and a phase in a LATER wave that
// depends on it NEVER runs.
func TestRunParallel_GateFailureAborts(t *testing.T) {
	rec := &safeRec{}
	failTest := func(name string) gate.Result { return gate.Result{Name: name, OK: name != "test"} }
	eng := Engine{Exec: rec, RunGate: failTest}
	wf := parallelWF(
		ph("impl"),
		asset.Phase{Name: "gate", RequiredGates: []string{"test"}, DependsOn: []string{"impl"}},
		ph("final", "gate"),
	)
	err := eng.RunParallel(wf, "balanced")
	if err == nil {
		t.Fatal("a red gate must abort the parallel run")
	}
	for _, n := range rec.executed {
		if n == "final" {
			t.Errorf("the phase after a failed gate must NOT run; executed=%v", rec.executed)
		}
	}
}

// A malformed dependency graph (cycle) aborts BEFORE any phase runs — fail-closed.
func TestRunParallel_CycleErrorsBeforeAnyPhase(t *testing.T) {
	rec := &safeRec{}
	eng := Engine{Exec: rec, RunGate: allOK}
	wf := parallelWF(ph("a", "b"), ph("b", "a"))
	err := eng.RunParallel(wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), "cycle") {
		t.Fatalf("a cycle must abort with an error; got %v", err)
	}
	if len(rec.executed) != 0 {
		t.Errorf("no phase may run on a malformed graph; executed=%v", rec.executed)
	}
}

// The per-run agent-call budget is enforced even under concurrency: with a cap below the
// fan-out size, at least one phase is refused (fail-closed) and the run errors.
func TestRunParallel_AgentBudgetEnforcedConcurrently(t *testing.T) {
	rec := &safeRec{}
	eng := Engine{Exec: rec, RunGate: allOK, MaxAgentCalls: 2}
	wf := parallelWF(ph("a"), ph("b"), ph("c"), ph("d")) // 4 phases, cap 2
	err := eng.RunParallel(wf, "balanced")
	if err == nil || !strings.Contains(err.Error(), "budget") {
		t.Fatalf("a fan-out over the agent-call cap must fail-closed; got %v", err)
	}
}
