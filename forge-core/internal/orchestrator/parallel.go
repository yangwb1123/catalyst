// parallel.go — the OPT-IN parallel execution path (ROADMAP direction-5). Where
// RunFrom steps phases strictly SERIALLY (and owns directed gate loop-back), RunParallel
// groups phases into dependency WAVES (waves.go) and runs the mutually-independent phases
// WITHIN a wave CONCURRENTLY — so a Discover fan-out (scan/market/capability) or fan-out
// implementers no longer block one another. It is reached ONLY when the operator passes
// --parallel AND the workflow declares depends_on; every existing workflow (no depends_on,
// or no --parallel) keeps using RunFrom byte-for-byte.
//
// SCOPE / HONESTY (v1):
//   - NO directed loop-back. Loop-back (a red gate jumping back to re-run an agent phase)
//     is a SEQUENTIAL-SPINE feature; a fan-out wave has no single "back" target. So in
//     parallel mode a red gate ABORTS the run (fail-closed) rather than looping — the spine
//     workflows that rely on loop-back (build.yml) simply stay on the serial RunFrom.
//   - NO per-phase checkpoint. RunParallel does NOT fire Engine.OnPhase: concurrent phases
//     completing at once cannot share a single linear PhaseIndex, and concurrent checkpoint
//     writes would race. Per-ITERATION checkpointing (the loop's OnIteration) is unaffected,
//     so `forge evolve --parallel` still resumes at iteration boundaries.
//   - Shared cost/prompt state IS concurrency-safe — every mutable structure the concurrent
//     spawn touches takes a mutex: runBudget (cost.go), the four prompt ledgers
//     (prompt_context.go), the prompt ContextCache (internal/prompt/cache.go) AND, on the
//     evolve path, loopProbe (gates.go) — plus trace.Tracer.Emit already locks, and the
//     agent-call budget counter here is guarded by the local mu below. (ContextCache +
//     loopProbe were MISSED in the first cut — a fresh-context reviewer's -race run caught the
//     ContextCache race end-to-end; the guards above close it.) The slow part — each phase's
//     spawn (and its ADR retrieval) — runs OUTSIDE every lock, so phases genuinely overlap.
package orchestrator

import (
	"fmt"
	"sync"

	"forgeos/forge-core/internal/asset"
)

// RunParallel executes the workflow wave by wave, running each wave's independent phases
// concurrently. It returns the FIRST phase error (after letting the failing wave finish),
// or nil on a clean run. A malformed dependency graph (unknown dep / cycle) aborts before
// any phase runs (Waves is fail-closed). The discover-stage mode skip is honored exactly
// as in RunFrom.
func (e Engine) RunParallel(wf asset.Workflow, mode string) error {
	if e.discoverStageSkipped(wf) {
		e.logf("discover stage skipped (mode gating: explorer skips discovery)")
		e.reportStop(wf)
		return nil
	}
	waves, err := Waves(wf.Phases)
	if err != nil {
		return fmt.Errorf("parallel orchestration: %w", err)
	}
	e.logf("parallel: %d phase(s) in %d dependency wave(s)", len(wf.Phases), len(waves))
	var mu sync.Mutex // guards agentCalls + firstErr across the wave's goroutines
	agentCalls := 0
	var firstErr error
	for w, wave := range waves {
		e.logf("parallel: wave %d/%d — %d concurrent phase(s)", w+1, len(waves), len(wave))
		var wg sync.WaitGroup
		for _, idx := range wave {
			wg.Add(1)
			go func(i int) {
				defer wg.Done()
				if err := e.runPhaseParallel(wf, i, mode, &mu, &agentCalls); err != nil {
					mu.Lock()
					if firstErr == nil {
						firstErr = err
					}
					mu.Unlock()
				}
			}(idx)
		}
		wg.Wait() // wg.Wait establishes happens-before: firstErr is safe to read after it.
		if firstErr != nil {
			return firstErr // abort: do NOT start the next wave on a failure
		}
	}
	e.reportStop(wf)
	return nil
}

// runPhaseParallel runs ONE phase under the parallel engine — the concurrency-safe,
// loop-back-free analogue of RunFrom's loop body. A gate phase runs its gates (a red gate
// returns an error -> RunParallel aborts; no loop-back). An agent phase charges BOTH run-
// level budgets under the shared lock (the counter is shared) and then spawns OUTSIDE the
// lock so phases overlap. A mode-skipped phase is a no-op. It never fires OnPhase (see the
// file header: no per-phase checkpoint in parallel mode).
func (e Engine) runPhaseParallel(wf asset.Workflow, i int, mode string, mu *sync.Mutex, agentCalls *int) error {
	p := wf.Phases[i]
	if len(p.RequiredGates) > 0 {
		return e.runGates(p, e.gatesFor(p))
	}
	if e.skipByMode(p) {
		e.logf("phase %s skipped (mode gating: reviewer off)", p.Name)
		return nil
	}
	e.narrateADR(wf, p)
	// Budget pre-flight under the shared lock (agentCalls is mutated by every goroutine);
	// checkAgentBudget increments it, so completed = the count BEFORE this phase.
	mu.Lock()
	budgetErr := e.checkAgentBudget(agentCalls)
	completed := *agentCalls - 1
	mu.Unlock()
	if budgetErr != nil {
		return budgetErr
	}
	if err := e.checkRunBudget(completed); err != nil {
		return err
	}
	return e.runAgentPhase(p, mode)
}
