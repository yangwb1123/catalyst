package orchestrator

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

// --- DIRECTED RESTART ACROSS ITERATIONS (on_unmet) ---------------------------
//
// Split out of loop_test.go (file-size gate) as a self-contained concern: the
// on_unmet directed-restart behavior and its dedicated fixtures/helpers
// (phaseRecorder, threePhaseAgentWorkflow, loadThreePhase, unmetStop, countOf).
// It shares the package-level helpers that stay in loop_test.go (signalSeq,
// roadmapDone, allOK). Every symbol here is defined ONCE across the package.

// phaseRecorder is an AgentExecutor that records every agent phase it runs, in
// order, across all iterations — so a test can see WHICH phase each iteration
// started from and prove the on_unmet directed restart.
type phaseRecorder struct{ ran []string }

func (r *phaseRecorder) Execute(p asset.Phase, _ string) error {
	r.ran = append(r.ran, p.Name)
	return nil
}

// threePhaseAgentWorkflow is planner→implementer→qa, all AGENT phases (no gates),
// so every phase an iteration runs surfaces in phaseRecorder.ran. The stop is
// supplied by the test (with or without on_unmet) so the same shape exercises both
// the directed-restart and the back-compat replay paths.
const threePhaseAgentWorkflow = `{
  "stage": "build",
  "phases": [
    {"name": "planner", "agent": "planner", "readonly": true, "required_gates": []},
    {"name": "implementer", "agent": "implementer", "readonly": false, "required_gates": []},
    {"name": "qa", "agent": "qa", "readonly": true, "required_gates": []}
  ]
}`

func loadThreePhase(t *testing.T) asset.Workflow {
	t.Helper()
	wf, err := asset.LoadWorkflowJSON([]byte(threePhaseAgentWorkflow))
	if err != nil {
		t.Fatalf("load three-phase fixture: %v", err)
	}
	return wf
}

// unmetStop is a conjunction that NEVER converges (roadmap_completion == 100 with
// a signal stream stuck below 100), optionally carrying an on_unmet directive so
// subsequent iterations restart at target.
func unmetStop(onUnmet *asset.OnUnmet) asset.StopCondition {
	return asset.StopCondition{Type: "conjunction", AllOf: roadmapDone(), OnUnmet: onUnmet}
}

// ★ on_unmet directed restart ★: a never-converging conjunction whose on_unmet
// says loop_to_next_roadmap_item with target_phase=implementer. The FIRST
// iteration runs the whole workflow (planner→implementer→qa); every SUBSEQUENT
// iteration must begin at implementer (skipping planner) — so across 3 iterations
// planner runs ONCE while implementer and qa run three times each. Rising-but-
// incomplete progress avoids the tripwire so the loop runs to MaxIter=3.
func TestLoop_OnUnmetRestartsAtTargetPhase(t *testing.T) {
	wf := loadThreePhase(t)
	rec := &phaseRecorder{}
	eng := Engine{Exec: rec, RunGate: allOK}
	rising := signalSeq(
		converge.Signals{RoadmapCompletion: 0.3},
		converge.Signals{RoadmapCompletion: 0.6},
		converge.Signals{RoadmapCompletion: 0.9},
	)
	l := NewLoopEngine(eng,
		unmetStop(&asset.OnUnmet{Action: "loop_to_next_roadmap_item", TargetPhase: "implementer"}),
		rising, 3, 9, nil)

	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if out.Iterations != 3 || out.Reason != "max-iterations safety bound" {
		t.Fatalf("rising-but-incomplete must run to the bound; got %+v", out)
	}
	// planner once (iter 1 only), implementer & qa three times — the directed
	// restart skipped planner on iterations 2 and 3.
	if got := countOf(rec.ran, "planner"); got != 1 {
		t.Errorf("planner ran %d times, want 1 (directed restart skips it after iter 1); ran=%v", got, rec.ran)
	}
	if got := countOf(rec.ran, "implementer"); got != 3 {
		t.Errorf("implementer ran %d times, want 3 (every iteration restarts at it); ran=%v", got, rec.ran)
	}
	if got := countOf(rec.ran, "qa"); got != 3 {
		t.Errorf("qa ran %d times, want 3; ran=%v", got, rec.ran)
	}
	// And the exact order proves the shift: planner only leads iteration 1.
	want := []string{"planner", "implementer", "qa", "implementer", "qa", "implementer", "qa"}
	if strings.Join(rec.ran, ",") != strings.Join(want, ",") {
		t.Errorf("directed-restart order = %v, want %v", rec.ran, want)
	}
}

// BACK-COMPAT: with NO on_unmet, every iteration replays the WHOLE workflow —
// planner runs every iteration, byte-for-byte the pre-on_unmet behavior. Same
// never-converging stop, same rising signals, but no directive.
func TestLoop_NoOnUnmetReplaysWholeWorkflow(t *testing.T) {
	wf := loadThreePhase(t)
	rec := &phaseRecorder{}
	eng := Engine{Exec: rec, RunGate: allOK}
	rising := signalSeq(
		converge.Signals{RoadmapCompletion: 0.3},
		converge.Signals{RoadmapCompletion: 0.6},
		converge.Signals{RoadmapCompletion: 0.9},
	)
	l := NewLoopEngine(eng, unmetStop(nil), rising, 3, 9, nil)

	out, err := l.Run(wf, "balanced")
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if out.Iterations != 3 {
		t.Fatalf("expected 3 iterations; got %+v", out)
	}
	// Whole-workflow replay: planner runs every iteration (3 times).
	if got := countOf(rec.ran, "planner"); got != 3 {
		t.Errorf("with no on_unmet, planner must run every iteration (3); ran %d times: %v", got, rec.ran)
	}
}

// An on_unmet whose target_phase is unresolvable falls back to a whole-workflow
// replay (start phase 0) — never a panic, never a skipped run. planner runs every
// iteration, exactly as the no-on_unmet back-compat path.
func TestLoop_OnUnmetUnresolvableTargetReplays(t *testing.T) {
	wf := loadThreePhase(t)
	rec := &phaseRecorder{}
	eng := Engine{Exec: rec, RunGate: allOK}
	rising := signalSeq(
		converge.Signals{RoadmapCompletion: 0.3},
		converge.Signals{RoadmapCompletion: 0.6},
	)
	l := NewLoopEngine(eng,
		unmetStop(&asset.OnUnmet{Action: "loop_to_next_roadmap_item", TargetPhase: "nonexistent"}),
		rising, 2, 9, nil)

	if _, err := l.Run(wf, "balanced"); err != nil {
		t.Fatalf("Run: %v", err)
	}
	if got := countOf(rec.ran, "planner"); got != 2 {
		t.Errorf("an unresolvable on_unmet target must replay the whole workflow; planner ran %d times, want 2: %v", got, rec.ran)
	}
}

func countOf(ss []string, want string) int {
	n := 0
	for _, s := range ss {
		if s == want {
			n++
		}
	}
	return n
}
