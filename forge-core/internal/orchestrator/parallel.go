// parallel.go — the OPT-IN parallel execution path (ROADMAP direction-5). Where
// RunFrom steps phases strictly SERIALLY (and owns directed gate loop-back), RunParallel
// groups phases into dependency WAVES (waves.go) and runs the mutually-independent phases
// WITHIN a wave CONCURRENTLY — so a Discover fan-out (scan/market/capability) or fan-out
// implementers no longer block one another. It is reached ONLY when the operator passes
// --parallel AND the workflow declares depends_on; every existing workflow (no depends_on,
// or no --parallel) keeps using RunFrom byte-for-byte.
//
// ═══════════════════════════════════════════════════════════════════════════
// LOCK ORDER CONTRACT (edgecases-and-perf.md §1.3)
// ═══════════════════════════════════════════════════════════════════════════
// Parallel mode accesses shared mutable state under multiple locks. The
// following LOCK ORDER must be strictly observed by every goroutine — any
// violation can cause a deadlock that is schedule-dependent (a Heisenbug).
//
// ACQUISITION ORDER (from outermost/earliest to innermost/latest):
//  1. trace.Tracer.mu        — trace event emission (the innermost / fastest)
//  2. runBudget.mu            — cost.go: cumulative spend tracking
//  3. loopProbe.mu            — gates.go: iteration-level acceptance probe cache
//  4. gateLedger.mu           — prompt_context.go: gate result recording
//  5. phaseOutputLedger.mu    — prompt_context.go: feed-forward output recording
//  6. ContextCache.mu         — internal/prompt/cache.go: ADR/AGENTS cache
//  7. reviewFindingsLedger.mu — prompt_context.go: reviewer findings
//  8. verdictLedger.mu        — prompt_context.go: reviewer verdict tracking
//
// Every function that holds a lock MUST document which level(s) it acquires.
// New mutable state added to the parallel path MUST be added to this contract.
// ═══════════════════════════════════════════════════════════════════════════
//
// SCOPE / HONESTY (v1):
//   - FAIL-FAST: per-wave context cancellation. When a phase fails, the wave
//     context is cancelled so remaining phases abort promptly via CommandExecutor's
//     commandContext chain — no wasted agent budget on discarded work.
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
	"context"
	"fmt"
	"sync"

	"forgeos/forge-core/internal/asset"
)

// RunParallel executes the workflow wave by wave, running each wave's independent phases
// concurrently. It returns the FIRST phase error (after letting the failing wave finish),
// or nil on a clean run. A malformed dependency graph (unknown dep / cycle) aborts before
// any phase runs (Waves is fail-closed). The discover-stage and review-stage mode skips
// are honored exactly as in RunFrom (see stageSkipped, mode_gating.go). ctx propagates
// cancellation so a cancelled run stops between waves and each active phase goroutine
// checks ctx before spawning.
func (e Engine) RunParallel(ctx context.Context, wf asset.Workflow, mode string) error {
	if err := asset.ValidateWorkflowStructure(wf); err != nil {
		return fmt.Errorf("parallel orchestration: invalid workflow structure: %w", err)
	}
	for _, p := range wf.Phases {
		if p.VerdictContract == asset.VerdictContractQAV1 {
			return fmt.Errorf(
				"parallel orchestration: phase %s: verdict_contract %q requires serial directed loop-back orchestration",
				p.Name, p.VerdictContract,
			)
		}
	}
	if e.checkStageSkip(wf) {
		return nil
	}
	var err error
	wf, err = e.applyEvolveCutoff(wf)
	if err != nil {
		return fmt.Errorf("parallel orchestration: evolve policy: %w", err)
	}
	waves, err := Waves(wf.Phases)
	if err != nil {
		return fmt.Errorf("parallel orchestration: %w", err)
	}
	e.logf("parallel: %d phase(s) in %d dependency wave(s)", len(wf.Phases), len(waves))
	var mu sync.Mutex
	agentCalls := 0
	var firstErr error
	for w, wave := range waves {
		if err := e.runWave(ctx, wf, mode, w, wave, &mu, &agentCalls, &firstErr); err != nil {
			return err
		}
	}
	e.reportStop(wf)
	return nil
}

// runWave runs one dependency wave: spawns its phases concurrently and cancels
// the wave on the first failure (fail-fast via per-wave context).
func (e Engine) runWave(parentCtx context.Context, wf asset.Workflow, mode string, w int, wave []int, mu *sync.Mutex, agentCalls *int, firstErr *error) error {
	// Before each wave, check if the parent context has been cancelled.
	select {
	case <-parentCtx.Done():
		if *firstErr == nil {
			*firstErr = fmt.Errorf("cancelled at wave %d: %w", w, parentCtx.Err())
		}
		return *firstErr
	default:
	}
	// FAIL-FAST: per-wave cancellable context. When any phase fails, cancel
	// the wave — remaining phases abort via CommandExecutor's context chain.
	waveCtx, waveCancel := context.WithCancel(parentCtx)
	defer waveCancel() // ensure cleanup even on success
	e.logf("parallel: wave %d — %d concurrent phase(s)", w+1, len(wave))
	var wg sync.WaitGroup
	completed := 0
	var completedMu sync.Mutex
	for _, idx := range wave {
		if waveCtx.Err() != nil {
			continue
		}
		wg.Add(1)
		go func(i int) {
			defer wg.Done()
			if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
				mu.Lock()
				if *firstErr == nil {
					*firstErr = err
					waveCancel()
				}
				mu.Unlock()
			} else {
				completedMu.Lock()
				completed++
				completedMu.Unlock()
			}
		}(idx)
	}
	wg.Wait()
	if *firstErr != nil {
		discarded := len(wave) - completed
		if discarded > 0 {
			e.logf("parallel: wave %d cancelled after %d/%d phases (%d discarded — potential cost loss)",
				w+1, completed, len(wave), discarded)
		}
		return *firstErr
	}
	return nil
}

// runPhaseParallel runs ONE phase under the parallel engine — the concurrency-safe,
// loop-back-free analogue of RunFrom's loop body. Required gates run first (a red gate
// returns an error -> RunParallel aborts; no loop-back). `agent: harness` is gate-only;
// every other gated agent continues through the same budgeted executor path as an ungated
// agent. A mode-skipped phase is a no-op. It never fires OnPhase (see the file header: no
// per-phase checkpoint in parallel mode).
func (e Engine) runPhaseParallel(ctx context.Context, wf asset.Workflow, i int, mode string, mu *sync.Mutex, agentCalls *int) error {
	p := wf.Phases[i]
	// If the wave context was cancelled (another phase in this wave failed),
	// abort promptly — do not start billing for discarded work.
	if err := ctx.Err(); err != nil {
		return err
	}
	if len(p.RequiredGates) > 0 {
		if err := e.runGates(p, e.gatesFor(p)); err != nil {
			return err
		}
		if gateOnlyPhase(p) {
			return nil
		}
	}
	if e.skipByMode(p, wf.Stage) {
		e.logf("phase %s skipped (mode gating: reviewer off)", p.Name)
		return nil
	}
	e.narrateADR(wf, p)
	// Budget pre-flight under the shared lock (agentCalls is mutated by every goroutine);
	// checkAgentBudget increments it only for an allowed execution.
	mu.Lock()
	budgetErr := e.checkAgentBudget(agentCalls)
	completed := *agentCalls
	if budgetErr == nil {
		completed--
	}
	mu.Unlock()
	if budgetErr != nil {
		return budgetErr
	}
	if err := e.checkRunBudget(completed); err != nil {
		return err
	}
	return e.runAgentPhase(ctx, p, mode)
}
