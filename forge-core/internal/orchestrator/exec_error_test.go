package orchestrator

import (
	"context"
	"errors"
	"fmt"
	"os/exec"
	"testing"
)

// KindOverloaded is the transient capacity fault added for 529/overload resilience: it
// must be retryable (so the engine re-attempts) and render as "overloaded" in logs. These
// two assertions pin the contract the backoff path and the log line depend on.
func TestKindOverloaded_RetryableAndString(t *testing.T) {
	e := &ExecError{Phase: "implementer", Kind: KindOverloaded}
	if !e.Retryable() {
		t.Error("KindOverloaded must be Retryable (a transient overload may succeed after a backoff)")
	}
	if got := e.Kind.String(); got != "overloaded" {
		t.Errorf("KindOverloaded.String() = %q, want \"overloaded\"", got)
	}
}

// The existing retryable kind (timeout) and the non-retryable kinds must keep their verdicts,
// so adding KindOverloaded did not perturb the rest of the Retryable contract.
func TestRetryable_KindMatrixUnchanged(t *testing.T) {
	cases := map[ExecKind]bool{
		KindTimeout:        true,  // transient, pre-existing
		KindOverloaded:     true,  // transient, new
		KindConfig:         false, // permanent
		KindFailed:         false, // agent's own verdict
		KindRecursionLimit: false, // permanent guard
	}
	for kind, want := range cases {
		if got := (&ExecError{Kind: kind}).Retryable(); got != want {
			t.Errorf("Retryable(%s) = %v, want %v", kind, got, want)
		}
	}
}

// classifyRunErr's new isOverload tail parameter: a PLAIN non-zero exit (an *exec.ExitError,
// no deadline) with isOverload=true must classify as KindOverloaded — proving the caller's
// transient verdict routes a would-be KindFailed onto the retryable path.
func TestClassifyRunErr_IsOverloadTrueOnPlainExitIsOverloaded(t *testing.T) {
	exitErr := runFalse(t) // a real *exec.ExitError (non-zero exit), no deadline
	got := classifyRunErr("implementer", exitErr, nil, true)
	if got.Kind != KindOverloaded {
		t.Errorf("plain ExitError + isOverload=true: want KindOverloaded, got %s", got.Kind)
	}
	if !errors.Is(got, exitErr) {
		t.Error("overloadErr must wrap the underlying run error so errors.Is reaches it")
	}
}

// TIMEOUT WINS OVER OVERLOAD: a deadline-exceeded run with isOverload=true (e.g. a truncated
// SIGKILL dump that incidentally tripped the caller's overload detector) must STILL be a
// KindTimeout — the deadline is the real cause, retried immediately not after a backoff. This
// is the decisive proof of the switch ordering (deadline checked before isOverload).
func TestClassifyRunErr_TimeoutPrecedesOverload(t *testing.T) {
	got := classifyRunErr("implementer", errors.New("signal: killed"), context.DeadlineExceeded, true)
	if got.Kind != KindTimeout {
		t.Errorf("DeadlineExceeded + isOverload=true: want KindTimeout (deadline wins), got %s", got.Kind)
	}
}

// notfound (permanent config) still precedes overload: a missing binary with isOverload=true is
// KindConfig, never a transient retry — a permanent fault must never be upgraded to retryable.
func TestClassifyRunErr_NotFoundPrecedesOverload(t *testing.T) {
	got := classifyRunErr("implementer", fmt.Errorf("wrap: %w", exec.ErrNotFound), nil, true)
	if got.Kind != KindConfig {
		t.Errorf("ErrNotFound + isOverload=true: want KindConfig (permanent wins), got %s", got.Kind)
	}
}

// isOverload=false preserves the ORIGINAL classification exactly: a plain non-zero exit stays
// KindFailed. This is the byte-for-byte back-compat assertion — the new parameter is inert when false.
func TestClassifyRunErr_IsOverloadFalseIsFailed(t *testing.T) {
	exitErr := runFalse(t)
	got := classifyRunErr("implementer", exitErr, nil, false)
	if got.Kind != KindFailed {
		t.Errorf("plain ExitError + isOverload=false: want KindFailed (unchanged), got %s", got.Kind)
	}
}

// overloadBackoff is the pure exponential schedule: base<<attempt, capped. Asserts the exact
// deterministic sequence (2s,4s,8s,…) and the cap saturation, so the backoff is verifiable
// without real time and a regression in the curve is caught.
func TestOverloadBackoff_ExponentialThenCapped(t *testing.T) {
	cases := map[int]string{
		0:   "2s",
		1:   "4s",
		2:   "8s",
		3:   "16s",
		4:   "32s",
		5:   "1m0s", // 64s would exceed the 60s cap -> saturates
		10:  "1m0s", // well past the cap
		100: "1m0s", // overflow-guard path saturates, never wraps negative
		-1:  "2s",   // defensive: negative attempt floors to the base
	}
	for attempt, want := range cases {
		if got := overloadBackoff(attempt).String(); got != want {
			t.Errorf("overloadBackoff(%d) = %s, want %s", attempt, got, want)
		}
	}
	// The cap must never be exceeded for any plausible attempt.
	for attempt := 0; attempt < 200; attempt++ {
		if d := overloadBackoff(attempt); d > overloadBackoffCap || d <= 0 {
			t.Fatalf("overloadBackoff(%d) = %s out of (0, cap] bounds", attempt, d)
		}
	}
}

// runFalse runs the `false` command and returns the resulting *exec.ExitError (a clean non-zero
// exit, no deadline) — the real run error a KindFailed/KindOverloaded classification keys on.
func runFalse(t *testing.T) error {
	t.Helper()
	err := exec.Command("false").Run()
	if err == nil {
		t.Fatal("`false` must exit non-zero")
	}
	return err
}
