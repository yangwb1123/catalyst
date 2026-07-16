package main

import (
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/persist"
)

// phaseCheckpointHook is the MID-iteration write side of phase-granular resume. Its
// (Iteration:iter-1, PhaseIndex:phaseIdx+1) write convention MUST pair exactly with
// resumeStart's (StartIter:Iteration+1, StartPhase:PhaseIndex) read convention — an
// off-by-one on EITHER side strands a resume. This drives the REAL hook THEN the REAL
// resumeStart and asserts the round-trip, so a +1/-1 mutation on either side turns it red
// (a guard a fake-hook test leaves open — the gap a fresh reviewer's mutation testing found).
func TestPhaseCheckpointHook_WriteResumeRoundTrips(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	o := runOpts{root: root, mode: "balanced"}
	wf := asset.Workflow{Stage: "evolve"}
	// A prior COMPLETED-iteration checkpoint whose RoadmapCompletion the mid-iteration hook
	// must PRESERVE (a phase checkpoint has no fresh measurement of its own).
	if err := persist.Save(checkpointPath(root), persist.Checkpoint{
		Workflow: "evolve", Mode: "balanced", Iteration: 2, RoadmapCompletion: 0.6,
	}, 0); err != nil {
		t.Fatalf("seed prior checkpoint: %v", err)
	}
	budget := &runBudget{}
	budget.seed(500_000) // 0.5 USD already billed this run — the finer mid-iteration spend.
	hook := phaseCheckpointHook(o, wf, budget, func(string) {})
	hook(3, 2) // iteration 3 in progress; agent phase index 2 just completed.

	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("phase checkpoint: found=%v err=%v", found, err)
	}
	// Write convention: last COMPLETED iteration = iter-1 = 2; next phase = phaseIdx+1 = 3.
	if cp.Iteration != 2 || cp.PhaseIndex != 3 {
		t.Errorf("phase checkpoint = {iter %d, phase %d}, want {2, 3} (iter-1 / phaseIdx+1)", cp.Iteration, cp.PhaseIndex)
	}
	if cp.RoadmapCompletion != 0.6 {
		t.Errorf("must PRESERVE the last completed iteration's RoadmapCompletion 0.6; got %v", cp.RoadmapCompletion)
	}
	if cp.SpentUsdMicros != 500_000 {
		t.Errorf("must record the FINER mid-iteration spend; got %d want 500000", cp.SpentUsdMicros)
	}
	// Read convention pairs EXACTLY: resume re-enters iteration 3 at phase 3.
	start, prev, spent, phaseStart, err := resumeStart(root, true)
	if err != nil {
		t.Fatalf("resumeStart: %v", err)
	}
	if start != 3 || phaseStart != 3 || prev != 0.6 || spent != 500_000 {
		t.Errorf("resume round-trip = (start %d, phase %d, prev %v, spent %d), want (3, 3, 0.6, 500000) — write/read conventions must pair", start, phaseStart, prev, spent)
	}
}
