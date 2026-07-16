package orchestrator

import (
	"context"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

func ptr(f float64) *float64 { return &f }

// roadmapDone is the conjunction criterion "roadmap_completion == 100".
func roadmapDone() []asset.Criterion {
	return []asset.Criterion{{Metric: "roadmap_completion", Operator: "==", Threshold: ptr(100)}}
}

// conjunctionStop wraps criteria as a conjunction stop condition (the shape the
// LoopEngine now holds — the full StopCondition, not bare criteria).
func conjunctionStop(allOf []asset.Criterion) asset.StopCondition {
	return asset.StopCondition{Type: "conjunction", AllOf: allOf}
}

// externalStop is the external-stop shape (no conjunction; runs to a safety
// bound or a clean external trigger).
func externalStop() asset.StopCondition {
	return asset.StopCondition{Type: "external"}
}

// signalSeq yields each entry in turn (repeating the last), so a test can
// script the convergence trajectory the loop sees.
func signalSeq(seq ...converge.Signals) func() converge.Signals {
	i := 0
	return func() converge.Signals {
		s := seq[i]
		if i < len(seq)-1 {
			i++
		}
		return s
	}
}

func loopOver(sig func() converge.Signals, maxIter, noProgress int) LoopEngine {
	return NewLoopEngine(
		Engine{Exec: DryRunExecutor{}, RunGate: allOK},
		conjunctionStop(roadmapDone()), sig, maxIter, noProgress, nil)
}

func TestLoop_Converges(t *testing.T) {
	wf := loadFixture(t)
	out, err := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 1.0}), 5, 3).Run(wf, "balanced")
	if err != nil || !out.Converged || out.Reason != "converged" {
		t.Fatalf("expected converged; got %+v err=%v", out, err)
	}
}

// PHASE-GRANULAR RESUME: StartPhase makes the FIRST iteration begin mid-workflow, so a
// crash that was checkpointed at phase K resumes there — the already-completed earlier
// phases are NOT re-run. Here StartPhase=implementer's index, so planner (before it) is
// skipped while implementer still runs.
func TestLoop_StartPhaseSkipsCompletedPhasesOnResume(t *testing.T) {
	wf := loadFixture(t)
	implIdx := -1
	for i, p := range wf.Phases {
		if p.Name == "implementer" {
			implIdx = i
		}
	}
	if implIdx < 1 {
		t.Skip("fixture lacks a non-first implementer phase")
	}
	rec := &recorder{}
	l := NewLoopEngine(Engine{Exec: rec.executor(), RunGate: allOK}, conjunctionStop(roadmapDone()),
		signalSeq(converge.Signals{RoadmapCompletion: 1.0}), 5, 3, nil)
	l.StartPhase = implIdx
	if _, err := l.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run: %v", err)
	}
	if contains(rec.executed, "planner") {
		t.Errorf("StartPhase=%d must SKIP the already-done planner; executed=%v", implIdx, rec.executed)
	}
	if !contains(rec.executed, "implementer") {
		t.Errorf("StartPhase=%d must still run implementer; executed=%v", implIdx, rec.executed)
	}
}

// The loop wires the engine's per-phase hook to EACH iteration's index, so l.OnPhase
// receives (iteration, phaseIdx) for every completed agent phase. Here it converges on
// iteration 1, so every callback must carry iter==1.
func TestLoop_OnPhaseCarriesIterationContext(t *testing.T) {
	wf := loadFixture(t)
	var calls [][2]int
	l := NewLoopEngine(Engine{Exec: DryRunExecutor{}, RunGate: allOK}, conjunctionStop(roadmapDone()),
		signalSeq(converge.Signals{RoadmapCompletion: 1.0}), 5, 3, nil)
	l.OnPhase = func(iter, phaseIdx int) { calls = append(calls, [2]int{iter, phaseIdx}) }
	if _, err := l.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run: %v", err)
	}
	if len(calls) == 0 {
		t.Fatal("l.OnPhase never fired through the loop — the per-phase wiring is dead")
	}
	for _, c := range calls {
		if c[0] != 1 {
			t.Errorf("OnPhase iteration = %d, want 1 (converged on iteration 1); calls=%v", c[0], calls)
		}
	}
}

// Flat progress must trip the doom-loop guard rather than spin forever.
func TestLoop_TripwireOnNoProgress(t *testing.T) {
	wf := loadFixture(t)
	out, _ := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.75}), 10, 2).Run(wf, "balanced")
	if out.Converged || out.Reason != "no-progress tripwire (anti doom-loop)" {
		t.Errorf("flat progress must trip the doom-loop guard; got %+v", out)
	}
}

// Rising-but-incomplete progress makes no convergence and no tripwire, so the
// safety bound ends it.
func TestLoop_MaxIterBound(t *testing.T) {
	wf := loadFixture(t)
	rising := signalSeq(
		converge.Signals{RoadmapCompletion: 0.2},
		converge.Signals{RoadmapCompletion: 0.4},
		converge.Signals{RoadmapCompletion: 0.6},
	)
	out, _ := loopOver(rising, 3, 5).Run(wf, "balanced")
	if out.Converged || out.Reason != "max-iterations safety bound" || out.Iterations != 3 {
		t.Errorf("rising-but-incomplete must hit the safety bound; got %+v", out)
	}
}

// P12: NoProgress is clamped to 1 by the constructor, so a flat first reading
// trips on the second flat iteration — not on the very first reading.
func TestLoop_NoProgressZeroClampedToOne(t *testing.T) {
	wf := loadFixture(t)
	l := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.5}), 10, 0)
	if l.NoProgress != 1 {
		t.Fatalf("NoProgress=0 must clamp to 1; got %d", l.NoProgress)
	}
	out, _ := l.Run(wf, "balanced")
	// First iter: cur(0.5) <= prev(-1)? no -> stale stays 0, not tripped.
	// Second iter: cur(0.5) <= prev(0.5)? yes -> stale=1 >= 1 -> trips.
	if out.Converged || out.Reason != "no-progress tripwire (anti doom-loop)" || out.Iterations != 2 {
		t.Errorf("clamped NoProgress should trip on the 2nd flat iter; got %+v", out)
	}
}

// P12: NoProgress=1 trips on the first flat-vs-previous reading.
func TestLoop_NoProgressOne(t *testing.T) {
	wf := loadFixture(t)
	out, _ := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.3}), 10, 1).Run(wf, "balanced")
	if out.Converged || out.Reason != "no-progress tripwire (anti doom-loop)" || out.Iterations != 2 {
		t.Errorf("NoProgress=1 should trip on the 2nd flat iter; got %+v", out)
	}
}

// P12: MaxIter=0 runs zero iterations and reports the safety bound cleanly,
// without entering the loop body.
func TestLoop_MaxIterZero(t *testing.T) {
	wf := loadFixture(t)
	out, err := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 1.0}), 0, 2).Run(wf, "balanced")
	if err != nil {
		t.Fatalf("MaxIter=0 should not error; got %v", err)
	}
	if out.Converged || out.Reason != "max-iterations safety bound" || out.Iterations != 0 {
		t.Errorf("MaxIter=0 must report the bound with 0 iterations; got %+v", out)
	}
}

// P7: an external-stop workflow that hits MaxIter is the EXPECTED clean outcome
// — Converged=true with an honest reason — never a round-count failure.
func TestLoop_ExternalStopHitsBoundCleanly(t *testing.T) {
	wf := loadFixture(t)
	l := NewLoopEngine(
		Engine{Exec: DryRunExecutor{}, RunGate: allOK},
		externalStop(),
		signalSeq(converge.Signals{RoadmapCompletion: 0.4, GatesGreen: false}),
		3, 5, nil)
	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("external stop must not error; got %v", err)
	}
	if !out.Converged || out.Reason != "ran to safety bound (external stop)" {
		t.Errorf("external stop at MaxIter must be a clean stop; got %+v", out)
	}
}

// FALSE-CLEAN GUARD: a workflow with ZERO phases runs no work and must NEVER report
// converged — not even an external-stop loop (which otherwise reports a clean stop at
// the bound). This is the depth-two backstop behind asset.LoadWorkflowJSON's loop.phases
// hoist: a stage-bearing-but-phaseless asset can no longer silently pass as "converged".
func TestLoop_ZeroPhasesNeverConverges(t *testing.T) {
	empty := asset.Workflow{Stage: "evolve", Stop: externalStop()} // no phases
	l := NewLoopEngine(
		Engine{Exec: DryRunExecutor{}, RunGate: allOK},
		externalStop(),
		signalSeq(converge.Signals{RoadmapCompletion: 1.0, GatesGreen: true}), // would "converge" if run
		3, 5, nil)
	out, err := l.Run(empty, "balanced")
	if err != nil {
		t.Fatalf("a zero-phase workflow must not error; got %v", err)
	}
	if out.Converged {
		t.Errorf("a zero-phase workflow must NOT be reported converged (false-clean); got %+v", out)
	}
	if out.Iterations != 0 {
		t.Errorf("a zero-phase workflow runs no iterations; got %d", out.Iterations)
	}
}

// P7: a stale external-stop loop reports no_gaps_found semantics — a clean stop,
// never the conjunction tripwire.
func TestLoop_ExternalStopNoGapsFound(t *testing.T) {
	wf := loadFixture(t)
	l := NewLoopEngine(
		Engine{Exec: DryRunExecutor{}, RunGate: allOK},
		externalStop(),
		signalSeq(converge.Signals{RoadmapCompletion: 0.5}),
		10, 2, nil)
	out, _ := l.Run(wf, "balanced")
	if !out.Converged || out.Reason != "no gaps found (external stop)" {
		t.Errorf("stale external stop must map to no_gaps_found clean stop; got %+v", out)
	}
}

// --- human_gate non-bypassability AT THE LOOPENGINE LAYER ---------------------

// humanGateStop is a human_gate stop condition that ALSO carries a fully
// satisfiable all_of (roadmap==100 AND gates==green). If the loop ever evaluated
// the conjunction instead of the approval, this all_of would make it "converge" —
// so it is exactly the bypass the depth-two guard must defeat.
func humanGateStop() asset.StopCondition {
	return asset.StopCondition{
		Type:          converge.HumanGateType,
		HumanApproval: "required",
		AllOf:         []asset.Criterion{{Metric: "roadmap_completion", Operator: "==", Threshold: ptr(100)}, {Metric: "gates_status", Operator: "==", Value: "green"}},
	}
}

// THE depth-two security invariant, proven at the LoopEngine layer (not just the
// converge pure function): a human_gate driven by the loop must NEVER converge
// without approval — even when its all_of is fully satisfied, roadmap is 100%, and
// gates are green. Before the fix the loop called converge.Evaluate(all_of) and
// reported converged=true here; now it calls converge.Converge, which judges a
// human_gate by approval alone, so an unapproved human_gate can only end at the
// safety bound. This is the regression test for the bypass the reviewer found.
func TestLoop_HumanGateUnapprovedNeverConvergesInLoop(t *testing.T) {
	wf := loadFixture(t)
	maxed := converge.Signals{
		RoadmapCompletion: 1.0,
		GatesGreen:        true,
		Criteria:          map[string]string{"test_pass": "PASS", "app_test_pass": "PASS", "architecture": "PASS"},
		HumanApproved:     false, // the ONLY thing missing
	}
	l := NewLoopEngine(
		Engine{Exec: DryRunExecutor{}, RunGate: allOK},
		humanGateStop(), signalSeq(maxed), 3, 5, nil)
	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("unapproved human_gate loop must not error; got %v", err)
	}
	if out.Converged {
		t.Fatalf("BYPASS: unapproved human_gate converged in the loop despite satisfied all_of/roadmap/gates; got %+v", out)
	}
	if out.Reason != "max-iterations safety bound" || out.Iterations != 3 {
		t.Errorf("unapproved human_gate must run to the safety bound, never converge; got %+v", out)
	}
}

// The mirror: once approval is present, the SAME human_gate loop converges on the
// first iteration — proving approval is the sole lever, not a permanent block, and
// that the depth-two guard is approval-gated rather than just always-deny.
func TestLoop_HumanGateApprovedConvergesInLoop(t *testing.T) {
	wf := loadFixture(t)
	approved := converge.Signals{RoadmapCompletion: 1.0, GatesGreen: true, HumanApproved: true}
	l := NewLoopEngine(
		Engine{Exec: DryRunExecutor{}, RunGate: allOK},
		humanGateStop(), signalSeq(approved), 3, 5, nil)
	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("approved human_gate loop must not error; got %v", err)
	}
	if !out.Converged || out.Reason != "converged" || out.Iterations != 1 {
		t.Errorf("approved human_gate must converge on iter 1; got %+v", out)
	}
}

// iterObs records each OnIteration call so a test can assert the hook fires once
// per round with the right index, the round's measured signals, and the measured
// wall-clock duration the loop timed for that iteration.
type iterObs struct {
	iters []int
	sigs  []converge.Signals
	durs  []int64
}

func (o *iterObs) hook() func(int, converge.Signals, int64) {
	return func(i int, sig converge.Signals, durationMs int64) {
		o.iters = append(o.iters, i)
		o.sigs = append(o.sigs, sig)
		o.durs = append(o.durs, durationMs)
	}
}

// OnIteration must fire exactly once per executed iteration, after that round's
// Signals() measurement, with the 1-based index and the signals that round saw.
// This is the persistence/trace point the resilience wiring hangs off of.
func TestLoop_OnIterationCalledPerRound(t *testing.T) {
	wf := loadFixture(t)
	obs := &iterObs{}
	l := loopOver(signalSeq(
		converge.Signals{RoadmapCompletion: 0.2},
		converge.Signals{RoadmapCompletion: 0.5},
		converge.Signals{RoadmapCompletion: 0.8},
	), 3, 5)
	l.OnIteration = obs.hook()
	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if out.Iterations != 3 {
		t.Fatalf("expected 3 iterations (rising-but-incomplete -> bound); got %+v", out)
	}
	if want := []int{1, 2, 3}; !eqInts(obs.iters, want) {
		t.Errorf("OnIteration indices = %v, want %v", obs.iters, want)
	}
	// The hook must observe each round's OWN measured signal, in order.
	wantSig := []float64{0.2, 0.5, 0.8}
	for i, s := range obs.sigs {
		if s.RoadmapCompletion != wantSig[i] {
			t.Errorf("OnIteration[%d] sig = %.2f, want %.2f", i, s.RoadmapCompletion, wantSig[i])
		}
	}
}

// sleepyExecutor is an AgentExecutor whose RunFrom (via Execute on each phase)
// sleeps a fixed span, so the loop's wall-clock measurement around RunFrom is a
// real, non-zero observation rather than an unmeasurably fast no-op. It lets the
// duration assertion below be honest (a true elapsed time) without a fake clock.
type sleepyExecutor struct{ d time.Duration }

func (s sleepyExecutor) Execute(_ context.Context, _ asset.Phase, _ string) error {
	time.Sleep(s.d)
	return nil
}

// The duration the loop measures around RunFrom must reach OnIteration as a
// positive wall-clock value — the honest per-iteration cost telemetry needs for
// p95 latency. Before the fix the hook never saw a duration and the trace recorded
// 0; here a sleeping executor guarantees real elapsed time, so every observed
// durationMs must be > 0. (We assert strictly positive, not an exact value: the
// measurement is a real clock reading, so its precise magnitude is non-deterministic.)
func TestLoop_OnIterationReceivesMeasuredDuration(t *testing.T) {
	wf := loadFixture(t)
	obs := &iterObs{}
	l := NewLoopEngine(
		Engine{Exec: sleepyExecutor{d: 5 * time.Millisecond}, RunGate: allOK},
		conjunctionStop(roadmapDone()),
		signalSeq(
			converge.Signals{RoadmapCompletion: 0.3},
			converge.Signals{RoadmapCompletion: 0.6},
		),
		2, 9, nil)
	l.OnIteration = obs.hook()
	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if len(obs.durs) != out.Iterations || out.Iterations == 0 {
		t.Fatalf("expected one measured duration per iteration; durs=%v out=%+v", obs.durs, out)
	}
	for i, d := range obs.durs {
		if d <= 0 {
			t.Errorf("OnIteration[%d] durationMs = %d, want > 0 (real measured wall-clock)", i, d)
		}
	}
}

// A nil OnIteration must be a no-op — the loop runs exactly as before (this is
// the default/back-compat path every existing test already exercises, asserted
// here explicitly against the converge case).
func TestLoop_NilOnIterationIsNoOp(t *testing.T) {
	wf := loadFixture(t)
	l := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 1.0}), 5, 3)
	l.OnIteration = nil
	out, err := l.Run(wf, "balanced")
	if err != nil || !out.Converged || out.Reason != "converged" {
		t.Fatalf("nil hook must not change behavior; got %+v err=%v", out, err)
	}
}

// Resume must begin at StartIter and seed ResumePrev so the stale/tripwire math
// is continuous across the resume boundary. Here the loop resumes at iteration 4
// with the prior completion 0.6; a FLAT first post-resume reading (0.6) must
// count as stale (cur <= prev), and with NoProgress=1 it trips on that very
// reading — proving prev was restored, not reset to the -1.0 fresh sentinel.
func TestLoop_ResumeSeedsPrevForTripwire(t *testing.T) {
	wf := loadFixture(t)
	obs := &iterObs{}
	l := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.6}), 10, 1)
	l.StartIter, l.ResumePrev = 4, 0.6
	l.OnIteration = obs.hook()
	out, _ := l.Run(wf, "balanced")
	// First resumed iter (index 4): cur(0.6) <= prev(0.6) -> stale=1 >= 1 -> trips.
	if out.Converged || out.Reason != "no-progress tripwire (anti doom-loop)" || out.Iterations != 4 {
		t.Errorf("resumed prev must make a flat reading trip immediately; got %+v", out)
	}
	if want := []int{4}; !eqInts(obs.iters, want) {
		t.Errorf("resume must start at iteration 4; OnIteration indices = %v, want %v", obs.iters, want)
	}
}

// Resume numbering: the loop continues at StartIter and runs through MaxIter,
// so a resume at 3 with MaxIter 5 executes iterations 3,4,5 — never replaying
// the already-completed 1,2.
func TestLoop_ResumeStartsAtIndex(t *testing.T) {
	wf := loadFixture(t)
	obs := &iterObs{}
	l := loopOver(signalSeq(
		converge.Signals{RoadmapCompletion: 0.4},
		converge.Signals{RoadmapCompletion: 0.6},
		converge.Signals{RoadmapCompletion: 0.8},
	), 5, 9)
	l.StartIter, l.ResumePrev = 3, 0.2
	l.OnIteration = obs.hook()
	out, _ := l.Run(wf, "balanced")
	if out.Iterations != 5 || out.Reason != "max-iterations safety bound" {
		t.Errorf("resume at 3 with max 5 must end at the bound (iter 5); got %+v", out)
	}
	if want := []int{3, 4, 5}; !eqInts(obs.iters, want) {
		t.Errorf("resume must run iters 3,4,5 only; got %v", obs.iters)
	}
}

// StartIter of 0 or 1 is the fresh default: both begin at iteration 1 with the
// -1.0 sentinel, so a flat first reading is NOT counted as stale. Asserting both
// values pins the loopStart contract that back-compat depends on.
func TestLoop_FreshStartIterDefaults(t *testing.T) {
	wf := loadFixture(t)
	for _, start := range []int{0, 1} {
		obs := &iterObs{}
		l := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.5}), 10, 1)
		l.StartIter = start // ResumePrev left 0.0; must be IGNORED on a fresh run.
		l.OnIteration = obs.hook()
		out, _ := l.Run(wf, "balanced")
		// First iter: cur(0.5) <= prev(-1.0)? no -> not stale. Second: trips.
		if out.Iterations != 2 || out.Reason != "no-progress tripwire (anti doom-loop)" {
			t.Errorf("StartIter=%d must behave as a fresh run (trip on 2nd flat iter); got %+v", start, out)
		}
		if want := []int{1, 2}; !eqInts(obs.iters, want) {
			t.Errorf("StartIter=%d must start at iteration 1; got %v", start, obs.iters)
		}
	}
}

func eqInts(a, b []int) bool {
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}
