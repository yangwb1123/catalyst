package orchestrator

import (
	"errors"
	"testing"

	"forgeos/forge-core/internal/converge"
)

// A checkpoint at MaxInt-1 resumes at MaxInt. Completing that final allowed
// iteration must stop at the bound without i++ wrapping the loop index negative.
func TestLoop_MaxIntBoundDoesNotOverflow(t *testing.T) {
	wf := loadFixture(t)
	maxInt := int(^uint(0) >> 1)
	obs := &iterObs{}
	l := loopOver(signalSeq(converge.Signals{RoadmapCompletion: 0.5}), maxInt, 1)
	l.StartIter = maxInt
	l.OnIteration = obs.hook()

	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("MaxInt boundary run: %v", err)
	}
	if out.Converged || out.Reason != "max-iterations safety bound" || out.Iterations != maxInt {
		t.Fatalf("MaxInt boundary outcome = %+v, want one final iteration at the bound", out)
	}
	if want := []int{maxInt}; !eqInts(obs.iters, want) {
		t.Fatalf("MaxInt boundary iterations = %v, want %v (no signed wrap)", obs.iters, want)
	}
}

func TestLoop_NegativeBoundFailsClosed(t *testing.T) {
	l := loopOver(signalSeq(converge.Signals{}), -1, 1)
	out, err := l.Run(loadFixture(t), "balanced")
	if err == nil || out.Converged || out.Iterations != 0 {
		t.Fatalf("negative bound outcome=%+v err=%v, want pre-run rejection", out, err)
	}
}

func TestLoop_OnIterationErrorStopsBeforeNextRound(t *testing.T) {
	wf := loadAgentOnly(t)
	exec := &countingExec{}
	hookErr := errors.New("iteration checkpoint failed")
	hookCalls := 0
	l := NewLoopEngine(
		Engine{Exec: exec, RunGate: allOK}, wf.Stop,
		signalSeq(converge.Signals{}), 3, 3, nil,
	)
	l.OnIteration = func(_ int, _ converge.Signals, _ int64) error {
		hookCalls++
		return hookErr
	}
	out, err := l.Run(wf, "balanced")
	if !errors.Is(err, hookErr) || out.Reason != "iteration checkpoint failure" {
		t.Fatalf("iteration checkpoint outcome=%+v err=%v", out, err)
	}
	if out.Iterations != 1 || exec.calls != 1 || hookCalls != 1 {
		t.Fatalf("iteration failure did not stop: outcome=%+v exec=%d hooks=%d", out, exec.calls, hookCalls)
	}
}
