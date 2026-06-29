package orchestrator

import (
	"fmt"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

// LoopEngine drives a workflow repeatedly until its stop condition converges.
// ForgeOS forbids round-count termination: MaxIter is a SAFETY backstop and
// NoProgress is an anti-doom-loop tripwire — neither is the goal. The loop ends
// on real convergence, a stale-progress tripwire, a gate/agent failure, or the
// safety bound, and always reports which.
//
// External-stop workflows (StopType=="external", e.g. evolve.yml) have no
// all_of conjunction: they run continuously until a human pause, budget
// exhaustion, or no-gaps-found. For those, reaching MaxIter is the EXPECTED
// non-failure outcome (a clean safety stop), and a stale-progress tripwire maps
// onto no_gaps_found semantics — also a clean stop. Such a workflow is NEVER
// silently degraded to round-count + failure.
type LoopEngine struct {
	Engine Engine // runs one iteration's phases (enforces gates)
	// Stop is the FULL stop condition (not just Type+AllOf), so the loop's
	// convergence check goes through converge.Converge — the single entry point
	// that honors EVERY stop shape. For a conjunction/external stop Converge
	// delegates to Evaluate(all_of), so those paths are byte-for-byte unchanged;
	// for a human_gate it judges by approval alone and NEVER by the all_of, so a
	// human_gate that somehow reaches the loop can never be bypassed by a satisfied
	// conjunction. (`forge evolve` also refuses a human_gate up front — see
	// cmd/forge/evolve.go rejectHumanGate — this is the depth-two backstop.)
	Stop       asset.StopCondition
	Signals    func() converge.Signals // live signals measured after an iteration
	MaxIter    int                     // safety backstop on iterations
	NoProgress int                     // halt after this many stale iterations (tripwire)
	Log        func(string)

	// OnIteration is an injected post-measurement hook called once per iteration,
	// right after Signals() — the point where "this round's work AND its
	// measurement are done", so it is the honest place to persist a checkpoint and
	// emit a trace event. It receives the 1-based iteration index, that round's
	// live signals, and the iteration's measured wall-clock duration in ms (the
	// observed cost of RunFrom — what telemetry needs for real p95 latency, since a
	// 0 here makes the scorecard read every iteration as instantaneous). Kept as an
	// injected callback (not a direct trace/persist import) so the engine stays
	// decoupled from the IO layer, matching how Engine/Signals/Log/RunGate are
	// already wired. Nil-safe: a nil hook is a no-op.
	OnIteration func(i int, sig converge.Signals, durationMs int64)

	// StartIter and ResumePrev support resuming a crashed/paused run from a
	// checkpoint. StartIter is the 1-based iteration to begin at (0 or 1 both mean
	// "start fresh from iteration 1" — the default). ResumePrev seeds the previous
	// RoadmapCompletion so the stale/tripwire computation is continuous across the
	// resume boundary; for a fresh run it is the sentinel -1.0 (set by loopStart),
	// so the first reading is never counted as "no progress vs nothing".
	StartIter  int
	ResumePrev float64

	// OnPhase is the ITERATION-AWARE per-phase checkpoint hook (the phase-granular
	// twin of OnIteration). Run sets Engine.OnPhase per iteration so the engine
	// reports a completed agent phase's index, and this is invoked with (iteration,
	// phaseIdx) so the caller can persist a checkpoint mid-iteration — a crash then
	// resumes at the next unstarted phase instead of replaying every completed
	// (billed) agent phase. Nil-safe; nil = per-iteration-only checkpointing (the
	// pre-phase-granular behavior). StartPhase is the phase index the FIRST resumed
	// iteration begins at (from a mid-iteration checkpoint's PhaseIndex); 0 (the
	// default) means the first iteration runs the whole workflow, byte-for-byte as
	// before. Only the first iteration honors it — subsequent iterations reset via
	// nextStartPhase (the on_unmet directed restart), unchanged.
	OnPhase    func(iter, phaseIdx int)
	StartPhase int

	// Parallel routes each iteration through Engine.RunParallel (dependency-wave
	// concurrency) instead of the serial RunFrom. Set by cmd/forge only when
	// --parallel AND the workflow declares depends_on. In parallel mode there is no
	// directed loop-back and no per-PHASE checkpoint (StartPhase/OnPhase are unused —
	// concurrent phases can't share a linear PhaseIndex); per-ITERATION checkpointing
	// is unaffected, so `forge evolve --parallel` still resumes at iteration boundaries.
	Parallel bool
}

// NewLoopEngine constructs a LoopEngine, clamping a non-positive NoProgress to 1
// so the doom-loop tripwire is always well-defined (it can never disable itself
// by being asked to fire after zero stale iterations). It takes the FULL stop
// condition so convergence is judged by converge.Converge (which honors every
// stop shape), not the conjunction-only Evaluate.
func NewLoopEngine(eng Engine, stop asset.StopCondition,
	signals func() converge.Signals, maxIter, noProgress int, log func(string)) LoopEngine {
	if noProgress < 1 {
		noProgress = 1
	}
	return LoopEngine{
		Engine: eng, Stop: stop, Signals: signals,
		MaxIter: maxIter, NoProgress: noProgress, Log: log,
	}
}

// external reports whether this is an external-stop workflow (no conjunction;
// runs to a safety bound or an external trigger, never round-count failure).
func (l LoopEngine) external() bool { return l.Stop.Type == "external" }

// LoopOutcome reports how the loop ended.
type LoopOutcome struct {
	Iterations int
	Converged  bool
	Reason     string
}

// Run loops the workflow until convergence, a tripwire, a failure, or MaxIter.
// For an external-stop workflow, convergence is not a conjunction check: the
// loop runs to the safety bound (a clean stop) and reports it as such.
//
// DIRECTED RESTART (on_unmet): the FIRST iteration always runs the whole workflow
// (startPhase 0). Once an iteration is measured NOT converged and the stop
// declares on_unmet:{action:loop_to_next_roadmap_item, target_phase: planner},
// every SUBSEQUENT iteration begins at the target phase (Engine.RunFrom) — the
// loop pulling the next roadmap item from the planner rather than replaying every
// phase. With no on_unmet (or an unresolvable target) startPhase stays 0 and the
// loop replays the whole workflow each round, byte-for-byte as before.
func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
	// FALSE-CLEAN GUARD: a workflow that resolves to ZERO phases runs no work, so it
	// must NEVER be reported as converged — that is exactly the "zero work read as
	// success" anti-pattern ForgeOS exists to prevent (it is how a dropped evolve loop
	// body silently passed before loop.phases was hoisted in asset.LoadWorkflowJSON).
	// Depth-two backstop: even a future stage-bearing-but-phaseless asset (or one nested
	// under an unrecognized key) ends not-converged with an honest reason, never a
	// false-clean boundOutcome. No real workflow trips this (all carry phases).
	if len(wf.Phases) == 0 {
		return LoopOutcome{0, false, "no phases to run (empty workflow — not converged)"}, nil
	}
	start, prev := l.loopStart()
	stale := 0
	startPhase := l.StartPhase // 0 fresh; a resumed mid-iteration checkpoint begins here.
	for i := start; i <= l.MaxIter; i++ {
		l.logf("iteration %d/%d", i, l.MaxIter)
		t0 := time.Now()
		// Parallel mode runs each iteration's full dependency-wave concurrency (no
		// startPhase resume, no per-phase checkpoint). Serial mode wires the per-phase
		// checkpoint hook to THIS iteration's index (a value receiver means this mutates
		// only the local Engine copy RunFrom reads).
		var runErr error
		if l.Parallel {
			runErr = l.Engine.RunParallel(wf, mode)
		} else {
			l.Engine.OnPhase = l.phaseCheckpoint(i)
			runErr = l.Engine.RunFrom(wf, mode, startPhase)
		}
		if runErr != nil {
			return LoopOutcome{i, false, "gate/agent failure"}, runErr
		}
		durationMs := time.Since(t0).Milliseconds()
		sig := l.Signals()
		l.onIteration(i, sig, durationMs)
		l.reportConvergence(sig)
		if out, done := l.checkStop(i, sig); done {
			return out, nil
		}
		if stale = staleCount(sig.RoadmapCompletion, prev, stale); l.tripped(stale) {
			return l.staleOutcome(i), nil
		}
		prev = sig.RoadmapCompletion
		startPhase = l.nextStartPhase(wf) // unmet this round -> directed restart next.
	}
	return l.boundOutcome(), nil
}

// nextStartPhase resolves where the NEXT iteration should begin after this one was
// measured not-converged. It is the on_unmet directed restart: when the stop
// declares action "loop_to_next_roadmap_item" and its target_phase resolves to a
// phase, the next iteration starts there (the planner, to pull the next roadmap
// item). Absent on_unmet, an unknown action, or an unresolvable target, it returns
// 0 — the next iteration replays the whole workflow, exactly the prior behavior.
func (l LoopEngine) nextStartPhase(wf asset.Workflow) int {
	ou := l.Stop.OnUnmet
	if ou == nil || ou.Action != "loop_to_next_roadmap_item" {
		return 0
	}
	if idx, ok := phaseIndex(wf, ou.TargetPhase); ok {
		return idx
	}
	return 0
}

// loopStart resolves the first iteration index and the initial `prev` completion
// from the resume fields. A StartIter of 0 or 1 is a fresh run: start at
// iteration 1 with prev = -1.0 (the sentinel that makes the first reading never
// register as stale). A StartIter > 1 is a resume: begin there and seed prev with
// ResumePrev (last persisted completion) so the stale/tripwire math is continuous
// across the resume — a flat first post-resume reading correctly counts as stale.
// This keeps the default (fresh) path bit-for-bit identical to the original.
func (l LoopEngine) loopStart() (start int, prev float64) {
	if l.StartIter > 1 {
		return l.StartIter, l.ResumePrev
	}
	return 1, -1.0
}

// onIteration invokes the post-measurement hook (the checkpoint/trace point) when
// one is injected, forwarding the round's measured wall-clock duration. Nil-safe,
// so the loop runs unchanged when no hook is wired.
func (l LoopEngine) onIteration(i int, sig converge.Signals, durationMs int64) {
	if l.OnIteration != nil {
		l.OnIteration(i, sig, durationMs)
	}
}

// phaseCheckpoint builds the engine's per-phase hook BOUND to iteration `iter`,
// forwarding each completed agent phase's index to the iteration-aware l.OnPhase.
// Returns nil when no per-phase checkpoint is wired, so the engine's hook stays nil
// and the per-iteration-only path is byte-for-byte unchanged.
func (l LoopEngine) phaseCheckpoint(iter int) func(phaseIdx int) {
	if l.OnPhase == nil {
		return nil
	}
	return func(phaseIdx int) { l.OnPhase(iter, phaseIdx) }
}

// checkStop reports a converged outcome for a conjunction workflow whose
// criteria are all met. External-stop workflows never converge on a conjunction
// (they have none) — they only end at the safety bound or a clean trigger.
//
// Convergence goes through converge.Converge (the dispatch that honors every stop
// shape), NOT the conjunction-only Evaluate. For a conjunction this is identical
// to before (Converge delegates to Evaluate(all_of)); the difference is the
// safety property: if a human_gate ever reaches the loop, Converge judges it by
// approval alone, so an unapproved human_gate can NEVER report "converged" here —
// not even with a fully satisfied all_of. (`forge evolve` refuses a human_gate
// before the loop; this is the depth-two backstop that holds regardless.)
func (l LoopEngine) checkStop(i int, sig converge.Signals) (LoopOutcome, bool) {
	if l.external() {
		return LoopOutcome{}, false
	}
	if _, met := converge.Converge(l.Stop, sig); met {
		return LoopOutcome{i, true, "converged"}, true
	}
	return LoopOutcome{}, false
}

// tripped reports whether the no-progress tripwire fires. It requires at least
// one stale iteration AND a positive NoProgress bound, so a zero/negative bound
// can never trip on the very first flat reading.
func (l LoopEngine) tripped(stale int) bool {
	return stale >= l.NoProgress && l.NoProgress >= 1
}

// staleOutcome describes a stale-progress halt. For an external workflow this is
// a clean stop (no_gaps_found semantics); for a conjunction it is the anti
// doom-loop tripwire. Neither is a failure.
func (l LoopEngine) staleOutcome(i int) LoopOutcome {
	if l.external() {
		return LoopOutcome{i, true, "no gaps found (external stop)"}
	}
	return LoopOutcome{i, false, "no-progress tripwire (anti doom-loop)"}
}

// boundOutcome describes hitting MaxIter. For an external workflow this is the
// EXPECTED clean stop (ran to the safety bound); for a conjunction it is the
// safety backstop firing without convergence.
func (l LoopEngine) boundOutcome() LoopOutcome {
	if l.external() {
		return LoopOutcome{l.MaxIter, true, "ran to safety bound (external stop)"}
	}
	return LoopOutcome{l.MaxIter, false, "max-iterations safety bound"}
}

// reportConvergence logs the LIVE stop verdict for this iteration so `forge
// evolve` surfaces the same real per-criterion evaluation that `forge run`
// prints. An external-stop workflow has no conjunction to evaluate; it reports
// the live signals it runs against instead.
func (l LoopEngine) reportConvergence(sig converge.Signals) {
	if l.external() {
		l.logf("convergence: external stop (runs to safety bound) — roadmap_completion=%.0f%%, gates_green=%v",
			sig.RoadmapCompletion*100, sig.GatesGreen)
		return
	}
	results, met := converge.Converge(l.Stop, sig)
	l.logf("convergence: %s (%s)", convergeVerdict(met), l.Stop.Type)
	for _, r := range results {
		l.logf("  [%s] %s — %s", convergeMark(r.Met), r.Expr, r.Detail)
	}
}

func convergeVerdict(met bool) string {
	if met {
		return "MET"
	}
	return "NOT MET"
}

func convergeMark(met bool) string {
	if met {
		return "x"
	}
	return " "
}

// staleCount increments the stale counter when progress did not advance, else
// resets it — the basis of the doom-loop tripwire.
func staleCount(cur, prev float64, stale int) int {
	if cur <= prev {
		return stale + 1
	}
	return 0
}

func (l LoopEngine) logf(format string, args ...any) {
	if l.Log != nil {
		l.Log(fmt.Sprintf(format, args...))
	}
}
