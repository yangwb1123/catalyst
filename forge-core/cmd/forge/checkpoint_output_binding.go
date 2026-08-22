package main

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/evolvescan"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/persist"
)

type validatedOutputRestorer interface {
	RestoreValidatedOutput(asset.Phase, string, ...outputbinding.AgentOutputReceipt) error
}

func checkpointWorkflowDigest(wf asset.Workflow) string {
	data, err := json.Marshal(wf)
	if err != nil {
		return ""
	}
	return fmt.Sprintf("%x", sha256.Sum256(data))
}

func checkpointScanReport(wf asset.Workflow, phaseOut *phaseOutputLedger, phaseIndex int) (string, bool) {
	for i, phase := range wf.Phases {
		if phase.ScanContract != asset.ScanContractEvolveV1 || phaseIndex <= i {
			continue
		}
		report, _ := phaseOut.output(phase.Name)
		return report, true
	}
	return "", false
}

func scanPhaseName(wf asset.Workflow) string {
	for _, phase := range wf.Phases {
		if phase.ScanContract == asset.ScanContractEvolveV1 {
			return phase.Name
		}
	}
	return ""
}

func (runtime *outputBindingRuntime) recoveryReceipts() map[string]outputbinding.AgentOutputReceipt {
	if runtime == nil {
		return nil
	}
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	result := make(map[string]outputbinding.AgentOutputReceipt, len(runtime.accepted))
	for phase, receipt := range runtime.accepted {
		result[phase] = receipt
	}
	return result
}

func (runtime *outputBindingRuntime) recoverySemantic(phase string) (string, bool) {
	if runtime == nil {
		return "", false
	}
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	value, ok := runtime.acceptedSemantic[phase]
	return value, ok
}

func bindCheckpointRecovery(cp *persist.Checkpoint, root string, wf asset.Workflow,
	o runOpts, runtime *outputBindingRuntime) error {
	if wf.OutputBindingContract != asset.OutputBindingContractLocalDigestV1 {
		cp.FormatVersion, cp.RunID = persist.CheckpointFormatCurrent, "run_id_not_bound"
		return nil
	}
	if runtime == nil {
		return fmt.Errorf("bound checkpoint recovery runtime is unavailable")
	}
	cp.FormatVersion = persist.CheckpointFormatCurrent
	cp.RunID = runtime.runID
	index, err := loadRecoveryReceiptIndex(root)
	if err != nil {
		return err
	}
	cp.ReceiptHead = index.head
	cp.PhaseReceipts = map[string]string{}
	cp.PhaseSemanticOutputs = map[string]string{}
	cp.StageReceipts = map[string]string{}
	cp.ApprovalContexts = map[string]string{}
	receipts := runtime.recoveryReceipts()
	for _, phase := range checkpointReferencedPhases(wf, o, cp) {
		receipt, ok := receipts[phase.Name]
		if !ok {
			return fmt.Errorf("checkpoint cursor lacks accepted receipt for phase %s", phase.Name)
		}
		if err := outputbindingstore.New(root).RequireReceiptClaim(receipt); err != nil {
			return err
		}
		cp.PhaseReceipts[phaseReceiptKey(wf.Stage, phase.Name)] = receipt.ReceiptSHA256
		if phaseNeedsDurableSemantic(phase) && cp.PhaseIndex != 0 {
			semantic, ok := runtime.recoverySemantic(phase.Name)
			if !ok {
				return fmt.Errorf("checkpoint cursor lacks semantic output for feed-forward phase %s", phase.Name)
			}
			cp.PhaseSemanticOutputs[phaseReceiptKey(wf.Stage, phase.Name)] = semantic
		}
	}
	if cp.Iteration > 0 && cp.PhaseIndex == 0 {
		phases := checkpointReferencedPhases(wf, o, cp)
		if len(phases) > 0 {
			cp.StageReceipts[wf.Stage] = receipts[phases[len(phases)-1].Name].ReceiptSHA256
		}
	}
	return nil
}

func checkpointReferencedPhases(wf asset.Workflow, o runOpts, cp *persist.Checkpoint) []asset.Phase {
	phases, err := expectedBoundCommandPhases(wf, o.mode, resolveLifecycle(o), durableRunMateriality(o))
	if err != nil {
		return nil
	}
	if cp.PhaseIndex == 0 {
		if cp.Iteration > 0 {
			return phases
		}
		return []asset.Phase{}
	}
	result := make([]asset.Phase, 0, len(phases))
	for _, phase := range phases {
		if workflowPhaseIndex(wf, phase.Name) < cp.PhaseIndex {
			result = append(result, phase)
		}
	}
	return result
}

func workflowPhaseIndex(wf asset.Workflow, name string) int {
	for index, phase := range wf.Phases {
		if phase.Name == name {
			return index
		}
	}
	return len(wf.Phases)
}

func restoreCheckpointReceipts(runtime *outputBindingRuntime,
	receipts map[string]outputbinding.AgentOutputReceipt, head string,
	semantics ...map[string]string,
) {
	if runtime == nil {
		return
	}
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	for phase, receipt := range receipts {
		runtime.accepted[phase] = receipt
	}
	if len(semantics) > 0 {
		for phase, semantic := range semantics[0] {
			runtime.acceptedSemantic[phase] = semantic
		}
	}
	runtime.journalHead = head
}

type checkpointRecovery struct {
	receipts map[string]outputbinding.AgentOutputReceipt
	semantic map[string]string
	scan     *outputbinding.AgentOutputReceipt
}

func validateBoundCheckpointRecovery(root string, cp persist.Checkpoint,
	binding checkpointBinding) (checkpointRecovery, error) {
	if binding.WorkflowAsset.OutputBindingContract != asset.OutputBindingContractLocalDigestV1 {
		return checkpointRecovery{}, nil
	}
	if cp.FormatVersion != persist.CheckpointFormatCurrent {
		return checkpointRecovery{}, fmt.Errorf("checkpoint format %q is diagnostic-only for bound workflow", cp.FormatVersion)
	}
	if cp.RunID == "" {
		return checkpointRecovery{}, fmt.Errorf("checkpoint lacks bound run")
	}
	live, err := loadWorkflowNativeOnly(root, cp.Workflow)
	if err != nil || checkpointWorkflowDigest(live) != cp.WorkflowDigest {
		return checkpointRecovery{}, fmt.Errorf("checkpoint workflow changed before recovery")
	}
	binding.WorkflowAsset = live
	index, err := loadRecoveryReceiptIndex(root)
	if err != nil || index.head != cp.ReceiptHead {
		return checkpointRecovery{}, fmt.Errorf("checkpoint receipt journal head differs from live journal")
	}
	want := checkpointReferencedPhases(binding.WorkflowAsset, runOpts{
		mode: cp.Mode, lifecycle: cp.Lifecycle, materiality: cp.Materiality,
	}, &cp)
	if len(cp.PhaseReceipts) != len(want) {
		return checkpointRecovery{}, fmt.Errorf("checkpoint phase receipt map is not exact for cursor")
	}
	result := checkpointRecovery{
		receipts: map[string]outputbinding.AgentOutputReceipt{}, semantic: map[string]string{},
	}
	if err := collectCheckpointReceipts(root, cp, binding.WorkflowAsset, want, index, &result); err != nil {
		return checkpointRecovery{}, err
	}
	if err := validateCheckpointStageRefs(cp, want); err != nil {
		return checkpointRecovery{}, err
	}
	if len(want) > 0 {
		terminal := result.receipts[want[len(want)-1].Name]
		if err := verifyRecoveryReceiptLive(root, terminal); err != nil {
			return checkpointRecovery{}, fmt.Errorf("checkpoint terminal receipt is stale: %w", err)
		}
	}
	if err := validateCheckpointScanBinding(cp, result.scan); err != nil {
		return checkpointRecovery{}, err
	}
	if err := collectCheckpointSemantics(cp, want, &result); err != nil {
		return checkpointRecovery{}, err
	}
	return result, nil
}

func collectCheckpointSemantics(cp persist.Checkpoint, phases []asset.Phase,
	result *checkpointRecovery) error {
	want := 0
	for _, phase := range phases {
		if !phaseNeedsDurableSemantic(phase) || cp.PhaseIndex == 0 {
			continue
		}
		want++
		key := phaseReceiptKey(cp.Workflow, phase.Name)
		semantic, ok := cp.PhaseSemanticOutputs[key]
		receipt := result.receipts[phase.Name]
		if !ok || int64(len([]byte(semantic))) != receipt.SemanticOutputBytes ||
			outputbinding.SHA256([]byte(semantic)) != receipt.SemanticOutputSHA256 {
			return fmt.Errorf("checkpoint semantic output %q is absent or differs from receipt", key)
		}
		result.semantic[phase.Name] = semantic
	}
	if len(cp.PhaseSemanticOutputs) != want {
		return fmt.Errorf("checkpoint phase semantic output map is not exact for cursor")
	}
	return nil
}

func collectCheckpointReceipts(root string, cp persist.Checkpoint, wf asset.Workflow,
	phases []asset.Phase, index recoveryReceiptIndex, result *checkpointRecovery) error {
	var prior int64
	for _, phase := range phases {
		receipt, err := validateCheckpointPhaseReceipt(root, cp, wf, phase, index)
		if err != nil {
			return err
		}
		if receipt.LedgerSequence <= prior {
			return fmt.Errorf("checkpoint phase receipts are not in traversal order")
		}
		prior, result.receipts[phase.Name] = receipt.LedgerSequence, receipt
		if phase.ScanContract == asset.ScanContractEvolveV1 && cp.PhaseIndex != 0 {
			copy := receipt
			result.scan = &copy
		}
	}
	return nil
}

func validateCheckpointScanBinding(cp persist.Checkpoint,
	receipt *outputbinding.AgentOutputReceipt) error {
	if receipt == nil {
		if cp.EvolveScanReport != "" || cp.EvolveScanSemanticOutput != "" {
			return fmt.Errorf("checkpoint carries scan output before its receipt cursor")
		}
		return nil
	}
	key := phaseReceiptKey(cp.Workflow, receipt.Phase)
	semantic := cp.PhaseSemanticOutputs[key]
	if semantic == "" || int64(len([]byte(semantic))) != receipt.SemanticOutputBytes ||
		outputbinding.SHA256([]byte(semantic)) != receipt.SemanticOutputSHA256 {
		return fmt.Errorf("checkpoint scan semantic output differs from its receipt")
	}
	canonical, err := evolvescan.Canonicalize(semantic)
	if err != nil || canonical != cp.EvolveScanReport {
		return fmt.Errorf("checkpoint canonical scan report differs from its exact semantic output")
	}
	if cp.EvolveScanSemanticOutput != semantic {
		return fmt.Errorf("checkpoint scan semantic alias differs from phase semantic output")
	}
	return nil
}

func validateCheckpointPhaseReceipt(root string, cp persist.Checkpoint, wf asset.Workflow,
	phase asset.Phase, index recoveryReceiptIndex) (outputbinding.AgentOutputReceipt, error) {
	key := phaseReceiptKey(cp.Workflow, phase.Name)
	digest, ok := cp.PhaseReceipts[key]
	receipt, present := index.byDigest[digest]
	if !ok || !present || receipt.RunID != cp.RunID || receipt.Workflow != cp.Workflow ||
		receipt.Phase != phase.Name || receipt.RuntimePolicy.WorkflowSHA256 != cp.WorkflowDigest ||
		receipt.RuntimePolicy.Mode != cp.Mode || receipt.RuntimePolicy.Lifecycle != cp.Lifecycle ||
		receipt.RuntimePolicy.Materiality != cp.Materiality {
		return outputbinding.AgentOutputReceipt{}, fmt.Errorf("checkpoint phase receipt %q is absent or mismatched", key)
	}
	if err := outputbindingstore.New(root).RequireReceiptClaim(receipt); err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	if err := verifyEmbeddedRuntimePolicy(receipt.RuntimePolicy, wf, phase, cp.Materiality); err != nil {
		return outputbinding.AgentOutputReceipt{}, fmt.Errorf("checkpoint phase receipt %q policy: %w", key, err)
	}
	return receipt, nil
}

func validateCheckpointStageRefs(cp persist.Checkpoint, phases []asset.Phase) error {
	if len(cp.ApprovalContexts) != 0 {
		return fmt.Errorf("standalone evolve checkpoint must not carry approval contexts")
	}
	if cp.Iteration == 0 || cp.PhaseIndex != 0 {
		if len(cp.StageReceipts) != 0 {
			return fmt.Errorf("mid-iteration checkpoint must not carry a stage receipt")
		}
		return nil
	}
	if len(phases) == 0 || len(cp.StageReceipts) != 1 {
		return fmt.Errorf("iteration checkpoint lacks its terminal stage receipt")
	}
	terminal := cp.PhaseReceipts[phaseReceiptKey(cp.Workflow, phases[len(phases)-1].Name)]
	if cp.StageReceipts[cp.Workflow] != terminal {
		return fmt.Errorf("iteration checkpoint stage receipt is not terminal")
	}
	return nil
}
