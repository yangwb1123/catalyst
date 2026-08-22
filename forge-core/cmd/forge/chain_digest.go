package main

import (
	"context"
	"fmt"
	"reflect"
	"slices"
	"strings"

	"forgeos/forge-core/internal/artifact"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/declaredartifact"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/productsource"
)

const workflowDigestHexLength = 64

func validateChainWorkflowDigestMap(digests map[string]string) error {
	if digests == nil {
		return fmt.Errorf("workflow_digests must be a non-null object")
	}
	for stage, digest := range digests {
		if !knownWorkflowStage(stage) {
			return fmt.Errorf("workflow_digests contains unknown stage %q", stage)
		}
		if !validWorkflowDigest(digest) {
			return fmt.Errorf("workflow_digests[%q] must be a lowercase SHA-256 digest", stage)
		}
	}
	return nil
}

func validWorkflowDigest(value string) bool {
	if len(value) != workflowDigestHexLength {
		return false
	}
	for _, char := range value {
		if !strings.ContainsRune("0123456789abcdef", char) {
			return false
		}
	}
	return true
}

func (s *chainState) bindWorkflow(stage, digest string, bound ...bool) error {
	if !validWorkflowDigest(digest) {
		return fmt.Errorf("cannot bind invalid workflow digest for stage %q", stage)
	}
	if s.WorkflowDigests == nil {
		s.WorkflowDigests = make(map[string]string)
	}
	if prior, exists := s.WorkflowDigests[stage]; exists && prior != digest {
		return fmt.Errorf("workflow digest changed for stage %q", stage)
	}
	s.WorkflowDigests[stage] = digest
	if len(bound) > 0 && bound[0] && !stageIn(s.BoundStages, stage) {
		s.BoundStages = append(s.BoundStages, stage)
		slices.Sort(s.BoundStages)
	}
	return nil
}

func validateResumableChainWorkflowDigests(state chainState) error {
	if err := validateChainWorkflowDigestMap(state.WorkflowDigests); err != nil {
		return fmt.Errorf("persisted waiting chain has invalid workflow digest binding: %w", err)
	}
	required := make(map[string]struct{}, len(state.CompletedStages)+len(state.InheritedStages)+1)
	for _, stage := range state.CompletedStages {
		required[stage] = struct{}{}
	}
	for _, stage := range state.InheritedStages {
		required[stage] = struct{}{}
	}
	required[state.CurrentStage] = struct{}{}
	for stage := range required {
		if _, exists := state.WorkflowDigests[stage]; !exists {
			return fmt.Errorf("persisted waiting chain lacks workflow digest for stage %q", stage)
		}
	}
	if len(state.WorkflowDigests) != len(required) {
		return fmt.Errorf("persisted waiting chain workflow_digests do not exactly bind completed/current stages")
	}
	return nil
}

func validatePersistedWorkflowDigest(state chainState, stage, got string) error {
	want, exists := state.WorkflowDigests[stage]
	if !exists {
		return fmt.Errorf("persisted chain lacks workflow digest for stage %q", stage)
	}
	if got != want {
		return fmt.Errorf("persisted chain workflow digest mismatch for stage %q: checkpoint=%q current=%q",
			stage, want, got)
	}
	return nil
}

type recoveryReceiptIndex struct {
	receipts []outputbinding.AgentOutputReceipt
	byDigest map[string]outputbinding.AgentOutputReceipt
	head     string
}

func loadRecoveryReceiptIndex(root string) (recoveryReceiptIndex, error) {
	receipts, err := outputbindingstore.New(root).Load()
	if err != nil {
		return recoveryReceiptIndex{}, fmt.Errorf("load bound recovery receipt journal: %w", err)
	}
	index := recoveryReceiptIndex{
		receipts: receipts, byDigest: make(map[string]outputbinding.AgentOutputReceipt, len(receipts)),
	}
	for _, receipt := range receipts {
		index.byDigest[receipt.ReceiptSHA256] = receipt
		index.head = receipt.ReceiptSHA256
	}
	return index, nil
}

func (index recoveryReceiptIndex) latest(runID, stage, phase string) (outputbinding.AgentOutputReceipt, bool) {
	for position := len(index.receipts) - 1; position >= 0; position-- {
		receipt := index.receipts[position]
		if receipt.RunID == runID && receipt.Workflow == stage && receipt.Phase == phase {
			return receipt, true
		}
	}
	return outputbinding.AgentOutputReceipt{}, false
}

func expectedBoundCommandPhases(wf asset.Workflow, runMode, lifecycle, materiality string) ([]asset.Phase, error) {
	if wf.OutputBindingContract != asset.OutputBindingContractLocalDigestV1 {
		return nil, nil
	}
	policy := materialityPolicy(wf, runOpts{materiality: materiality}, mode.Effective(runMode, lifecycle))
	if (wf.Stage == "discover" && policy.DiscoverSkipped()) ||
		(wf.Stage == "review" && policy.ReviewSkipped()) {
		return []asset.Phase{}, nil
	}
	limit, err := orchestrator.EvolvePhaseLimit(wf, policy)
	if err != nil {
		return nil, err
	}
	result := make([]asset.Phase, 0, limit)
	for _, phase := range wf.Phases[:limit] {
		if phase.Agent != "harness" && !recoveryPhaseSkipped(phase, wf.Stage, policy) {
			result = append(result, phase)
		}
	}
	return result, nil
}

func recoveryPhaseSkipped(phase asset.Phase, stage string, policy mode.Policy) bool {
	if requiredBuildReviewer(asset.Workflow{Stage: stage}, runOpts{}, phase) {
		return false
	}
	if strings.HasSuffix(phase.RequiredWhen, "#workflow_depth.reviewer") && !policy.Reviewer {
		return true
	}
	depthAtMax := (stage == "discover" && policy.DiscoverDepth == mode.DiscoverFull) ||
		(stage == "review" && policy.ReviewDepth == mode.ReviewFull)
	if depthAtMax {
		return false
	}
	for _, optional := range phase.OptionalFor {
		if optional == policy.Mode {
			return true
		}
	}
	return false
}

func phaseReceiptKey(stage, phase string) string { return stage + "/" + phase }

func verifyRecoveryReceiptIdentity(root string, receipt outputbinding.AgentOutputReceipt,
	wf asset.Workflow, phase asset.Phase, state chainState) error {
	if err := outputbindingstore.New(root).RequireReceiptClaim(receipt); err != nil {
		return err
	}
	if receipt.RunID != state.RunID || receipt.Workflow != wf.Stage || receipt.Phase != phase.Name {
		return fmt.Errorf("receipt identity differs from recovery cursor")
	}
	if receipt.RuntimePolicy.WorkflowSHA256 != state.WorkflowDigests[wf.Stage] ||
		receipt.RuntimePolicy.WorkflowSHA256 != checkpointWorkflowDigest(wf) {
		return fmt.Errorf("receipt workflow digest differs from recovery cursor")
	}
	if receipt.RuntimePolicy.Mode != state.Mode || receipt.RuntimePolicy.Lifecycle != state.Lifecycle ||
		receipt.RuntimePolicy.Materiality != state.Materiality {
		return fmt.Errorf("receipt mode/lifecycle/materiality differs from recovery cursor")
	}
	return verifyEmbeddedRuntimePolicy(receipt.RuntimePolicy, wf, phase, state.Materiality)
}

func verifyRecoveryReceiptLive(root string, receipt outputbinding.AgentOutputReceipt) error {
	snapshot, err := productsource.Capture(context.Background(), root, productSourceEnvironment())
	if err != nil {
		return fmt.Errorf("capture recovery product source: %w", err)
	}
	if snapshot.SHA256 != receipt.SourceAfterSHA256 ||
		snapshot.Manifest.SourceRevision != receipt.SourceRevision {
		return fmt.Errorf("current product source differs from terminal receipt")
	}
	outputs, err := recaptureRecoveryManifest(snapshot, receipt.ArtifactOutputs)
	if err != nil || !reflect.DeepEqual(outputs, receipt.ArtifactOutputs) {
		if err == nil {
			err = fmt.Errorf("current output manifest differs")
		}
		return err
	}
	inputs, err := currentApprovalInputs(receipt.ArtifactInputs, receipt.ArtifactOutputs)
	if err != nil {
		return err
	}
	currentInputs, err := recaptureRecoveryManifest(snapshot, inputs)
	if err != nil || !reflect.DeepEqual(currentInputs, inputs) {
		if err == nil {
			err = fmt.Errorf("current input manifest differs")
		}
		return err
	}
	return nil
}

func recaptureRecoveryManifest(snapshot productsource.Snapshot,
	want outputbinding.ArtifactManifest) (outputbinding.ArtifactManifest, error) {
	paths := make([]string, len(want.Items))
	for index, item := range want.Items {
		paths[index] = item.Path
	}
	return declaredartifact.Capture(context.Background(), snapshot, paths)
}

func receiptPhase(wf asset.Workflow, receipt outputbinding.AgentOutputReceipt) (asset.Phase, error) {
	for _, phase := range wf.Phases {
		if phase.Name == receipt.Phase {
			return phase, nil
		}
	}
	return asset.Phase{}, fmt.Errorf("receipt references unknown phase %q", receipt.Phase)
}

func prepareRollbackRecoveryBranch(first asset.Workflow, o runOpts,
	completed chainState) (asset.Workflow, *chainState, error) {
	if completed.Format != chainStateFormat || !stageIn(completed.CompletedStages, "deploy") {
		return first, nil, nil
	}
	if err := validateBoundChainRecovery(o.root, completed); err != nil {
		return asset.Workflow{}, nil, fmt.Errorf("validate rollback parent chain: %w", err)
	}
	if err := validateChainRunOptionConflicts(o, completed, resolveLifecycle(o)); err != nil {
		return asset.Workflow{}, nil, err
	}
	live, err := loadWorkflowNativeOnly(o.root, "rollback")
	if err != nil || checkpointWorkflowDigest(live) != checkpointWorkflowDigest(first) {
		return asset.Workflow{}, nil, fmt.Errorf("rollback workflow changed before branch preparation")
	}
	if _, err := resolveBoundChainArtifactInput(
		o.root, completed.RunID, "deploy", "release-planning",
		"docs/release/release-manifest.yml",
	); err != nil {
		return asset.Workflow{}, nil, fmt.Errorf("validate rollback deploy provenance: %w", err)
	}
	branch := rollbackBranchState(completed)
	bound := live.OutputBindingContract == asset.OutputBindingContractLocalDigestV1
	if err := branch.bindWorkflow("rollback", checkpointWorkflowDigest(live), bound); err != nil {
		return asset.Workflow{}, nil, err
	}
	return live, &branch, nil
}

func rollbackBranchState(parent chainState) chainState {
	branch := parent
	branch.Status, branch.EntryStage, branch.CurrentStage = "branching", "rollback", "rollback"
	branch.CompletedStages = nil
	branch.InheritedStages = []string{"deploy"}
	branch.BoundStages = filterStringSlice(parent.BoundStages, func(stage string) bool { return stage == "deploy" })
	branch.Reason = "rollback branch inherits exact approved deploy output provenance"
	branch.WorkflowDigests = filterStringMap(parent.WorkflowDigests, func(key string) bool {
		return key == "deploy"
	})
	branch.PhaseReceipts = filterStringMap(parent.PhaseReceipts, func(key string) bool {
		return strings.HasPrefix(key, "deploy/")
	})
	branch.StageReceipts = filterStringMap(parent.StageReceipts, func(key string) bool {
		return key == "deploy"
	})
	branch.ApprovalContexts = filterStringMap(parent.ApprovalContexts, func(key string) bool {
		return key == "deploy"
	})
	return branch
}

func filterStringSlice(values []string, keep func(string) bool) []string {
	result := make([]string, 0, len(values))
	for _, value := range values {
		if keep(value) {
			result = append(result, value)
		}
	}
	return result
}

func filterStringMap(values map[string]string, keep func(string) bool) map[string]string {
	result := make(map[string]string)
	for key, value := range values {
		if keep(key) {
			result[key] = value
		}
	}
	return result
}

func stageIn(stages []string, want string) bool {
	for _, stage := range stages {
		if stage == want {
			return true
		}
	}
	return false
}

func resolveBoundChainArtifactInput(root, runID, producerStage, producerPhase,
	path string) (outputbinding.ManifestItem, error) {
	state, found, err := loadChainState(root)
	if err != nil || !found {
		return outputbinding.ManifestItem{}, fmt.Errorf("load chain v5 provenance: %w", missingStateError(err, found))
	}
	complete := stageIn(state.CompletedStages, producerStage) || stageIn(state.InheritedStages, producerStage)
	if state.Format != chainStateFormat || state.RunID != runID || !complete ||
		!stageIn(state.BoundStages, producerStage) {
		return outputbinding.ManifestItem{}, fmt.Errorf("artifact producer is not inherited/completed in the same chain v5 run")
	}
	index, err := loadRecoveryReceiptIndex(root)
	if err != nil {
		return outputbinding.ManifestItem{}, err
	}
	if state.ReceiptHead == "" {
		return outputbinding.ManifestItem{}, fmt.Errorf("inherited chain has no receipt head")
	}
	if _, ok := index.byDigest[state.ReceiptHead]; !ok {
		return outputbinding.ManifestItem{}, fmt.Errorf("inherited chain receipt head is absent from the live journal")
	}
	wf, phase, err := inheritedProducerWorkflow(root, state, producerStage, producerPhase)
	if err != nil {
		return outputbinding.ManifestItem{}, err
	}
	digest := state.PhaseReceipts[phaseReceiptKey(producerStage, producerPhase)]
	receipt := index.byDigest[digest]
	if err := verifyRecoveryReceiptIdentity(root, receipt, wf, phase, state); err != nil {
		return outputbinding.ManifestItem{}, err
	}
	if err := verifyHistoricalApprovalContext(root, state, producerStage, index); err != nil {
		return outputbinding.ManifestItem{}, err
	}
	for _, item := range receipt.ArtifactOutputs.Items {
		if item.Path == path {
			return verifyCurrentReleaseItem(root, item)
		}
	}
	return outputbinding.ManifestItem{}, fmt.Errorf("producer receipt does not contain artifact %q", path)
}

func inheritedProducerWorkflow(root string, state chainState, stage, phaseName string) (
	asset.Workflow, asset.Phase, error,
) {
	wf, err := loadWorkflowNativeOnly(root, stage)
	if err != nil || checkpointWorkflowDigest(wf) != state.WorkflowDigests[stage] {
		return asset.Workflow{}, asset.Phase{}, fmt.Errorf("inherited producer workflow is absent or changed")
	}
	phase, ok := findPhase(wf, phaseName)
	if !ok {
		return asset.Workflow{}, asset.Phase{}, fmt.Errorf("inherited producer phase %q is absent", phaseName)
	}
	return wf, phase, nil
}

func verifyCurrentReleaseItem(root string,
	item outputbinding.ManifestItem) (outputbinding.ManifestItem, error) {
	data, present, err := readReleaseFileBytes(root, item.Path)
	if err != nil || !present || int64(len(data)) != item.Bytes || artifact.Digest(data) != item.SHA256 {
		if err == nil {
			err = fmt.Errorf("current artifact bytes differ from producing receipt")
		}
		return outputbinding.ManifestItem{}, err
	}
	return item, nil
}

func missingStateError(err error, found bool) error {
	if err != nil {
		return err
	}
	if !found {
		return fmt.Errorf("state is absent")
	}
	return nil
}
