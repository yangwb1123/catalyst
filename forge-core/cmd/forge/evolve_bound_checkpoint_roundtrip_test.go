package main

import (
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/persist"
)

func TestBoundEvolveCheckpointRoundTripsValidatedFeedForward(t *testing.T) {
	fixture := newBoundEvolveCheckpointFixture(t)
	state, err := loadLoopResumeState(fixture.root, true, fixture.binding)
	if err != nil {
		t.Fatal(err)
	}
	if len(state.phaseReceipts) != 3 || len(state.phaseSemantics) != 2 {
		t.Fatalf("recovery refs=%d semantics=%d, want 3/2",
			len(state.phaseReceipts), len(state.phaseSemantics))
	}
	restarted := restartOutputBindingRuntime(fixture.runtime)
	restoreCheckpointReceipts(restarted, state.phaseReceipts, state.receiptHead, state.phaseSemantics)
	recovered := newPhaseOutputLedger()
	exec := orchestrator.CommandExecutor{
		ValidateOutput: phaseOutputContractWithPolicy(fixture.root, fixture.wf, fixture.policy),
		ObserveSemantic: semanticObserveFor(false, recovered, feedsForwardOf(fixture.wf),
			verdictContractOf(fixture.wf), scanContractOf(fixture.wf)),
	}
	loop := orchestrator.LoopEngine{Engine: orchestrator.Engine{Exec: exec}}
	if err := applyLoopResume(&loop, fixture.wf, state); err != nil {
		t.Fatal(err)
	}
	assertRestoredPhaseOutput(t, fixture.live, recovered, "scan")
	assertRestoredPhaseOutput(t, fixture.live, recovered, "roadmap-update")
	if len(restarted.accepted) != 3 || restarted.journalHead != state.receiptHead {
		t.Fatalf("runtime receipt recovery count/head = %d/%q", len(restarted.accepted), restarted.journalHead)
	}
}

func TestBoundEvolveCheckpointRejectsNonExactSemanticAndReceiptMaps(t *testing.T) {
	fixture := newBoundEvolveCheckpointFixture(t)
	base, found, err := persist.Load(checkpointPath(fixture.root))
	if err != nil || !found {
		t.Fatalf("load base checkpoint: found=%v err=%v", found, err)
	}
	roadmapKey, gapKey := "evolve/roadmap-update", "evolve/gap-analysis"
	for _, test := range []struct {
		name   string
		mutate func(*persist.Checkpoint)
	}{
		{"missing semantic", func(cp *persist.Checkpoint) { delete(cp.PhaseSemanticOutputs, roadmapKey) }},
		{"extra semantic", func(cp *persist.Checkpoint) { cp.PhaseSemanticOutputs[gapKey] = "extra" }},
		{"tampered semantic", func(cp *persist.Checkpoint) { cp.PhaseSemanticOutputs[roadmapKey] += "tamper" }},
		{"missing receipt", func(cp *persist.Checkpoint) { delete(cp.PhaseReceipts, gapKey) }},
		{"extra receipt", func(cp *persist.Checkpoint) { cp.PhaseReceipts["evolve/implement"] = cp.PhaseReceipts[gapKey] }},
		{"tampered receipt", func(cp *persist.Checkpoint) { cp.PhaseReceipts[gapKey] = strings.Repeat("0", 64) }},
	} {
		t.Run(test.name, func(t *testing.T) {
			cp := cloneRecoveryCheckpoint(base)
			test.mutate(&cp)
			if err := persist.Save(checkpointPath(fixture.root), cp, 0); err != nil {
				t.Fatalf("save adversarial checkpoint: %v", err)
			}
			if _, err := loadLoopResumeState(fixture.root, true, fixture.binding); err == nil {
				t.Fatal("non-exact bound recovery checkpoint was accepted")
			}
		})
	}
}

type boundEvolveCheckpointFixture struct {
	root    string
	wf      asset.Workflow
	policy  mode.Policy
	binding checkpointBinding
	runtime *outputBindingRuntime
	live    *phaseOutputLedger
}

func newBoundEvolveCheckpointFixture(t *testing.T) boundEvolveCheckpointFixture {
	t.Helper()
	root := scanEvidenceRepo(t)
	runBindingGit(t, root, "init", "-q")
	runBindingGit(t, root, "add", "evidence")
	runBindingGit(t, root, "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
		"commit", "-qm", "fixture")
	wf := boundEvolveRecoveryWorkflow()
	o := runOpts{root: root, mode: "engineering", lifecycle: "mvp", materiality: "L2", agentCmd: "agent"}
	policy := mode.Effective(o.mode, o.lifecycle)
	runtime := newOutputBindingRuntime(
		outputBindingWorkflowInfo{root: root, runID: "evolve-run", wf: wf},
		outputBindingExecutionInfo{opts: o, policy: policy}, priorEmitsOf(wf),
	)
	mkdir(t, filepath.Join(root, ".agent", "workflows"))
	writeBindingWorkflow(t, runtime)
	live := newPhaseOutputLedger()
	outputs := []string{scanReport(t, evolvescan.DepthThorough, true), "ranked gaps",
		"TASK_LIST:\n- [ ] T001: roadmap exact — acceptance: pass — files: docs/fake.md — depends_on: none — model: sonnet — roadmap: v2"}
	observe := observeFor(false, nil, nil, live, feedsForwardOf(wf), nil, nil, nil,
		verdictContractOf(wf), scanContractOf(wf))
	for index, phase := range wf.Phases[:3] {
		commitRecoveryPhase(t, runtime, phase, outputs[index])
		observe(phase.Name, outputs[index], 0)
	}
	hook := phaseCheckpointHook(o, wf, &runBudget{}, live, func(string) {}, runtime)
	if err := hook(1, 3, 3, 0); err != nil {
		t.Fatalf("persist bound phase checkpoint: %v", err)
	}
	return boundEvolveCheckpointFixture{
		root: root, wf: wf, policy: policy, runtime: runtime, live: live,
		binding: checkpointBinding{
			Workflow: wf.Stage, WorkflowDigest: checkpointWorkflowDigest(wf), WorkflowAsset: wf,
			Mode: o.mode, Lifecycle: o.lifecycle, Materiality: o.materiality,
			MaterialityExplicit: true, PhaseLimit: len(wf.Phases),
		},
	}
}

func boundEvolveRecoveryWorkflow() asset.Workflow {
	return asset.Workflow{
		Stage: "evolve", OutputBindingContract: asset.OutputBindingContractLocalDigestV1,
		Phases: []asset.Phase{
			{Name: "scan", Agent: "explorer", Readonly: true, Effect: "observe",
				FeedsForward: true, ScanContract: asset.ScanContractEvolveV1},
			{Name: "gap-analysis", Agent: "architect", Readonly: true, Effect: "propose"},
			{Name: "roadmap-update", Agent: "planner", Readonly: true, Effect: "propose", FeedsForward: true},
			{Name: "implement", Agent: "implementer", Effect: "mutate"},
		},
	}
}

func commitRecoveryPhase(t *testing.T, runtime *outputBindingRuntime, phase asset.Phase, semantic string) {
	t.Helper()
	if err := runtime.prepare(phase, runtime.opts.mode); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, runtime.opts.mode, []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	if err := runtime.commit(phase.Name, semantic, semantic, 0); err != nil {
		t.Fatal(err)
	}
}

func assertRestoredPhaseOutput(t *testing.T, live, recovered *phaseOutputLedger, phase string) {
	t.Helper()
	want, wantOK := live.output(phase)
	got, gotOK := recovered.output(phase)
	if !wantOK || !gotOK || got != want {
		t.Fatalf("phase %s live/recovered = %q/%q (present %v/%v)", phase, want, got, wantOK, gotOK)
	}
}

func cloneRecoveryCheckpoint(cp persist.Checkpoint) persist.Checkpoint {
	clone := cp
	clone.PhaseReceipts = cloneStringMap(cp.PhaseReceipts)
	clone.PhaseSemanticOutputs = cloneStringMap(cp.PhaseSemanticOutputs)
	clone.StageReceipts = cloneStringMap(cp.StageReceipts)
	clone.ApprovalContexts = cloneStringMap(cp.ApprovalContexts)
	return clone
}

func cloneStringMap(source map[string]string) map[string]string {
	result := make(map[string]string, len(source))
	for key, value := range source {
		result[key] = value
	}
	return result
}
