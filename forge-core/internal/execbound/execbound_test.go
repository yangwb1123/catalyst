package execbound

import (
	"context"
	"errors"
	"os/exec"
	"strings"
	"testing"
	"time"
)

// Validate rejects the three config errors: negative timeout, negative cap,
// and the ambiguous Unbounded && Timeout > 0 combination. The zero value (safe
// defaults) and Unbounded alone must pass.
func TestOptions_Validate(t *testing.T) {
	cases := []struct {
		name string
		opts Options
		want bool
	}{
		{"zero value passes", Options{}, true},
		{"negative timeout", Options{Timeout: -1 * time.Second}, false},
		{"negative max bytes", Options{MaxOutputBytes: -1}, false},
		{"unbounded+timeout ambiguous", Options{Unbounded: true, Timeout: 5 * time.Second}, false},
		{"unbounded alone", Options{Unbounded: true}, true},
		{"explicit timeout", Options{Timeout: 5 * time.Second}, true},
		{"explicit cap", Options{MaxOutputBytes: 4096}, true},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			err := tc.opts.Validate()
			if (err == nil) != tc.want {
				t.Errorf("Validate() = %v, want err-nil=%v", err, tc.want)
			}
		})
	}
}

// A clean single-process command exits 0 with its output retained and no
// CtxErr — the byte-for-byte happy path.
func TestRun_CleanExit(t *testing.T) {
	res := Run(context.Background(), []string{"echo", "hello"}, Options{}, CaptureCombined, Spec{})
	if res.Err != nil {
		t.Fatalf("clean exit must have no run error: %v", res.Err)
	}
	if res.CtxErr != nil {
		t.Fatalf("clean exit must have no ctx error: %v", res.CtxErr)
	}
	if got := strings.TrimSpace(res.Rendered()); got != "hello" {
		t.Errorf("Rendered() = %q, want %q", got, "hello")
	}
	if res.TimedOut() {
		t.Error("clean exit must not report TimedOut")
	}
}

// A non-zero exit surfaces as the run error (an *exec.ExitError), not a
// timeout and not a panic.
func TestRun_NonZeroExit(t *testing.T) {
	res := Run(context.Background(), []string{"sh", "-c", "echo boom; exit 3"}, Options{}, CaptureCombined, Spec{})
	if res.Err == nil {
		t.Fatal("non-zero exit must carry a run error")
	}
	if res.TimedOut() {
		t.Error("non-zero exit must not be a timeout")
	}
	if !strings.Contains(res.Rendered(), "boom") {
		t.Errorf("output must be retained on failure; got %q", res.Rendered())
	}
}

// A missing binary fails with exec.ErrNotFound wrapped, never a panic.
func TestRun_MissingBinary(t *testing.T) {
	res := Run(context.Background(), []string{"forge-no-such-binary-xyz"}, Options{}, CaptureCombined, Spec{})
	if res.Err == nil {
		t.Fatal("missing binary must carry a run error")
	}
	if !errors.Is(res.Err, exec.ErrNotFound) {
		t.Errorf("missing binary must surface exec.ErrNotFound; got %v", res.Err)
	}
}

// Spec.Dir anchors the child's cwd.
func TestRun_SpecDir(t *testing.T) {
	res := Run(context.Background(), []string{"pwd"}, Options{}, CaptureCombined, Spec{Dir: "/"})
	if res.Err != nil {
		t.Fatalf("pwd in /: %v", res.Err)
	}
	if got := strings.TrimSpace(res.Rendered()); got != "/" {
		t.Errorf("Spec.Dir not applied: cwd = %q, want /", got)
	}
}

// Unbounded means NO deadline: a command sleeping past the safe default's
// would-be trip survives (escape is real), while a parent cancel still kills.
func TestRun_Unbounded_NoDeadline(t *testing.T) {
	start := time.Now()
	res := Run(context.Background(), []string{"sleep", "1"}, Options{Unbounded: true}, CaptureCombined, Spec{})
	if res.Err != nil {
		t.Fatalf("unbounded sleep 1 must complete: %v", res.Err)
	}
	if elapsed := time.Since(start); elapsed > 3*time.Second {
		t.Errorf("unbounded run took %v — suspiciously long for sleep 1", elapsed)
	}
	if res.TimedOut() {
		t.Error("unbounded run must not report TimedOut")
	}
}

// A parent cancellation propagates even under Unbounded (A1.1): the child is
// killed, not left running.
func TestRun_Unbounded_ParentCancelStillKills(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan struct{})
	var res Result
	go func() {
		defer close(done)
		res = Run(ctx, []string{"sleep", "30"}, Options{Unbounded: true}, CaptureCombined, Spec{})
	}()
	time.Sleep(100 * time.Millisecond) // let the child spawn
	cancel()
	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("Run did not return after parent cancellation (unbounded must still cancel)")
	}
	if res.CtxErr == nil {
		t.Error("parent cancellation must surface in CtxErr")
	}
	if res.TimedOut() {
		t.Error("parent cancel before any deadline must not report TimedOut")
	}
}

// A timeout surfaces as TimedOut with the deadline cause.
func TestRun_Timeout_TimedOut(t *testing.T) {
	res := Run(context.Background(), []string{"sleep", "30"}, Options{Timeout: 300 * time.Millisecond}, CaptureCombined, Spec{})
	if !res.TimedOut() {
		t.Fatalf("must report TimedOut; CtxErr=%v Err=%v", res.CtxErr, res.Err)
	}
	if res.Err == nil {
		t.Error("a killed command must carry a run error")
	}
}

// GroupKillAvailable is a compile-time platform capability query; the value
// itself is platform-specific (asserted in the tagged tests).
func TestRun_GroupKillAvailable_Smoke(t *testing.T) {
	_ = GroupKillAvailable() // must not panic on any platform
}

// FromBytes applies the same cap+marker semantics to pre-captured bytes.
func TestFromBytes_CapsAndMarks(t *testing.T) {
	payload := strings.Repeat("x", 70000)
	res := FromBytes([]byte(payload), 65536)
	if res.Retained != 65536 {
		t.Errorf("Retained = %d, want 65536", res.Retained)
	}
	if res.Total != 70000 {
		t.Errorf("Total = %d, want 70000", res.Total)
	}
	if res.Merged != nil {
		t.Error("FromBytes must leave Merged nil (Observed/Rendered read Stdout)")
	}
	if got := res.Rendered(); !strings.Contains(got, "retained 65536 of 70000 bytes") {
		t.Errorf("Rendered() must carry the truncation marker; got %q", got)
	}
	if len(res.Stdout) != 65536 {
		t.Errorf("Stdout retained %d bytes, want cap 65536", len(res.Stdout))
	}
}

func TestFromBytes_UnderCap_NoMarker(t *testing.T) {
	res := FromBytes([]byte("small"), 65536)
	if res.Retained != 5 || res.Total != 5 {
		t.Errorf("Retained/Total = %d/%d, want 5/5", res.Retained, res.Total)
	}
	if strings.Contains(res.Rendered(), "truncated") {
		t.Error("under-cap FromBytes must not mark truncation")
	}
}
