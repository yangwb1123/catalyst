package main

import (
	"fmt"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/prompt"
)

type engineRuntimeWiring struct {
	exec        orchestrator.AgentExecutor
	binding     *outputBindingRuntime
	provenance  *artifactProvenance
	priorEmits  func(string) []string
	bindingExec outputBindingEngineHooks
}

func buildEngineRuntimeWiring(wf asset.Workflow, o runOpts, pol mode.Policy,
	logln func(string), costSink func(string, string, float64, time.Duration),
	tierOf func(asset.Phase) string, phaseOut *phaseOutputLedger,
	ledgers enginePromptLedgers, runID string) engineRuntimeWiring {
	priorEmits := priorEmitsOf(wf)
	modelFor := phaseTierByName(wf, tierOf)
	provenance := newArtifactProvenance(o.root, wf.Stage, runID, o.releaseAgentSHA256)
	binding := newOutputBindingRuntime(
		outputBindingWorkflowInfo{root: o.root, runID: runID, wf: wf},
		outputBindingExecutionInfo{opts: o, policy: pol, isClaude: isClaudeExecutable(o.agentCmd)},
		priorEmits, provenance.bindingOutputPaths,
	)
	binding.setOutputValidator(provenance.validateBoundOutputManifest)
	hooks := binding.hooks()
	bindingObserver := outputBindingBuildObserver(binding)
	exec := agentExecutor(o, logln, costSink, tierOf, modelFor,
		ledgers.context, ledgers.gates, phaseOut, feedsForwardOf(wf), ledgers.verdicts,
		ledgers.findings, onFailTargetOf(wf), priorEmits, executorHooks{
			PrepareCommand: hooks.PrepareCommand, ValidateOutput: phaseOutputContractWithPolicy(o.root, wf, pol, provenance),
			ValidateRawOutput: workflowRawOutputContract(wf, o, o.agentCmd), SemanticOutput: hooks.SemanticOutput,
			FinalizeCommand: combineCommandFinalizers(
				provenance.validateBuildPreparation, hooks.FinalizeCommand,
			), CommitValidatedOutput: hooks.CommitValidatedOutput,
			OnBuild: combineBuildObservers(provenance.recordBuild, bindingObserver), ModelFor: provenance.modelFor,
			FrozenEmits:        binding.promptEmitBlocks,
			VerdictContractFor: effectiveVerdictContractOf(wf, o), ScanContractFor: scanContractOf(wf),
			ScanDepth: pol.EvolveDepth,
		})
	return engineRuntimeWiring{
		exec: exec, binding: binding, provenance: provenance, priorEmits: priorEmits,
		bindingExec: binding.engineHooks(),
	}
}

func combineCommandFinalizers(
	finalizers ...func(asset.Phase, string, []string) ([]string, error),
) func(asset.Phase, string, []string) ([]string, error) {
	return func(phase asset.Phase, mode string, argv []string) ([]string, error) {
		var err error
		for _, finalize := range finalizers {
			if finalize == nil {
				continue
			}
			argv, err = finalize(phase, mode, argv)
			if err != nil {
				return nil, err
			}
		}
		return argv, nil
	}
}

func outputBindingBuildObserver(runtime *outputBindingRuntime) func(asset.Phase, string, string, string, map[string]string) {
	if runtime == nil {
		return nil
	}
	return runtime.recordBuild
}

func combineBuildObservers(observers ...func(asset.Phase, string, string, string, map[string]string)) func(asset.Phase, string, string, string, map[string]string) {
	return func(phase asset.Phase, model, promptText, source string, inputs map[string]string) {
		for _, observe := range observers {
			if observe != nil {
				observe(phase, model, promptText, source, inputs)
			}
		}
	}
}

type enginePromptLedgers struct {
	context  *prompt.ContextCache
	gates    *gateLedger
	verdicts *verdictLedger
	findings *reviewFindingsLedger
}

type executorHooks struct {
	PrepareCommand        func(phase asset.Phase, mode string) error
	ValidateOutput        func(phase, output string) error
	ValidateRawOutput     func(phase, output string) error
	SemanticOutput        func(rawOutput string) (string, error)
	FinalizeCommand       func(phase asset.Phase, mode string, argv []string) ([]string, error)
	CommitValidatedOutput func(phase, rawOutput, output string, latency time.Duration) error
	OnBuild               func(phase asset.Phase, model, promptText, frozenSourceRevision string, frozenReleaseInputs map[string]string)
	FrozenEmits           func(phase asset.Phase) ([]string, bool)
	ModelFor              func(phase string) string
	VerdictContractFor    func(phase string) string
	ScanContractFor       func(phase string) string
	ScanDepth             string
}

func firstExecutorHooks(hooks []executorHooks) executorHooks {
	if len(hooks) == 0 {
		return executorHooks{}
	}
	return hooks[0]
}

func preferPhaseModel(primary, fallback func(string) string) func(string) string {
	return func(name string) string {
		if model := phaseModelOf(primary, name); model != "" {
			return model
		}
		return phaseModelOf(fallback, name)
	}
}

func bindingCompletionValidator(wiring engineRuntimeWiring, wf asset.Workflow) func() error {
	return func() error {
		if wiring.bindingExec.workflowComplete == nil {
			return nil
		}
		return wiring.bindingExec.workflowComplete(wf)
	}
}

func (r *chainRuntime) bindStageRecovery(wf asset.Workflow) error {
	index, err := loadRecoveryReceiptIndex(r.opts.root)
	if err != nil {
		return err
	}
	r.state.ReceiptHead = index.head
	r.state.ensureRecoveryMaps()
	if wf.OutputBindingContract != asset.OutputBindingContractLocalDigestV1 {
		return nil
	}
	phases, err := expectedBoundCommandPhases(wf, r.state.Mode, r.state.Lifecycle, r.state.Materiality)
	if err != nil {
		return fmt.Errorf("resolve bound command phases for %s: %w", wf.Stage, err)
	}
	if len(phases) == 0 {
		return fmt.Errorf("bound stage %s has no accepted command phase and cannot be durable", wf.Stage)
	}
	receipts, err := selectStageRecoveryReceipts(r.opts.root, index, r.state, wf, phases)
	if err != nil {
		return err
	}
	if err := verifyRecoveryReceiptLive(r.opts.root, receipts[len(receipts)-1]); err != nil {
		return fmt.Errorf("bound stage %s terminal freshness: %w", wf.Stage, err)
	}
	r.replaceStageReceiptRefs(wf.Stage, phases, receipts)
	return r.bindStageApprovalContext(wf.Stage, receipts[len(receipts)-1])
}

func selectStageRecoveryReceipts(root string, index recoveryReceiptIndex, state chainState,
	wf asset.Workflow, phases []asset.Phase) ([]outputbinding.AgentOutputReceipt, error) {
	receipts := make([]outputbinding.AgentOutputReceipt, len(phases))
	var priorSequence int64
	for position, phase := range phases {
		receipt, ok := index.latest(state.RunID, wf.Stage, phase.Name)
		if !ok {
			return nil, fmt.Errorf("bound stage %s lacks accepted receipt for phase %s", wf.Stage, phase.Name)
		}
		if err := verifyRecoveryReceiptIdentity(root, receipt, wf, phase, state); err != nil {
			return nil, fmt.Errorf("bound stage %s phase %s: %w", wf.Stage, phase.Name, err)
		}
		if receipt.LedgerSequence <= priorSequence {
			return nil, fmt.Errorf("bound stage %s receipt order is not a completed phase traversal", wf.Stage)
		}
		priorSequence, receipts[position] = receipt.LedgerSequence, receipt
	}
	return receipts, nil
}

func (r *chainRuntime) replaceStageReceiptRefs(stage string, phases []asset.Phase,
	receipts []outputbinding.AgentOutputReceipt) {
	for key := range r.state.PhaseReceipts {
		if strings.HasPrefix(key, stage+"/") {
			delete(r.state.PhaseReceipts, key)
		}
	}
	for index, phase := range phases {
		r.state.PhaseReceipts[phaseReceiptKey(stage, phase.Name)] = receipts[index].ReceiptSHA256
	}
	r.state.StageReceipts[stage] = receipts[len(receipts)-1].ReceiptSHA256
}

func (r *chainRuntime) bindStageApprovalContext(stage string,
	terminal outputbinding.AgentOutputReceipt) error {
	if !approvalContextStage(stage) {
		return nil
	}
	verified, err := verifyBoundApprovalContext(r.opts.root, stage)
	if err != nil {
		return fmt.Errorf("bind stage approval context: %w", err)
	}
	if verified.Receipt.ReceiptSHA256 != terminal.ReceiptSHA256 {
		return fmt.Errorf("stage approval context does not reference terminal accepted receipt")
	}
	r.state.ApprovalContexts[stage] = verified.ContextSHA256
	return nil
}

func (r *chainRuntime) refreshReceiptHead() error {
	index, err := loadRecoveryReceiptIndex(r.opts.root)
	if err != nil {
		return err
	}
	r.state.ReceiptHead = index.head
	r.state.ensureRecoveryMaps()
	return nil
}
