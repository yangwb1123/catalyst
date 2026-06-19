package orchestrator

import (
	"fmt"

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
	Engine     Engine                  // runs one iteration's phases (enforces gates)
	StopType   string                  // stop_condition.type ("conjunction", "external", ...)
	Stop       []asset.Criterion       // stop-condition criteria (conjunction)
	Signals    func() converge.Signals // live signals measured after an iteration
	MaxIter    int                     // safety backstop on iterations
	NoProgress int                     // halt after this many stale iterations (tripwire)
	Log        func(string)
}

// NewLoopEngine constructs a LoopEngine, clamping a non-positive NoProgress to 1
// so the doom-loop tripwire is always well-defined (it can never disable itself
// by being asked to fire after zero stale iterations).
func NewLoopEngine(eng Engine, stopType string, stop []asset.Criterion,
	signals func() converge.Signals, maxIter, noProgress int, log func(string)) LoopEngine {
	if noProgress < 1 {
		noProgress = 1
	}
	return LoopEngine{
		Engine: eng, StopType: stopType, Stop: stop, Signals: signals,
		MaxIter: maxIter, NoProgress: noProgress, Log: log,
	}
}

// external reports whether this is an external-stop workflow (no conjunction;
// runs to a safety bound or an external trigger, never round-count failure).
func (l LoopEngine) external() bool { return l.StopType == "external" }

// LoopOutcome reports how the loop ended.
type LoopOutcome struct {
	Iterations int
	Converged  bool
	Reason     string
}

// Run loops the workflow until convergence, a tripwire, a failure, or MaxIter.
// For an external-stop workflow, convergence is not a conjunction check: the
// loop runs to the safety bound (a clean stop) and reports it as such.
func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
	prev, stale := -1.0, 0
	for i := 1; i <= l.MaxIter; i++ {
		l.logf("iteration %d/%d", i, l.MaxIter)
		if err := l.Engine.Run(wf, mode); err != nil {
			return LoopOutcome{i, false, "gate/agent failure"}, err
		}
		sig := l.Signals()
		l.reportConvergence(sig)
		if out, done := l.checkStop(i, sig); done {
			return out, nil
		}
		if stale = staleCount(sig.RoadmapCompletion, prev, stale); l.tripped(stale) {
			return l.staleOutcome(i), nil
		}
		prev = sig.RoadmapCompletion
	}
	return l.boundOutcome(), nil
}

// checkStop reports a converged outcome for a conjunction workflow whose
// criteria are all met. External-stop workflows never converge on a conjunction
// (they have none) — they only end at the safety bound or a clean trigger.
func (l LoopEngine) checkStop(i int, sig converge.Signals) (LoopOutcome, bool) {
	if l.external() {
		return LoopOutcome{}, false
	}
	if _, met := converge.Evaluate(l.Stop, sig); met {
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
	results, met := converge.Evaluate(l.Stop, sig)
	l.logf("convergence: %s (%s)", convergeVerdict(met), l.StopType)
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
