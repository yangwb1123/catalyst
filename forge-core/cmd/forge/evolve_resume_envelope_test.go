package main

import (
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/persist"
)

func TestPrepareLoopResumeRestoresFrozenResourceEnvelope(t *testing.T) {
	root := scanEvidenceRepo(t)
	mkdir(t, filepath.Join(root, ".forge"))
	wf := evolveScanWorkflow()
	if err := persist.Save(checkpointPath(root), persist.Checkpoint{
		Workflow: "evolve", WorkflowDigest: checkpointWorkflowDigest(wf),
		Mode: "engineering", Lifecycle: "mvp",
		Iteration: 2, RoadmapCompletion: 0.4,
		Reason: "iteration complete", UpdatedAtUnix: 1_750_000_000,
		SpentUsdMicros: 700_000, BudgetCapMicros: 1_500_000,
		MaxAgentCalls: 5, MaxLoopBacks: maxLoopBack,
	}, 0); err != nil {
		t.Fatal(err)
	}
	o := runOpts{
		root: root, mode: "engineering", lifecycle: "mvp",
		runFlagsCaptured: true,
	}
	state, err := prepareLoopResume(wf, &o, true)
	if err != nil {
		t.Fatalf("prepare resume: %v", err)
	}
	if !state.found || state.start != 3 || o.maxAgentCalls != 5 {
		t.Fatalf("restored state=%+v max-agent-calls=%d", state, o.maxAgentCalls)
	}
	budget, err := newRunBudget(o.runBudgetUSD)
	if err != nil {
		t.Fatal(err)
	}
	if err := restoreLoopBudget(budget, state); err != nil {
		t.Fatal(err)
	}
	if budget.CapUsdMicros() != 1_500_000 || budget.SpentUsdMicros() != 700_000 {
		t.Fatalf("restored budget cap=%d spent=%d",
			budget.CapUsdMicros(), budget.SpentUsdMicros())
	}
}

func TestRestoreResumeRunOptionsRejectsExplicitResourceConflicts(t *testing.T) {
	state := loopResumeState{
		found: true, budgetCapMicros: 1_500_000,
		maxAgentCalls: 5, maxLoopBacks: maxLoopBack,
	}
	tests := []struct {
		name string
		opts runOpts
		want string
	}{
		{
			name: "agent calls",
			opts: runOpts{
				runFlagsCaptured: true, maxAgentCallsExplicit: true,
				maxAgentCalls: 4,
			},
			want: "persisted --max-agent-calls=5",
		},
		{
			name: "run budget",
			opts: runOpts{
				runFlagsCaptured: true, runBudgetExplicit: true,
				runBudgetUSD: "2",
			},
			want: "persisted run budget cap",
		},
		{
			name: "explicit zero budget",
			opts: runOpts{
				runFlagsCaptured: true, runBudgetExplicit: true,
				runBudgetUSD: "0",
			},
			want: "persisted run budget cap",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			err := restoreResumeRunOptions(&tc.opts, state)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("resource conflict error = %v, want %q", err, tc.want)
			}
		})
	}
}

func TestApplyLoopResumeSeedsSameIterationCounters(t *testing.T) {
	loop := orchestrator.LoopEngine{}
	state := loopResumeState{
		found: true, start: 4, prev: 0.6, phaseStart: 2,
		gatesGreen: true, agentCalls: 3, loopBacks: 1,
	}
	wf := evolveScanWorkflow()
	wf.Phases[0].ScanContract = ""
	if err := applyLoopResume(&loop, wf, state); err != nil {
		t.Fatal(err)
	}
	if loop.StartIter != 4 || loop.StartPhase != 2 ||
		loop.Engine.InitialAgentCalls != 3 || loop.Engine.InitialLoopBacks != 1 {
		t.Fatalf("loop resume seeds = iter %d phase %d calls %d loop-backs %d",
			loop.StartIter, loop.StartPhase,
			loop.Engine.InitialAgentCalls, loop.Engine.InitialLoopBacks)
	}
}

func TestValidateResumeCheckpointRejectsUnreachableResourceProgress(t *testing.T) {
	base := persist.Checkpoint{
		FormatVersion: persist.CheckpointFormatCurrent,
		Workflow:      "evolve", WorkflowDigest: "digest",
		Mode: "engineering", Lifecycle: "mvp",
		Iteration: 1, RoadmapCompletion: 0.4,
		Reason: "durable phase progress", UpdatedAtUnix: 1_750_000_000,
		PhaseIndex: 1, MaxLoopBacks: maxLoopBack,
	}
	binding := checkpointBinding{
		Workflow: "evolve", WorkflowDigest: "digest",
		Mode: "engineering", Lifecycle: "mvp", PhaseLimit: 3,
	}
	if err := validateResumeCheckpoint(base, binding); err == nil ||
		!strings.Contains(err.Error(), "zero agent_calls") {
		t.Fatalf("zero-call phase progress error = %v", err)
	}
	base.PhaseIndex, base.AgentCalls, base.LoopBacks = 0, 1, 2
	if err := validateResumeCheckpoint(base, binding); err == nil ||
		!strings.Contains(err.Error(), "exceeds recorded agent_calls") {
		t.Fatalf("impossible loop-back progress error = %v", err)
	}
}
