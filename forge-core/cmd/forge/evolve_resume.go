package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/materiality"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/persist"
)

type loopResumeState struct {
	found           bool
	start           int
	prev            float64
	spentMicros     int64
	budgetCapMicros int64
	phaseStart      int
	gatesGreen      bool
	scanReport      string
	scanSemantic    string
	maxAgentCalls   int
	agentCalls      int
	maxLoopBacks    int
	loopBacks       int
	materiality     string
	runID           string
	receiptHead     string
	phaseReceipts   map[string]outputbinding.AgentOutputReceipt
	phaseSemantics  map[string]string
	scanReceipt     *outputbinding.AgentOutputReceipt
}

func parseEvolveFlags(fs *flag.FlagSet, args []string) int {
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if fs.NArg() != 0 {
		fmt.Fprintln(os.Stderr, "forge evolve: unexpected trailing positional arguments")
		return 2
	}
	return 0
}

func freezeEvolveRunOptions(fs *flag.FlagSet, o *runOpts) error {
	if err := freezeRunMateriality(fs, o); err != nil {
		return fmt.Errorf("--materiality: %w", err)
	}
	return validateSandboxMemory(o.sandboxMemoryMB)
}

func validateEvolveEntry(
	wf asset.Workflow, o runOpts, fs *flag.FlagSet, requestedMaxIter int,
) (int, string, int) {
	if converge.IsHumanGate(wf.Stop) {
		return 0, "", rejectHumanGate(wf.Stage, o.root)
	}
	if wf.Stage != "evolve" {
		fmt.Fprintf(os.Stderr, "forge evolve: workflow stage must be %q (got %q)\n",
			"evolve", wf.Stage)
		return 0, "", 1
	}
	iter, source := resolveMaxIter(fs, requestedMaxIter, o)
	if iter < 0 {
		fmt.Fprintf(os.Stderr, "forge evolve: --max-iter must be non-negative (got %d)\n", iter)
		return 0, "", 2
	}
	return iter, source, 0
}

func loadEvolveWorkflow(root, name string, o runOpts) (asset.Workflow, error) {
	if name == "build" && (o.materiality == "L3" || o.materiality == "L4") {
		return loadWorkflowNativeOnly(root, name)
	}
	policy := mode.Effective(o.mode, o.lifecycle)
	if policy.BuildHalted() || policy.EvolveProposalOnly() {
		return loadWorkflowNativeOnly(root, name)
	}
	return loadWorkflow(root, name)
}

func loadEvolveCommandWorkflow(root, name string, o runOpts) (asset.Workflow, error) {
	if o.chain {
		return loadWorkflowForRunEntry(root, name, o)
	}
	return loadEvolveWorkflow(root, name, o)
}

func proposalOnlyEvolve(wf asset.Workflow, o runOpts, lifecycle string) bool {
	if wf.Stage != "evolve" {
		return false
	}
	policy := mode.Effective(o.mode, lifecycle)
	return policy.BuildHalted() || policy.EvolveProposalOnly()
}

func prepareLoopResume(wf asset.Workflow, o *runOpts, resume bool) (loopResumeState, error) {
	if o.lifecycle == "" {
		o.lifecycle = resolveLifecycle(*o)
	}
	policy := mode.Effective(o.mode, o.lifecycle)
	o.evolveProposalOnly = proposalOnlyEvolve(wf, *o, o.lifecycle)
	phaseLimit, err := orchestrator.EvolvePhaseLimit(wf, policy)
	if err != nil {
		return loopResumeState{}, fmt.Errorf("invalid evolve policy boundary: %w", err)
	}
	binding := checkpointBinding{
		Workflow: wf.Stage, WorkflowDigest: checkpointWorkflowDigest(wf),
		WorkflowAsset: wf,
		Mode:          o.mode, Lifecycle: o.lifecycle, Materiality: o.materiality,
		MaterialityExplicit: resumeMaterialityExplicit(*o), PhaseLimit: phaseLimit,
	}
	state, err := loadLoopResumeState(o.root, resume, binding)
	if err != nil {
		return loopResumeState{}, err
	}
	if err := restoreResumeRunOptions(o, state); err != nil {
		return loopResumeState{}, fmt.Errorf("--resume: %w", err)
	}
	if contractedScanBefore(wf, state.phaseStart) && state.scanReport == "" {
		return loopResumeState{}, fmt.Errorf("--resume: checkpoint phase %d lacks the contracted scan report required by downstream prompts",
			state.phaseStart)
	}
	return state, nil
}

func restoreResumeRunOptions(o *runOpts, state loopResumeState) error {
	if !state.found {
		return nil
	}
	maxCallsSet := o.maxAgentCalls != 0
	budgetSet := strings.TrimSpace(o.runBudgetUSD) != ""
	if o.runFlagsCaptured {
		maxCallsSet = o.maxAgentCallsExplicit
		budgetSet = o.runBudgetExplicit
	}
	if maxCallsSet && o.maxAgentCalls != state.maxAgentCalls {
		return fmt.Errorf("persisted --max-agent-calls=%d conflicts with requested %d",
			state.maxAgentCalls, o.maxAgentCalls)
	}
	if budgetSet {
		requested, err := newRunBudget(o.runBudgetUSD)
		if err != nil {
			return err
		}
		if requested.CapUsdMicros() != state.budgetCapMicros {
			return fmt.Errorf("persisted run budget cap is %d micro-USD, but requested --run-budget-usd resolves to %d",
				state.budgetCapMicros, requested.CapUsdMicros())
		}
	}
	if state.maxLoopBacks != maxLoopBack {
		return fmt.Errorf("persisted loop-back cap=%d conflicts with runtime cap=%d",
			state.maxLoopBacks, maxLoopBack)
	}
	if resumeMaterialityExplicit(*o) && o.materiality != state.materiality {
		return fmt.Errorf("persisted --materiality=%s conflicts with requested %s",
			state.materiality, o.materiality)
	}
	o.maxAgentCalls = state.maxAgentCalls
	o.materiality = state.materiality
	return nil
}

func restoreLoopBudget(budget *runBudget, state loopResumeState) error {
	if !state.found {
		return nil
	}
	if err := budget.restore(state.budgetCapMicros, state.spentMicros); err != nil {
		return fmt.Errorf("restore persisted run budget: %w", err)
	}
	return nil
}

func newResumedRunBudget(flagValue string, state loopResumeState) (*runBudget, error) {
	budget, err := newRunBudget(flagValue)
	if err != nil {
		return nil, err
	}
	if err := restoreLoopBudget(budget, state); err != nil {
		return nil, err
	}
	return budget, nil
}

func contractedScanBefore(wf asset.Workflow, phaseStart int) bool {
	for i, phase := range wf.Phases {
		if phase.ScanContract == asset.ScanContractEvolveV1 {
			return phaseStart > i
		}
	}
	return false
}

func restoreContractedScanOutput(
	executor orchestrator.AgentExecutor,
	wf asset.Workflow,
	phaseStart int,
	semantic string,
	receipts ...*outputbinding.AgentOutputReceipt,
) error {
	if !contractedScanBefore(wf, phaseStart) {
		return nil
	}
	for _, phase := range wf.Phases {
		if phase.ScanContract != asset.ScanContractEvolveV1 {
			continue
		}
		if len(receipts) > 0 && receipts[0] != nil {
			restorer, ok := executor.(validatedOutputRestorer)
			if !ok {
				return fmt.Errorf("executor cannot restore a receipt-bound contracted scan report")
			}
			if err := restorer.RestoreValidatedOutput(phase, semantic, *receipts[0]); err != nil {
				return fmt.Errorf("restore contracted scan report: %w", err)
			}
			return nil
		}
		restorer, ok := executor.(validatedOutputRestorer)
		if !ok {
			return fmt.Errorf("executor cannot restore a validated contracted scan report")
		}
		if err := restorer.RestoreValidatedOutput(phase, semantic); err != nil {
			return fmt.Errorf("restore contracted scan report: %w", err)
		}
		return nil
	}
	return fmt.Errorf("checkpoint carries a scan report but workflow has no contracted scan phase")
}

func restoreFeedForwardOutputs(executor orchestrator.AgentExecutor, wf asset.Workflow,
	phaseStart int, semantic map[string]string, receipts map[string]outputbinding.AgentOutputReceipt) error {
	restorer, ok := executor.(validatedOutputRestorer)
	restored := 0
	for index, phase := range wf.Phases {
		if index >= phaseStart || !phaseNeedsDurableSemantic(phase) {
			continue
		}
		output, present := semantic[phase.Name]
		receipt, referenced := receipts[phase.Name]
		if !ok || !present || !referenced {
			return fmt.Errorf("executor cannot restore receipt-bound feed-forward output for %s", phase.Name)
		}
		if err := restorer.RestoreValidatedOutput(phase, output, receipt); err != nil {
			return fmt.Errorf("restore feed-forward phase %s: %w", phase.Name, err)
		}
		restored++
	}
	if restored != len(semantic) {
		return fmt.Errorf("checkpoint carries extra feed-forward semantic outputs")
	}
	return nil
}

func applyLoopResume(loop *orchestrator.LoopEngine, wf asset.Workflow, resumed loopResumeState) error {
	if loop.Parallel &&
		(resumed.phaseStart > 0 || resumed.agentCalls > 0 || resumed.loopBacks > 0) {
		return fmt.Errorf(
			"parallel execution cannot resume serial mid-iteration checkpoint "+
				"at phase %d with agent_calls=%d loop_backs=%d; resume without --parallel",
			resumed.phaseStart, resumed.agentCalls, resumed.loopBacks,
		)
	}
	loop.StartIter, loop.ResumePrev = resumed.start, resumed.prev
	loop.StartPhase, loop.ResumeGatesGreen = resumed.phaseStart, resumed.gatesGreen
	loop.Engine.InitialAgentCalls = resumed.agentCalls
	loop.Engine.InitialLoopBacks = resumed.loopBacks
	if len(resumed.phaseSemantics) == 0 && resumed.scanReceipt == nil {
		return restoreContractedScanOutput(loop.Engine.Exec, wf, resumed.phaseStart, resumed.scanReport)
	}
	return restoreFeedForwardOutputs(loop.Engine.Exec, wf, resumed.phaseStart,
		resumed.phaseSemantics, resumed.phaseReceipts)
}

func phaseCheckpointHook(
	o runOpts,
	wf asset.Workflow,
	budget *runBudget,
	phaseOut *phaseOutputLedger,
	logln func(string),
	recoveries ...*outputBindingRuntime,
) func(iter, nextPhaseIdx, agentCalls, loopBacks int) error {
	workflowDigest := checkpointWorkflowDigest(wf)
	lifecycle := resolveLifecycle(o)
	boundMateriality := durableRunMateriality(o)
	recovery := firstBindingRecovery(recoveries)
	return func(iter, nextPhaseIdx, agentCalls, loopBacks int) error {
		cp := newPhaseCheckpoint(o, wf, budget, workflowDigest, lifecycle,
			boundMateriality, iter, nextPhaseIdx, agentCalls, loopBacks)
		report, required := checkpointScanReport(wf, phaseOut, cp.PhaseIndex)
		if required && report == "" {
			err := fmt.Errorf("validated scan report is unavailable at resume phase %d", cp.PhaseIndex)
			logln("forge evolve: ERROR durable phase checkpoint refused: " + err.Error())
			return err
		}
		cp.EvolveScanReport = report
		if required {
			semantic := report
			if wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1 {
				var ok bool
				semantic, ok = recovery.recoverySemantic(scanPhaseName(wf))
				if !ok {
					return fmt.Errorf("exact validated scan semantic output is unavailable")
				}
			}
			cp.EvolveScanSemanticOutput = semantic
		}
		if err := bindCheckpointRecovery(&cp, o.root, wf, o, recovery); err != nil {
			return fmt.Errorf("bind checkpoint recovery: %w", err)
		}
		if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil {
			logln(fmt.Sprintf("forge evolve: ERROR phase checkpoint write failed; stopping before more work (recovery state NOT durable): %v", err))
			return fmt.Errorf("persist durable phase progress: %w", err)
		}
		return nil
	}
}

func newPhaseCheckpoint(o runOpts, wf asset.Workflow, budget *runBudget,
	digest, lifecycle, boundMateriality string, iter, next, calls, loopBacks int) persist.Checkpoint {
	prev, _, _ := persist.Load(checkpointPath(o.root))
	if prev.FormatVersion != persist.CheckpointFormatCurrent || prev.Workflow != wf.Stage ||
		prev.WorkflowDigest != digest || prev.Mode != o.mode || prev.Lifecycle != lifecycle ||
		prev.Materiality != boundMateriality {
		prev = persist.Checkpoint{}
	}
	return persist.Checkpoint{
		Workflow: wf.Stage, WorkflowDigest: digest, Mode: o.mode, Lifecycle: lifecycle,
		Materiality: boundMateriality, Iteration: iter - 1, RoadmapCompletion: prev.RoadmapCompletion,
		GatesGreen: prev.GatesGreen, PhaseIndex: next, Reason: "durable phase progress (mid-iteration)",
		UpdatedAtUnix: time.Now().Unix(), SpentUsdMicros: budget.SpentUsdMicros(),
		BudgetCapMicros: budget.CapUsdMicros(), MaxAgentCalls: o.maxAgentCalls, AgentCalls: calls,
		MaxLoopBacks: maxLoopBack, LoopBacks: loopBacks,
	}
}

func firstBindingRecovery(values []*outputBindingRuntime) *outputBindingRuntime {
	if len(values) == 0 {
		return nil
	}
	return values[0]
}

func durableRunMateriality(o runOpts) string {
	if o.materiality == "" {
		return materiality.Unbound
	}
	return o.materiality
}

func proposalLoopSignals(root string, wf asset.Workflow, approved bool, verdicts *verdictLedger) converge.Signals {
	roadmap, _ := os.ReadFile(filepath.Join(root, ".agent", "ROADMAP.md"))
	return converge.Signals{
		RoadmapCompletion:     converge.RoadmapCompletion(string(roadmap)),
		HumanApproved:         approved,
		ReviewStatus:          reviewStatus(verdicts),
		RequirementConfidence: requirementConfidence(wf, verdicts),
	}
}

type checkpointBinding struct {
	Workflow       string
	WorkflowDigest string
	WorkflowAsset  asset.Workflow
	Mode           string
	Lifecycle      string
	Materiality    string
	// MaterialityExplicit makes only a caller-supplied selector conflict with
	// the durable value. Omission restores the checkpoint's exact declaration.
	MaterialityExplicit bool
	// PhaseLimit is the executable phase count after applying the same effective
	// authority policy. PhaseIndex==PhaseLimit means the last phase completed.
	PhaseLimit int
}

func resumeStart(root string, resume bool, binding checkpointBinding) (start int, prev float64, spentMicros int64, phaseStart int, gatesGreen bool, scanReport string, err error) {
	state, err := loadLoopResumeState(root, resume, binding)
	return state.start, state.prev, state.spentMicros, state.phaseStart,
		state.gatesGreen, state.scanReport, err
}

func loadLoopResumeState(
	root string, resume bool, binding checkpointBinding,
) (loopResumeState, error) {
	if !resume {
		return loopResumeState{prev: -1.0}, nil
	}
	if err := rejectTrackedForgeControlState(root); err != nil {
		return loopResumeState{}, fmt.Errorf("--resume: %w", err)
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil {
		return loopResumeState{}, fmt.Errorf("--resume: malformed checkpoint at %s: %w", checkpointPath(root), err)
	}
	if !found {
		fmt.Fprintf(os.Stderr, "forge evolve: --resume found no checkpoint at %s; starting fresh\n", checkpointPath(root))
		return loopResumeState{prev: -1.0}, nil
	}
	if err := validateResumeCheckpoint(cp, binding); err != nil {
		return loopResumeState{}, fmt.Errorf("--resume: invalid checkpoint at %s: %w", checkpointPath(root), err)
	}
	recovery, err := validateBoundCheckpointRecovery(root, cp, binding)
	if err != nil {
		return loopResumeState{}, fmt.Errorf("--resume: invalid bound recovery at %s: %w", checkpointPath(root), err)
	}
	at := ""
	if cp.PhaseIndex > 0 {
		at = fmt.Sprintf(", phase %d", cp.PhaseIndex)
	}
	fmt.Printf("forge evolve: resuming from iteration %d%s (roadmap=%.0f%%, last reason: %s)\n",
		cp.Iteration+1, at, cp.RoadmapCompletion*100, cp.Reason)
	return loopResumeState{
		found: true, start: cp.Iteration + 1, prev: cp.RoadmapCompletion,
		spentMicros: cp.SpentUsdMicros, budgetCapMicros: cp.BudgetCapMicros,
		phaseStart: cp.PhaseIndex, gatesGreen: cp.GatesGreen,
		scanReport: cp.EvolveScanReport, scanSemantic: cp.EvolveScanSemanticOutput,
		maxAgentCalls: cp.MaxAgentCalls,
		agentCalls:    cp.AgentCalls, maxLoopBacks: cp.MaxLoopBacks,
		loopBacks: cp.LoopBacks, materiality: cp.Materiality,
		runID: cp.RunID, receiptHead: cp.ReceiptHead,
		phaseReceipts: recovery.receipts, scanReceipt: recovery.scan,
		phaseSemantics: recovery.semantic,
	}, nil
}

func validateResumeCheckpoint(cp persist.Checkpoint, want checkpointBinding) error {
	boundMateriality, materialityErr := materiality.Normalize(want.Materiality)
	if want.Workflow == "" || want.WorkflowDigest == "" ||
		want.Mode == "" || want.Lifecycle == "" || want.PhaseLimit < 0 || materialityErr != nil {
		return fmt.Errorf("current invocation has incomplete checkpoint binding")
	}
	bound := want.WorkflowAsset.OutputBindingContract == asset.OutputBindingContractLocalDigestV1
	legacyUnbound := !bound && cp.FormatVersion == persist.CheckpointFormatLegacy
	if cp.FormatVersion != persist.CheckpointFormatCurrent && !legacyUnbound {
		return fmt.Errorf("checkpoint format %q is diagnostic-only; resume requires %q",
			cp.FormatVersion, persist.CheckpointFormatCurrent)
	}
	if cp.Workflow == "" || cp.Mode == "" || cp.Lifecycle == "" ||
		!materiality.Valid(cp.Materiality) {
		return fmt.Errorf("checkpoint lacks required workflow/mode/lifecycle/materiality binding; legacy checkpoints cannot be resumed safely")
	}
	if cp.WorkflowDigest == "" {
		return fmt.Errorf("checkpoint lacks required workflow digest; legacy checkpoints cannot be resumed safely")
	}
	if cp.Reason == "" || cp.UpdatedAtUnix <= 0 {
		return fmt.Errorf("checkpoint lacks required reason/updated_at_unix recovery metadata")
	}
	if cp.Workflow != want.Workflow {
		return fmt.Errorf("workflow mismatch: checkpoint=%q invocation=%q", cp.Workflow, want.Workflow)
	}
	if cp.WorkflowDigest != want.WorkflowDigest {
		return fmt.Errorf("workflow digest mismatch: checkpoint=%q invocation=%q", cp.WorkflowDigest, want.WorkflowDigest)
	}
	if cp.Mode != want.Mode {
		return fmt.Errorf("mode mismatch: checkpoint=%q invocation=%q", cp.Mode, want.Mode)
	}
	if cp.Lifecycle != want.Lifecycle {
		return fmt.Errorf("lifecycle mismatch: checkpoint=%q invocation=%q", cp.Lifecycle, want.Lifecycle)
	}
	if want.MaterialityExplicit && cp.Materiality != boundMateriality {
		return fmt.Errorf("materiality mismatch: checkpoint=%q invocation=%q",
			cp.Materiality, boundMateriality)
	}
	if cp.Iteration < 0 {
		return fmt.Errorf("iteration %d must be non-negative", cp.Iteration)
	}
	if cp.Iteration == int(^uint(0)>>1) {
		return fmt.Errorf("iteration %d cannot be incremented safely", cp.Iteration)
	}
	if cp.RoadmapCompletion < 0 || cp.RoadmapCompletion > 1 {
		return fmt.Errorf("roadmap_completion %v must be within [0,1]", cp.RoadmapCompletion)
	}
	return validateResumeResourceProgress(cp, want.PhaseLimit)
}

func validateResumeResourceProgress(cp persist.Checkpoint, phaseLimit int) error {
	switch {
	case cp.PhaseIndex < 0 || cp.PhaseIndex > phaseLimit:
		return fmt.Errorf("phase_index %d outside executable range [0,%d]",
			cp.PhaseIndex, phaseLimit)
	case cp.PhaseIndex > 0 && cp.AgentCalls == 0:
		return fmt.Errorf("phase_index %d is unreachable with zero agent_calls for an Evolve workflow",
			cp.PhaseIndex)
	case cp.LoopBacks > cp.AgentCalls:
		return fmt.Errorf("loop_backs %d exceeds recorded agent_calls %d",
			cp.LoopBacks, cp.AgentCalls)
	case cp.SpentUsdMicros < 0:
		return fmt.Errorf("spent_usd_micros %d must be non-negative", cp.SpentUsdMicros)
	default:
		return nil
	}
}
