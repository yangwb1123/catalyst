package main

import (
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"

	"forgeos/forge-core/internal/persist"
)

// These tests cover the third subsystem the central knob drives: `forge evolve`'s
// safety bound (--max-iter). When the operator does NOT pass --max-iter, the bound
// is the mode's evolve depth (explorer→opportunistic→2, engineering→thorough→10);
// an explicit --max-iter still wins (back-compat). They reuse this file's
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
// same reason this file's other evolve tests all use small --max-iter values).
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

func TestEvolve_NegativeMaxIterFailsBeforeRunState(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-iter", "-1"}); code != 2 {
		t.Fatalf("negative --max-iter exit=%d, want usage error 2", code)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("negative bound created run state before rejection: %v", err)
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

// --- resilience wiring: timeout / checkpoint / resume / trace -----------------

// fakeRepo builds a self-contained repo with a natively loadable JSON-as-YAML
// workflow plus a legacy shim retained for tests of the ordinary fallback path.
func fakeRepo(t *testing.T, name, workflowJSON string) string {
	t.Helper()
	root := t.TempDir()
	mkdir(t, filepath.Join(root, "harness"))
	mkdir(t, filepath.Join(root, ".agent", "workflows"))
	mkdir(t, filepath.Join(root, ".agent", "agents"))
	shim := "import sys\nsys.stdout.write(" + pyQuote(workflowJSON) + ")\n"
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	writeFile(t, filepath.Join(root, ".agent", "workflows", name+".yml"), workflowJSON)
	return root
}

// externalAgentWorkflow is an external-stop workflow with a single agent phase
// and NO gate phases, so the dry executor runs it with no node/harness probe and
// it reaches the safety bound cleanly (the expected external-stop outcome).
const externalAgentWorkflow = `{
  "stage": "evolve",
  "phases": [{"name": "implementer", "agent": "implementer", "readonly": false, "effect": "mutate", "required_gates": []}],
  "stop_condition": {"type": "external", "all_of": [], "anti_pattern": "round_count"}
}`

// humanGateLoopWorkflow is a human_gate workflow carrying a fully SATISFIABLE
// all_of (roadmap_completion == 0, true with no ROADMAP -> 0%) — the exact shape
// that bypassed the gate before the fix (evolve drove it, the loop evaluated the
// all_of, and it exited 0 "converged" with NO approval).
const humanGateLoopWorkflow = `{
  "stage": "design",
  "phases": [{"name": "solution-architect", "agent": "architect", "readonly": true, "required_gates": []}],
  "stop_condition": {"type": "human_gate", "human_approval": "required",
    "all_of": [{"metric": "roadmap_completion", "operator": "==", "threshold": 0}],
    "on_approved": {"next_stage": "build"}}
}`

// THE evolve-path security regression test. `forge evolve` on a human_gate must
// FAIL CLOSED (non-zero exit) and never enter the loop — the satisfied all_of that
// produced the pre-fix exit-0 "converged" bypass is now refused outright, so no
// .forge/ run artifacts are written (the loop never starts).
func TestEvolve_HumanGateFailsClosed(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := fakeRepo(t, "design", humanGateLoopWorkflow)
	// No --approved, no marker, and a satisfiable all_of: the pre-fix bypass setup.
	code := cmdEvolve([]string{"design", "--root", root, "--max-iter", "3"})
	if code == 0 {
		t.Fatalf("BYPASS: forge evolve on an unapproved human_gate exited 0 (converged); must fail closed")
	}
	if code != 1 {
		t.Errorf("forge evolve on a human_gate must fail closed with exit 1; got %d", code)
	}
	// The loop must never have started: no checkpoint, no trace under .forge.
	if _, found, _ := persist.Load(checkpointPath(root)); found {
		t.Error("a refused human_gate evolve must not write a checkpoint (loop never ran)")
	}
	if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); err == nil {
		t.Error("a refused human_gate evolve must not write a trace (loop never ran)")
	}
	// Even WITH --approved the refusal stands: a human_gate's single human approval
	// belongs to `forge run`, never to an autonomous loop that would re-drive it.
	if code := cmdEvolve([]string{"design", "--root", root, "--max-iter", "3", "--approved"}); code != 1 {
		t.Errorf("forge evolve must refuse a human_gate even with --approved; got exit %d", code)
	}
}

// --timeout must parse on both run and evolve (DurationVar): a bad duration is a
// parse error (exit 2), a good one is accepted and the command proceeds.
func TestEvolve_TimeoutFlagParses(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	// Valid duration + dry executor + max-iter 1 => one clean external-stop iter.
	code := cmdEvolve([]string{"evolve", "--root", root, "--timeout", "30s", "--max-iter", "1"})
	if code != 0 {
		t.Errorf("evolve --timeout 30s should run an external-stop loop to a clean stop; exit=%d", code)
	}
	// A malformed duration must be rejected at flag parse (exit 2), not ignored.
	if code := cmdEvolve([]string{"evolve", "--root", root, "--timeout", "notaduration"}); code != 2 {
		t.Errorf("malformed --timeout must be a parse error; exit=%d, want 2", code)
	}
}

// A full evolve run must materialize <root>/.forge with a checkpoint at the last
// iteration and a non-empty trace; a follow-up --resume must then continue from
// the persisted iteration+1 (proving the write->resume round-trip end to end).
func TestEvolve_WritesCheckpointAndResumes(t *testing.T) {
	if _, err := exec.LookPath("python3"); err != nil {
		t.Skip("python3 not available")
	}
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-iter", "3"}); code != 0 {
		t.Fatalf("first evolve run exit=%d, want 0 (external stop)", code)
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("checkpoint after run: found=%v err=%v", found, err)
	}
	if cp.Iteration != 3 || cp.Workflow != "evolve" {
		t.Errorf("checkpoint = %+v, want last iteration 3 of workflow evolve", cp)
	}
	if info, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); err != nil || info.Size() == 0 {
		t.Errorf("trace.jsonl should exist and be non-empty; err=%v", err)
	}
	// Resume must pick up at cp.Iteration+1 (=4). With max-iter 3 there is nothing
	// left to do, so it ends immediately at the bound without rerunning 1..3.
	if code := cmdEvolve([]string{"evolve", "--root", root, "--max-iter", "3", "--resume"}); code != 0 {
		t.Errorf("resume run exit=%d, want 0", code)
	}
}

// resumeStart is the fail-closed gate for --resume. Three paths: no --resume is a
// fresh run; a missing checkpoint with --resume is tolerated (fresh, reported); a
// MALFORMED checkpoint with --resume is a hard error — never a silent restart.
func TestResumeStart_Paths(t *testing.T) {
	root := t.TempDir()
	binding := checkpointBinding{
		Workflow: "evolve", WorkflowDigest: "workflow-digest",
		Mode: "balanced", Lifecycle: "mvp", PhaseLimit: 3,
	}
	// No --resume: fresh sentinel, no IO, no error, zero-spend seed, phase 0, gates false.
	if start, prev, spent, phase, gg, err := resumeStart(root, false, binding); err != nil || start != 0 || prev != -1.0 || spent != 0 || phase != 0 || gg != false {
		t.Errorf("no-resume = (%d,%v,%d,%d,%v,%v), want (0,-1,0,0,false,nil)", start, prev, spent, phase, gg, err)
	}
	// --resume, no checkpoint file present: tolerated as a fresh start (zero spend, phase 0).
	if start, prev, spent, phase, gg, err := resumeStart(root, true, binding); err != nil || start != 0 || prev != -1.0 || spent != 0 || phase != 0 || gg != false {
		t.Errorf("resume+missing = (%d,%v,%d,%d,%v,%v), want (0,-1,0,0,false,nil)", start, prev, spent, phase, gg, err)
	}
	// --resume with a present, valid ITERATION-BOUNDARY checkpoint (PhaseIndex 0): continue
	// at Iteration+1, seed prev AND the persisted spend, phase 0 (replay the iteration in
	// full), AND GatesGreen (regression: this used to be silently dropped, so a resumed
	// loop's stale detector always started as if gates were red regardless of the checkpoint).
	cp := persist.Checkpoint{
		Workflow: "evolve", WorkflowDigest: "workflow-digest",
		Mode: "balanced", Lifecycle: "mvp", Iteration: 5,
		RoadmapCompletion: 0.6, SpentUsdMicros: 1_250_000, GatesGreen: true,
		Reason: "iteration complete", UpdatedAtUnix: 1_750_000_000,
	}
	if err := persist.Save(checkpointPath(root), cp, 0); err != nil {
		t.Fatalf("seed checkpoint: %v", err)
	}
	if start, prev, spent, phase, gg, err := resumeStart(root, true, binding); err != nil || start != 6 || prev != 0.6 || spent != 1_250_000 || phase != 0 || gg != true {
		t.Errorf("resume+valid = (%d,%v,%d,%d,%v,%v), want (6,0.6,1250000,0,true,nil)", start, prev, spent, phase, gg, err)
	}
	// --resume with a MID-ITERATION checkpoint (PhaseIndex > 0): resume re-enters the
	// in-progress iteration AT that phase (phase-granular), not from phase 0.
	mid := persist.Checkpoint{
		Workflow: "evolve", WorkflowDigest: "workflow-digest",
		Mode: "balanced", Lifecycle: "mvp", Iteration: 5,
		RoadmapCompletion: 0.6, PhaseIndex: 3, SpentUsdMicros: 2_000_000,
		Reason: "phase complete", UpdatedAtUnix: 1_750_000_001,
	}
	if err := persist.Save(checkpointPath(root), mid, 0); err != nil {
		t.Fatalf("seed mid checkpoint: %v", err)
	}
	if start, _, spent, phase, _, err := resumeStart(root, true, binding); err != nil || start != 6 || phase != 3 || spent != 2_000_000 {
		t.Errorf("resume+mid-iteration = (start %d, phase %d, spent %d, %v), want (6,3,2000000,nil)", start, phase, spent, err)
	}
	// --resume with a MALFORMED checkpoint: hard error, no silent from-scratch.
	writeFile(t, checkpointPath(root), "{not valid json")
	if start, _, _, _, _, err := resumeStart(root, true, binding); err == nil || start != 0 {
		t.Errorf("resume+malformed must error out (got start=%d err=%v)", start, err)
	}
}

func TestResumeStart_RejectsBindingDriftAndInvalidState(t *testing.T) {
	binding := checkpointBinding{
		Workflow: "evolve", WorkflowDigest: "workflow-digest",
		Mode: "explorer", Lifecycle: "idea", PhaseLimit: 3,
	}
	valid := persist.Checkpoint{
		FormatVersion: persist.CheckpointFormatCurrent,
		Workflow:      "evolve", WorkflowDigest: "workflow-digest",
		Mode: "explorer", Lifecycle: "idea",
		Iteration: 2, RoadmapCompletion: 0.5, PhaseIndex: 2, SpentUsdMicros: 10,
		Reason: "phase complete", UpdatedAtUnix: 1_750_000_000,
	}
	tests := []struct {
		name string
		edit func(*persist.Checkpoint)
		want string
	}{
		{"missing workflow", func(cp *persist.Checkpoint) { cp.Workflow = "" }, "lacks required"},
		{"missing mode", func(cp *persist.Checkpoint) { cp.Mode = "" }, "lacks required"},
		{"legacy missing lifecycle", func(cp *persist.Checkpoint) { cp.Lifecycle = "" }, "legacy checkpoints"},
		{"legacy format", func(cp *persist.Checkpoint) { cp.FormatVersion = "forgeos.checkpoint.v1" }, "diagnostic-only"},
		{"missing workflow digest", func(cp *persist.Checkpoint) { cp.WorkflowDigest = "" }, "workflow digest"},
		{"missing reason", func(cp *persist.Checkpoint) { cp.Reason = "" }, "recovery metadata"},
		{"missing update time", func(cp *persist.Checkpoint) { cp.UpdatedAtUnix = 0 }, "recovery metadata"},
		{"workflow mismatch", func(cp *persist.Checkpoint) { cp.Workflow = "build" }, "workflow mismatch"},
		{"workflow digest mismatch", func(cp *persist.Checkpoint) { cp.WorkflowDigest = "other" }, "digest mismatch"},
		{"mode mismatch", func(cp *persist.Checkpoint) { cp.Mode = "engineering" }, "mode mismatch"},
		{"lifecycle mismatch", func(cp *persist.Checkpoint) { cp.Lifecycle = "production" }, "lifecycle mismatch"},
		{"negative iteration", func(cp *persist.Checkpoint) { cp.Iteration = -1 }, "non-negative"},
		{"iteration overflow", func(cp *persist.Checkpoint) { cp.Iteration = int(^uint(0) >> 1) }, "incremented safely"},
		{"negative roadmap", func(cp *persist.Checkpoint) { cp.RoadmapCompletion = -0.1 }, "within [0,1]"},
		{"roadmap above one", func(cp *persist.Checkpoint) { cp.RoadmapCompletion = 1.1 }, "within [0,1]"},
		{"negative phase", func(cp *persist.Checkpoint) { cp.PhaseIndex = -1 }, "executable range"},
		{"phase beyond policy cutoff", func(cp *persist.Checkpoint) { cp.PhaseIndex = 4 }, "executable range"},
		{"negative spend", func(cp *persist.Checkpoint) { cp.SpentUsdMicros = -1 }, "non-negative"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			cp := valid
			tc.edit(&cp)
			if err := validateResumeCheckpoint(cp, binding); err == nil ||
				!strings.Contains(err.Error(), tc.want) {
				t.Fatalf("validateResumeCheckpoint error = %v, want %q", err, tc.want)
			}
		})
	}
}

func TestEvolve_CheckpointPolicyBindingEndToEnd(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	firstArgs := []string{
		"evolve", "--root", root, "--mode", "engineering",
		"--lifecycle", "growth", "--max-iter", "1",
	}
	if code := cmdEvolve(firstArgs); code != 0 {
		t.Fatalf("first evolve exit=%d", code)
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("load checkpoint: found=%v err=%v", found, err)
	}
	if cp.Workflow != "evolve" || cp.Mode != "engineering" || cp.Lifecycle != "growth" {
		t.Fatalf("checkpoint binding = %q/%q/%q, want evolve/engineering/growth",
			cp.Workflow, cp.Mode, cp.Lifecycle)
	}
	if code := cmdEvolve([]string{
		"evolve", "--root", root, "--mode", "engineering",
		"--lifecycle", "growth", "--max-iter", "2", "--resume",
	}); code != 0 {
		t.Fatalf("same-policy resume exit=%d, want 0", code)
	}
	cp, found, err = persist.Load(checkpointPath(root))
	if err != nil || !found || cp.Iteration != 2 {
		t.Fatalf("same-policy resume did not advance: checkpoint=%+v found=%v err=%v", cp, found, err)
	}
}

func TestEvolve_ResumePolicyMismatchFailsBeforeTraceOrCheckpointMutation(t *testing.T) {
	requirePython(t)
	root := fakeRepo(t, "evolve", externalAgentWorkflow)
	if code := cmdEvolve([]string{
		"evolve", "--root", root, "--mode", "engineering",
		"--lifecycle", "growth", "--max-iter", "1",
	}); code != 0 {
		t.Fatalf("first evolve exit=%d", code)
	}
	cpBefore, err := os.ReadFile(checkpointPath(root))
	if err != nil {
		t.Fatal(err)
	}
	tracePath := filepath.Join(root, ".forge", "trace.jsonl")
	traceBefore, err := os.ReadFile(tracePath)
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct {
		name string
		args []string
	}{
		{"mode", []string{"--mode", "balanced", "--lifecycle", "growth"}},
		{"lifecycle", []string{"--mode", "engineering", "--lifecycle", "production"}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			args := []string{"evolve", "--root", root, "--max-iter", "1", "--resume"}
			args = append(args, tc.args...)
			if code := cmdEvolve(args); code != 1 {
				t.Fatalf("mismatched resume exit=%d, want 1", code)
			}
			cpAfter, _ := os.ReadFile(checkpointPath(root))
			traceAfter, _ := os.ReadFile(tracePath)
			if string(cpAfter) != string(cpBefore) {
				t.Fatal("mismatched resume modified checkpoint")
			}
			if string(traceAfter) != string(traceBefore) {
				t.Fatal("mismatched resume opened/appended the trace before rejection")
			}
		})
	}
}

func TestEvolve_ResumeRejectsForeignAndLegacyCheckpointBeforeTrace(t *testing.T) {
	requirePython(t)
	for _, tc := range []struct {
		name string
		cp   persist.Checkpoint
	}{
		{"foreign workflow", persist.Checkpoint{
			Workflow: "build", WorkflowDigest: "foreign-workflow",
			Mode: "balanced", Lifecycle: "mvp",
			Reason: "iteration complete", UpdatedAtUnix: 1_750_000_000,
		}},
		{"legacy missing lifecycle", persist.Checkpoint{
			FormatVersion: "forgeos.checkpoint.v1",
			Workflow:      "evolve", Mode: "balanced",
		}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			root := fakeRepo(t, "evolve", externalAgentWorkflow)
			if err := persist.Save(checkpointPath(root), tc.cp, 0); err != nil {
				t.Fatal(err)
			}
			if code := cmdEvolve([]string{
				"evolve", "--root", root, "--mode", "balanced",
				"--lifecycle", "mvp", "--max-iter", "1", "--resume",
			}); code != 1 {
				t.Fatalf("unsafe resume exit=%d, want 1", code)
			}
			if _, err := os.Stat(filepath.Join(root, ".forge", "trace.jsonl")); !os.IsNotExist(err) {
				t.Fatalf("unsafe resume created trace before rejection: %v", err)
			}
		})
	}
}

// --- test helpers (evolve group) ---------------------------------------------

// pyQuote renders s as a Python string literal (JSON's quoting is a compatible
// subset for our ASCII workflow fixtures), so the stub transcoder can emit it.
func pyQuote(s string) string {
	b, _ := json.Marshal(s)
	return string(b)
}
