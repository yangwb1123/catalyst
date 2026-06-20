package main

import (
	"bytes"
	"encoding/json"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/persist"
	"forgeos/forge-core/internal/trace"
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

// fakeRepo builds a self-contained repo root in a temp dir with the bits the CLI
// needs to load a workflow without the real ForgeOS tree: a stub yaml2json.py
// that emits the given workflow JSON, the workflow yml (content unused — the stub
// ignores it), and an empty .agent/agents dir. It returns the root.
func fakeRepo(t *testing.T, name, workflowJSON string) string {
	t.Helper()
	root := t.TempDir()
	mkdir(t, filepath.Join(root, "harness"))
	mkdir(t, filepath.Join(root, ".agent", "workflows"))
	mkdir(t, filepath.Join(root, ".agent", "agents"))
	// The stub transcoder ignores its argument and prints the workflow JSON, so
	// loadWorkflow's `python3 yaml2json.py <yml>` yields our fixture deterministically.
	shim := "import sys\nsys.stdout.write(" + pyQuote(workflowJSON) + ")\n"
	writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
	writeFile(t, filepath.Join(root, ".agent", "workflows", name+".yml"), "stub: true\n")
	return root
}

// externalAgentWorkflow is an external-stop workflow with a single agent phase
// and NO gate phases, so the dry executor runs it with no node/harness probe and
// it reaches the safety bound cleanly (the expected external-stop outcome).
const externalAgentWorkflow = `{
  "stage": "evolve",
  "phases": [{"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []}],
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

	// No --resume: fresh sentinel, no IO, no error.
	if start, prev, err := resumeStart(root, false); err != nil || start != 0 || prev != -1.0 {
		t.Errorf("no-resume = (%d,%v,%v), want (0,-1,nil)", start, prev, err)
	}
	// --resume, no checkpoint file present: tolerated as a fresh start.
	if start, prev, err := resumeStart(root, true); err != nil || start != 0 || prev != -1.0 {
		t.Errorf("resume+missing = (%d,%v,%v), want (0,-1,nil)", start, prev, err)
	}
	// --resume with a present, valid checkpoint: continue at Iteration+1, seed prev.
	cp := persist.Checkpoint{Workflow: "evolve", Iteration: 5, RoadmapCompletion: 0.6}
	if err := persist.Save(checkpointPath(root), cp); err != nil {
		t.Fatalf("seed checkpoint: %v", err)
	}
	if start, prev, err := resumeStart(root, true); err != nil || start != 6 || prev != 0.6 {
		t.Errorf("resume+valid = (%d,%v,%v), want (6,0.6,nil)", start, prev, err)
	}
	// --resume with a MALFORMED checkpoint: hard error, no silent from-scratch.
	writeFile(t, checkpointPath(root), "{not valid json")
	if start, _, err := resumeStart(root, true); err == nil || start != 0 {
		t.Errorf("resume+malformed must error out (got start=%d err=%v)", start, err)
	}
}

// checkpointHook is the per-iteration persistence+trace point. Invoking the
// returned closure must write the iteration's snapshot to <root>/.forge and emit
// a matching "iteration" trace event carrying the measured signals.
func TestCheckpointHook_PersistsAndTraces(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	var logs []string
	o := runOpts{root: root, mode: "balanced"}
	wf := asset.Workflow{Stage: "evolve"}
	hook := checkpointHook(o, wf, trace.NewTracer(&buf), func(s string) { logs = append(logs, s) })

	hook(2, converge.Signals{RoadmapCompletion: 0.75, GatesGreen: true})

	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("checkpoint: found=%v err=%v", found, err)
	}
	if cp.Iteration != 2 || cp.RoadmapCompletion != 0.75 || !cp.GatesGreen || cp.Mode != "balanced" {
		t.Errorf("checkpoint = %+v, want iter 2 / 0.75 / green / balanced", cp)
	}
	if cp.UpdatedAtUnix == 0 || cp.UpdatedAtUnix > time.Now().Unix()+5 {
		t.Errorf("UpdatedAtUnix = %d, want a recent main-injected timestamp", cp.UpdatedAtUnix)
	}
	ev := lastTraceEvent(t, buf.String())
	if ev.Kind != "iteration" || ev.Name != "2" || ev.Status != "ok" {
		t.Errorf("trace event = %+v, want iteration/2/ok", ev)
	}
	if !strings.Contains(ev.Detail, "roadmap=75%") {
		t.Errorf("trace detail = %q, want measured signals", ev.Detail)
	}
}

// The OnIteration hook must ALSO append one memory entry per round — the run
// trajectory recorded for cross-session recall. It is a KindLesson on the stage
// topic, carrying this round's measured signals and iteration number. (Honesty:
// this records the real dry-run trajectory, not a fabricated agent finding.)
func TestCheckpointHook_AppendsMemoryEntry(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	var buf bytes.Buffer
	hook := checkpointHook(runOpts{root: root, mode: "balanced"}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), func(string) {})

	hook(3, converge.Signals{RoadmapCompletion: 0.4, GatesGreen: false})

	entries, err := memory.Load(memoryPath(root))
	if err != nil {
		t.Fatalf("load memory: %v", err)
	}
	if len(entries) != 1 {
		t.Fatalf("hook should append exactly one memory entry, got %d", len(entries))
	}
	e := entries[0]
	if e.Kind != memory.KindLesson || e.Topic != "evolve" || e.Iteration != 3 {
		t.Errorf("entry = %+v, want KindLesson / topic evolve / iter 3", e)
	}
	if !strings.Contains(e.Detail, "roadmap=40%") || !strings.Contains(e.Detail, "gates_green=false") {
		t.Errorf("entry detail = %q, want the round's measured trajectory", e.Detail)
	}
	if e.CreatedAtUnix == 0 {
		t.Errorf("entry must carry a main-injected timestamp; got %d", e.CreatedAtUnix)
	}
}

// Fail-closed honesty: when the checkpoint write fails, the loop must NOT pretend
// it succeeded — the trace event status flips to a failure marker and a loud
// warning is logged, rather than silently dropping the recovery state.
func TestCheckpointHook_WriteFailureIsSurfaced(t *testing.T) {
	root := t.TempDir()
	// Make checkpointPath unwritable by planting a DIRECTORY where the file (and
	// its .tmp sibling) must go, so persist.Save's open/rename fails.
	mkdir(t, filepath.Join(root, ".forge"))
	mkdir(t, checkpointPath(root))
	var buf bytes.Buffer
	var logs []string
	hook := checkpointHook(runOpts{root: root}, asset.Workflow{Stage: "evolve"},
		trace.NewTracer(&buf), func(s string) { logs = append(logs, s) })

	hook(1, converge.Signals{RoadmapCompletion: 0.1})

	ev := lastTraceEvent(t, buf.String())
	if ev.Status != "checkpoint-write-failed" {
		t.Errorf("trace status = %q, want checkpoint-write-failed on a failed Save", ev.Status)
	}
	if !containsSub(logs, "checkpoint write failed") {
		t.Errorf("a failed checkpoint write must be logged loudly; logs=%v", logs)
	}
}

// openTracer must create <root>/.forge and APPEND to trace.jsonl across calls, so
// a --resume continues the same audit trail instead of truncating prior history.
func TestOpenTracer_CreatesDirAndAppends(t *testing.T) {
	root := t.TempDir()
	tr, closeFn, err := openTracer(root)
	if err != nil {
		t.Fatalf("openTracer: %v", err)
	}
	if err := tr.Emit(trace.Event{Kind: "iteration", Name: "1", Status: "ok"}); err != nil {
		t.Fatalf("emit: %v", err)
	}
	closeFn()
	// Second open (simulating a resume) must append, not truncate.
	tr2, closeFn2, err := openTracer(root)
	if err != nil {
		t.Fatalf("openTracer #2: %v", err)
	}
	if err := tr2.Emit(trace.Event{Kind: "iteration", Name: "2", Status: "ok"}); err != nil {
		t.Fatalf("emit #2: %v", err)
	}
	closeFn2()
	data, err := os.ReadFile(filepath.Join(root, ".forge", "trace.jsonl"))
	if err != nil {
		t.Fatalf("read trace: %v", err)
	}
	if n := bytes.Count(data, []byte("\n")); n != 2 {
		t.Errorf("trace has %d lines, want 2 (append preserved the first across reopen)", n)
	}
}

// --- test helpers (evolve group) ---------------------------------------------

// pyQuote renders s as a Python string literal (JSON's quoting is a compatible
// subset for our ASCII workflow fixtures), so the stub transcoder can emit it.
func pyQuote(s string) string {
	b, _ := json.Marshal(s)
	return string(b)
}

// lastTraceEvent decodes the final JSONL record from a trace buffer.
func lastTraceEvent(t *testing.T, jsonl string) trace.Event {
	t.Helper()
	lines := strings.Split(strings.TrimRight(jsonl, "\n"), "\n")
	if len(lines) == 0 || lines[0] == "" {
		t.Fatalf("no trace events emitted; buf=%q", jsonl)
	}
	var ev trace.Event
	if err := json.Unmarshal([]byte(lines[len(lines)-1]), &ev); err != nil {
		t.Fatalf("decode trace line: %v", err)
	}
	return ev
}

func containsSub(ss []string, sub string) bool {
	for _, s := range ss {
		if strings.Contains(s, sub) {
			return true
		}
	}
	return false
}
