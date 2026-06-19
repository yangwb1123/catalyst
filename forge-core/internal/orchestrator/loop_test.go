package orchestrator

import (
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

func ptr(f float64) *float64 { return &f }

// roadmapDone is the conjunction criterion "roadmap_completion == 100".
func roadmapDone() []asset.Criterion {
	return []asset.Criterion{{Metric: "roadmap_completion", Operator: "==", Threshold: ptr(100)}}
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
		"conjunction", roadmapDone(), sig, maxIter, noProgress, nil)
}

func TestLoop_Converges(t *testing.T) {
	wf := loadFixture(t)
	out, err := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 1.0}), 5, 3).Run(wf, "balanced")
	if err != nil || !out.Converged || out.Reason != "converged" {
		t.Fatalf("expected converged; got %+v err=%v", out, err)
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
		"external", nil,
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

// P7: a stale external-stop loop reports no_gaps_found semantics — a clean stop,
// never the conjunction tripwire.
func TestLoop_ExternalStopNoGapsFound(t *testing.T) {
	wf := loadFixture(t)
	l := NewLoopEngine(
		Engine{Exec: DryRunExecutor{}, RunGate: allOK},
		"external", nil,
		signalSeq(converge.Signals{RoadmapCompletion: 0.5}),
		10, 2, nil)
	out, _ := l.Run(wf, "balanced")
	if !out.Converged || out.Reason != "no gaps found (external stop)" {
		t.Errorf("stale external stop must map to no_gaps_found clean stop; got %+v", out)
	}
}
