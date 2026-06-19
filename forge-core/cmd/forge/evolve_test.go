package main

import (
	"os/exec"
	"strconv"
	"strings"
	"testing"
)

// These tests cover the third subsystem the central knob drives: `forge evolve`'s
// safety bound (--max-iter). When the operator does NOT pass --max-iter, the bound
// is the mode's evolve depth (explorer→opportunistic→2, engineering→thorough→10);
// an explicit --max-iter still wins (back-compat). They reuse main_test.go's
// fakeRepo + externalAgentWorkflow.
//
// The AUTHORITATIVE assertion is the run banner's "max-iter=N (source)": it is the
// resolved bound execLoop actually handed the engine (LoopEngine.MaxIter) plus its
// provenance, so it pins both the value and where it came from deterministically.
// The observed iteration count is a SECONDARY check and only used where it is safe:
// the dry executor never advances ROADMAP completion (flat 0%), so the anti-doom
// no-progress tripwire (fixed at 2 stale rounds) cleanly stops an external loop at
// iteration 3 regardless of a higher bound. Thus a count==N check is valid ONLY
// when N ≤ 3; above it the loop honestly stops at the tripwire, not the bound (the
// same reason main_test.go's evolve tests all use small --max-iter values).
const doomTripwireStop = 3 // external loop's flat-roadmap stop (NoProgress=2 → halts at iter 3)

// requirePython skips a test when python3 (the yaml2json shim) is unavailable, so
// the suite degrades cleanly off-box rather than failing on a missing transcoder.
func requirePython(t *testing.T) {
	t.Helper()
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
}

// Without --max-iter, the mode's evolve depth sets the bound: explorer's
// opportunistic → 2 iterations, engineering's thorough → 10. The banner must
// report the source as the mode default (not "explicit"), and the external-stop
// loop must actually run that many iterations.
func TestEvolve_MaxIterFromMode(t *testing.T) {
	requirePython(t)
	cases := []struct {
		mode     string
		wantIter int
	}{
		{"explorer", 2},     // opportunistic
		{"engineering", 10}, // thorough
	}
	for _, c := range cases {
		t.Run(c.mode, func(t *testing.T) {
			root := fakeRepo(t, "evolve", externalAgentWorkflow)
			var code int
			out := captureStdout(t, func() {
				// --lifecycle idea so production's veto never raises the floor;
				// this isolates the MODE's evolve depth as the sole driver.
				code = cmdEvolve([]string{"evolve", "--mode", c.mode, "--lifecycle", "idea", "--root", root})
			})
			if code != 0 {
				t.Fatalf("evolve --mode %s exit=%d, want 0 (clean external stop)\n%s", c.mode, code, out)
			}
			// AUTHORITATIVE: the banner carries the resolved bound AND attributes it
			// to the mode default — this is the value execLoop handed the engine.
			wantBanner := "max-iter=" + strconv.Itoa(c.wantIter) + " (mode=" + c.mode
			if !strings.Contains(out, wantBanner) {
				t.Errorf("banner missing %q (mode-derived max-iter); got:\n%s", wantBanner, out)
			}
			if !strings.Contains(out, "evolve-depth default") {
				t.Errorf("banner must attribute max-iter to the mode evolve-depth default; got:\n%s", out)
			}
			// SECONDARY: a flat-roadmap external loop stops at min(bound, tripwire).
			// For explorer (2 ≤ 3) that is the full bound; for engineering (10) it is
			// the tripwire (3) — either way it must never EXCEED the resolved bound.
			wantEnd := "ended after " + strconv.Itoa(minInt(c.wantIter, doomTripwireStop)) + " iter"
			if !strings.Contains(out, wantEnd) {
				t.Errorf("expected loop to stop at %q (min of mode bound %d and doom tripwire %d); got:\n%s",
					wantEnd, c.wantIter, doomTripwireStop, out)
			}
		})
	}
}

// An explicit --max-iter must OVERRIDE the mode default (back-compat): even with
// --mode engineering (whose thorough default is 10), --max-iter 3 wins, the banner
// says "explicit", and the loop runs exactly 3 iterations.
func TestEvolve_ExplicitMaxIterWins(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	var code int
	out := captureStdout(t, func() {
		code = cmdEvolve([]string{"evolve", "--mode", "engineering", "--max-iter", "3", "--root", root})
	})
	if code != 0 {
		t.Fatalf("evolve --max-iter 3 exit=%d, want 0\n%s", code, out)
	}
	if !strings.Contains(out, "max-iter=3 (explicit --max-iter=3)") {
		t.Errorf("explicit --max-iter must win over the mode default and say so; got:\n%s", out)
	}
	// It must NOT have silently used engineering's thorough default of 10.
	if strings.Contains(out, "max-iter=10") || strings.Contains(out, "ended after 10 iter") {
		t.Errorf("explicit --max-iter 3 was overridden by the mode default (10); got:\n%s", out)
	}
	if !strings.Contains(out, "ended after 3 iter") {
		t.Errorf("explicit --max-iter 3 should run exactly 3 iters; got:\n%s", out)
	}
}

// The production lifecycle veto reaches evolve depth too: explorer's opportunistic
// (2) is raised to the standard floor (5) under --lifecycle production, with no
// explicit --max-iter. This is the CLI-level proof of the production override on
// the evolve dimension.
func TestEvolve_ProductionRaisesMaxIter(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	var code int
	out := captureStdout(t, func() {
		code = cmdEvolve([]string{"evolve", "--mode", "explorer", "--lifecycle", "production", "--root", root})
	})
	if code != 0 {
		t.Fatalf("evolve explorer+production exit=%d, want 0\n%s", code, out)
	}
	// AUTHORITATIVE: explorer alone resolves to 2; production must raise the bound
	// to standard's 5, attributed to mode+lifecycle (not "explicit").
	if !strings.Contains(out, "max-iter=5 (mode=explorer lifecycle=production") {
		t.Errorf("production must raise explorer's opportunistic loop to standard (5); got:\n%s", out)
	}
	// SECONDARY: bound 5 > tripwire, so the flat-roadmap loop stops at iteration 3.
	if !strings.Contains(out, "ended after "+strconv.Itoa(doomTripwireStop)+" iter") {
		t.Errorf("explorer+production (bound 5) should stop at the doom tripwire (%d); got:\n%s", doomTripwireStop, out)
	}
}

// minInt returns the smaller of two ints (used to express min(bound, tripwire)).
func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}
