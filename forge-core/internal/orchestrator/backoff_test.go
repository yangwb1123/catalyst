package orchestrator

import (
	"context"
	"testing"
	"time"
)

// TestRunAgentPhase_OverloadBackoffHonorsContextCancellation is a regression
// test: e.sleep used to block for the FULL overload backoff duration
// regardless of ctx, so a SIGINT arriving mid-backoff wasn't honored until
// the sleep completed and runAgentPhase's next-attempt loop got a chance to
// notice cancellation. It must now return promptly once ctx is cancelled,
// well before the (deliberately long, uncapped-by-MaxRetries-here) backoff
// would otherwise elapse.
func TestRunAgentPhase_OverloadBackoffHonorsContextCancellation(t *testing.T) {
	wf := loadAgentOnly(t)
	ctx, cancel := context.WithCancel(context.Background())
	go func() {
		time.Sleep(20 * time.Millisecond) // well into the first (2s) backoff
		cancel()
	}()

	// No Sleep injected: this exercises the REAL context-cancellable wait
	// (time.NewTimer + ctx.Done race), not the fake test-only sleep path.
	exec := &seqExecutor{errs: []error{overloadedErr(), overloadedErr(), overloadedErr()}}
	eng := Engine{Exec: exec, RunGate: allOK, Log: func(string) {}, MaxRetries: 5, Ctx: ctx}

	start := time.Now()
	err := eng.Run(wf, "balanced")
	elapsed := time.Since(start)

	if err == nil {
		t.Fatal("expected an error after context cancellation mid-backoff, got nil")
	}
	// The first backoff alone (overloadBackoff(0)) is 2s; a correctly
	// cancellable sleep returns within tens of milliseconds of cancel(), so a
	// generous 1s ceiling still fails loudly if the old blocking behavior
	// regresses, without making the test flaky under CI scheduling jitter.
	if elapsed > time.Second {
		t.Errorf("RunFrom took %s to return after ctx cancellation mid-backoff, want well under the 2s backoff (cancellation not honored promptly)", elapsed)
	}
}
