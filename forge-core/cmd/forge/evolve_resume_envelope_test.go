package main

import (
	"encoding/json"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/persist"
)

func TestPrepareLoopResumeRestoresFrozenResourceEnvelope(t *testing.T) {
	root := scanEvidenceRepo(t)
	mkdir(t, filepath.Join(root, ".forge"))
	wf := evolveScanWorkflow()
	if err := persist.Save(checkpointPath(root), persist.Checkpoint{
		Workflow: "evolve", WorkflowDigest: checkpointWorkflowDigest(wf),
		Mode: "engineering", Lifecycle: "mvp", Materiality: "materiality_not_bound",
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

func TestApplyLoopResumeRestoresAllFeedForwardOutputsInTraversalOrder(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "scan", FeedsForward: true}, {Name: "gap"},
		{Name: "roadmap-update", FeedsForward: true}, {Name: "implement"},
	}}
	var observed []string
	exec := orchestrator.CommandExecutor{
		ValidateOutput: func(string, string) error { return nil },
		ObserveSemantic: func(phase, output string) {
			observed = append(observed, phase+"="+output)
		},
	}
	loop := orchestrator.LoopEngine{Engine: orchestrator.Engine{Exec: exec}}
	semantics := map[string]string{"scan": "scan-exact", "roadmap-update": "roadmap-exact"}
	receipts := map[string]outputbinding.AgentOutputReceipt{
		"scan":           semanticReceipt("scan", "scan-exact"),
		"roadmap-update": semanticReceipt("roadmap-update", "roadmap-exact"),
	}
	state := loopResumeState{phaseStart: 3, phaseSemantics: semantics, phaseReceipts: receipts}
	if err := applyLoopResume(&loop, wf, state); err != nil {
		t.Fatal(err)
	}
	if strings.Join(observed, ",") != "scan=scan-exact,roadmap-update=roadmap-exact" {
		t.Fatalf("restored outputs = %v", observed)
	}
}

func TestReceiptRestoreDoesNotReinterpretClaudeShapedSemanticJSON(t *testing.T) {
	const semantic = `{"result":"inner"}`
	var rawObserved, semanticObserved string
	exec := orchestrator.CommandExecutor{
		ValidateOutput:  func(string, string) error { return nil },
		Observe:         func(_ string, output string, _ time.Duration) { rawObserved = unwrapClaudeResult(output) },
		ObserveSemantic: func(_ string, output string) { semanticObserved = output },
	}
	phase := asset.Phase{Name: "roadmap-update"}
	if err := exec.RestoreValidatedOutput(phase, semantic, semanticReceipt(phase.Name, semantic)); err != nil {
		t.Fatal(err)
	}
	if rawObserved != "" || semanticObserved != semantic {
		t.Fatalf("receipt restore raw=%q semantic=%q", rawObserved, semanticObserved)
	}
}

func TestRecoveredFeedForwardMatchesLiveProviderNormalization(t *testing.T) {
	for _, isClaude := range []bool{false, true} {
		for _, semantic := range []string{`{"result":"inner"}`, " \x01 payload \n"} {
			live, recovered := newPhaseOutputLedger(), newPhaseOutputLedger()
			raw := semantic
			if isClaude {
				encoded, err := json.Marshal(map[string]any{"result": semantic})
				if err != nil {
					t.Fatal(err)
				}
				raw = string(encoded)
			}
			feeds := func(string) bool { return true }
			observeFor(isClaude, nil, nil, live, feeds, nil, nil, nil)("plan", raw, 0)
			semanticObserveFor(isClaude, recovered, feeds)("plan", semantic)
			liveValue, _ := live.output("plan")
			recoveredValue, _ := recovered.output("plan")
			if liveValue != recoveredValue {
				t.Fatalf("claude=%v semantic=%q live=%q recovered=%q",
					isClaude, semantic, liveValue, recoveredValue)
			}
		}
	}
}

func semanticReceipt(phase, output string) outputbinding.AgentOutputReceipt {
	return outputbinding.AgentOutputReceipt{
		Phase: phase, SemanticOutputBytes: int64(len(output)),
		SemanticOutputSHA256: outputbinding.SHA256([]byte(output)),
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
		Mode: "engineering", Lifecycle: "mvp", Materiality: "materiality_not_bound",
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
