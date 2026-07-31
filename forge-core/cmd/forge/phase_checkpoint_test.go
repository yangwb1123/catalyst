package main

import (
	"path/filepath"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/persist"
)

// phaseCheckpointHook is the MID-iteration write side of phase-granular resume. Its
// (Iteration:iter-1, PhaseIndex:nextPhaseIdx) write convention MUST pair exactly with
// resumeStart's (StartIter:Iteration+1, StartPhase:PhaseIndex) read convention — an
// off-by-one on EITHER side strands a resume. This drives the REAL hook THEN the REAL
// resumeStart and asserts the round-trip, so a +1/-1 mutation on either side turns it red
// (a guard a fake-hook test leaves open — the gap a fresh reviewer's mutation testing found).
func TestPhaseCheckpointHook_WriteResumeRoundTrips(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	o := runOpts{root: root, mode: "balanced"}
	wf := asset.Workflow{Stage: "evolve"}
	digest := checkpointWorkflowDigest(wf)
	// A prior COMPLETED-iteration checkpoint whose RoadmapCompletion the mid-iteration hook
	// must PRESERVE (a phase checkpoint has no fresh measurement of its own).
	if err := persist.Save(checkpointPath(root), persist.Checkpoint{
		Workflow: "evolve", WorkflowDigest: digest,
		Mode: "balanced", Lifecycle: "mvp", Iteration: 2, RoadmapCompletion: 0.6,
		Reason: "iteration complete", UpdatedAtUnix: 1_750_000_000,
	}, 0); err != nil {
		t.Fatalf("seed prior checkpoint: %v", err)
	}
	budget := &runBudget{}
	budget.seed(500_000) // 0.5 USD already billed this run — the finer mid-iteration spend.
	hook := phaseCheckpointHook(o, wf, budget, nil, func(string) {})
	if err := hook(3, 3, 3, 1); err != nil {
		t.Fatalf("write phase progress: %v", err)
	} // iteration 3 in progress; phase index 3 is the exact resume target.
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("phase checkpoint: found=%v err=%v", found, err)
	}
	// Write convention: last COMPLETED iteration = iter-1 = 2; exact next phase = 3.
	if cp.Iteration != 2 || cp.PhaseIndex != 3 {
		t.Errorf("phase checkpoint = {iter %d, phase %d}, want {2, 3} (iter-1 / phaseIdx+1)", cp.Iteration, cp.PhaseIndex)
	}
	assertPhaseCheckpointEnvelope(t, cp, digest)
	// Read convention pairs EXACTLY: resume re-enters iteration 3 at phase 3.
	start, prev, spent, phaseStart, _, _, err := resumeStart(root, true, checkpointBinding{
		Workflow: "evolve", WorkflowDigest: digest,
		Mode: "balanced", Lifecycle: "mvp", PhaseLimit: 3,
	})
	if err != nil {
		t.Fatalf("resumeStart: %v", err)
	}
	if start != 3 || phaseStart != 3 || prev != 0.6 || spent != 500_000 {
		t.Errorf("resume round-trip = (start %d, phase %d, prev %v, spent %d), want (3, 3, 0.6, 500000) — write/read conventions must pair", start, phaseStart, prev, spent)
	}
}

func TestPhaseCheckpointHookPersistsPreSpawnCounterAtPhaseZero(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".forge"))
	wf := asset.Workflow{Stage: "evolve"}
	hook := phaseCheckpointHook(
		runOpts{root: root, mode: "balanced"},
		wf, &runBudget{}, nil, func(string) {},
	)
	if err := hook(1, 0, 1, 0); err != nil {
		t.Fatalf("persist pre-spawn reservation: %v", err)
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found {
		t.Fatalf("load pre-spawn checkpoint: found=%v err=%v", found, err)
	}
	if cp.Iteration != 0 || cp.PhaseIndex != 0 || cp.AgentCalls != 1 {
		t.Fatalf("pre-spawn progress = iter %d phase %d calls %d, want 0/0/1",
			cp.Iteration, cp.PhaseIndex, cp.AgentCalls)
	}
}

func assertPhaseCheckpointEnvelope(t *testing.T, cp persist.Checkpoint, digest string) {
	t.Helper()
	if cp.Workflow != "evolve" || cp.WorkflowDigest != digest ||
		cp.Mode != "balanced" || cp.Lifecycle != "mvp" {
		t.Errorf("phase checkpoint binding = %q/%q/%q/%q, want evolve/<digest>/balanced/mvp",
			cp.Workflow, cp.WorkflowDigest, cp.Mode, cp.Lifecycle)
	}
	if cp.RoadmapCompletion != 0.6 {
		t.Errorf("must PRESERVE the last completed iteration's RoadmapCompletion 0.6; got %v", cp.RoadmapCompletion)
	}
	if cp.SpentUsdMicros != 500_000 {
		t.Errorf("must record the FINER mid-iteration spend; got %d want 500000", cp.SpentUsdMicros)
	}
	if cp.AgentCalls != 3 || cp.LoopBacks != 1 || cp.MaxLoopBacks != maxLoopBack {
		t.Errorf("phase checkpoint counters = calls %d, loop-backs %d/%d; want 3, 1/%d",
			cp.AgentCalls, cp.LoopBacks, cp.MaxLoopBacks, maxLoopBack)
	}
}
