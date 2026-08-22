package main

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"sort"

	"forgeos/forge-core/internal/approvalcontext"
	"forgeos/forge-core/internal/approvalcontextstore"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/declaredartifact"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/productsource"
	"forgeos/forge-core/internal/statefs"
)

func (runtime *outputBindingRuntime) validateVerdict(phase asset.Phase, verdict string) error {
	if runtime == nil || phase.VerdictContract != asset.VerdictContractReviewerV2 {
		return nil
	}
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	receipt, ok := runtime.accepted[phase.Name]
	if !ok || receipt.Verdict == nil || *receipt.Verdict != verdict {
		return fmt.Errorf("output binding: reviewer verdict lacks its current accepted receipt")
	}
	return runtime.validateCurrentReceipt(receipt, phase)
}

func (runtime *outputBindingRuntime) phaseStart(phase asset.Phase) error {
	return runtime.validateDownstreamBoundary(phase, "start")
}

func (runtime *outputBindingRuntime) phaseComplete(phase asset.Phase) error {
	return runtime.validateDownstreamBoundary(phase, "complete")
}

func (runtime *outputBindingRuntime) agentSpawn(phase asset.Phase) error {
	return runtime.validateDownstreamBoundary(phase, "spawn")
}

func (runtime *outputBindingRuntime) workflowComplete(wf asset.Workflow) error {
	if runtime == nil || wf.Stage != runtime.wf.Stage || len(wf.Phases) == 0 {
		return nil
	}
	return runtime.validateDownstreamBoundary(wf.Phases[len(wf.Phases)-1], "workflow-complete")
}

func (runtime *outputBindingRuntime) validateDownstreamBoundary(phase asset.Phase, boundary string) error {
	if runtime == nil || !strictBuildReview(runtime.wf, runtime.opts) {
		return nil
	}
	reviewer, reviewerIndex, ok := runtime.reviewer()
	if !ok || runtime.phaseIndex(phase.Name) < reviewerIndex ||
		(phase.Name == reviewer.Name && boundary == "start") {
		return nil
	}
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	receipt, exists := runtime.accepted[reviewer.Name]
	if !exists || receipt.Verdict == nil || *receipt.Verdict != VerdictApprove {
		if phase.Name == reviewer.Name {
			return nil // REQUEST_CHANGES is validated by the required-verdict transition.
		}
		return fmt.Errorf("output binding: phase %s cannot cross %s without a current reviewer_v2 approval", phase.Name, boundary)
	}
	if err := runtime.validateCurrentReceipt(receipt, reviewer); err != nil {
		return fmt.Errorf("output binding: phase %s %s review freshness: %w", phase.Name, boundary, err)
	}
	return nil
}

func (runtime *outputBindingRuntime) reviewer() (asset.Phase, int, bool) {
	for index, phase := range runtime.wf.Phases {
		if phase.VerdictContract == asset.VerdictContractReviewerV2 {
			return phase, index, true
		}
	}
	return asset.Phase{}, -1, false
}

func (runtime *outputBindingRuntime) validateCurrentReceipt(receipt outputbinding.AgentOutputReceipt, phase asset.Phase) error {
	if receipt.RunID != runtime.runID || receipt.Workflow != runtime.wf.Stage || receipt.Phase != phase.Name {
		return fmt.Errorf("receipt identity does not match current run/workflow/phase")
	}
	if err := outputbindingstore.New(runtime.root).RequireReceiptClaim(receipt); err != nil {
		return fmt.Errorf("receipt pre-spawn claim is not current: %w", err)
	}
	if err := runtime.receiptStillInJournal(receipt.ReceiptSHA256); err != nil {
		return err
	}
	loaded, err := loadWorkflowNativeOnly(runtime.root, runtime.wf.Stage)
	if err != nil {
		return fmt.Errorf("load reviewed workflow: %w", err)
	}
	if checkpointWorkflowDigest(loaded) != runtime.workflowSHA {
		return fmt.Errorf("workflow no longer matches reviewed policy")
	}
	current, err := productsource.Capture(context.Background(), runtime.root, productSourceEnvironment())
	if err != nil {
		return err
	}
	if current.SHA256 != receipt.SourceAfterSHA256 || current.Manifest.SourceRevision != receipt.SourceRevision {
		return fmt.Errorf("product source no longer matches reviewed bytes")
	}
	policy, err := runtime.sealPolicy(phase, receipt.Model)
	if err != nil {
		return err
	}
	if policy.BindingSHA256 != receipt.LocalRuntimePolicySHA256 {
		return fmt.Errorf("effective runtime policy no longer matches review")
	}
	if err := runtime.validateCurrentArtifactInputs(
		current, receipt.ArtifactInputs, manifestPaths(receipt.ArtifactOutputs),
	); err != nil {
		return fmt.Errorf("review artifact inputs no longer match: %w", err)
	}
	return runtime.verifyInputProvenance(phase.Name, receipt.ArtifactInputs)
}

func (runtime *outputBindingRuntime) receiptStillInJournal(want string) error {
	receipts, err := outputbindingstore.New(runtime.root).Load()
	if err != nil {
		return fmt.Errorf("load output receipt journal: %w", err)
	}
	if len(receipts) == 0 || runtime.journalHead == "" ||
		receipts[len(receipts)-1].ReceiptSHA256 != runtime.journalHead {
		return fmt.Errorf("output receipt journal head no longer matches the live run")
	}
	for _, receipt := range receipts {
		if receipt.ReceiptSHA256 == want {
			return nil
		}
	}
	return fmt.Errorf("accepted receipt is absent from the durable journal")
}

type verifiedApprovalContext struct {
	Context       approvalcontext.Context
	ContextSHA256 string
	Receipt       outputbinding.AgentOutputReceipt
	Workflow      asset.Workflow
}

// verifyBoundApprovalContext strictly replays every local digest observation
// used by a positive approval. Two complete passes narrow context/journal and
// worktree swap windows; they are observations, not an authority assertion.
func verifyBoundApprovalContext(root, stage string) (verifiedApprovalContext, error) {
	first, err := verifyApprovalContextPass(root, stage)
	if err != nil {
		return verifiedApprovalContext{}, err
	}
	second, err := verifyApprovalContextPass(root, stage)
	if err != nil {
		return verifiedApprovalContext{}, err
	}
	if first.ContextSHA256 != second.ContextSHA256 ||
		first.Receipt.ReceiptSHA256 != second.Receipt.ReceiptSHA256 ||
		checkpointWorkflowDigest(first.Workflow) != checkpointWorkflowDigest(second.Workflow) {
		return verifiedApprovalContext{}, fmt.Errorf("approval context changed while being verified")
	}
	return second, nil
}

func verifyApprovalContextPass(root, stage string) (verifiedApprovalContext, error) {
	value, digest, err := approvalcontextstore.Load(root, stage)
	if err != nil {
		return verifiedApprovalContext{}, fmt.Errorf("verify approval context: %w", err)
	}
	receipt, err := approvalContextHeadReceipt(root, value)
	if err != nil {
		return verifiedApprovalContext{}, err
	}
	wf, bound, err := loadBoundApprovalWorkflow(root, stage)
	if err != nil || !bound {
		if err == nil {
			err = fmt.Errorf("workflow does not select local_digest_v1")
		}
		return verifiedApprovalContext{}, fmt.Errorf("verify approval context workflow: %w", err)
	}
	if err := verifyApprovalWorkflow(value, receipt, wf); err != nil {
		return verifiedApprovalContext{}, err
	}
	if err := verifyApprovalLiveBytes(root, value, receipt); err != nil {
		return verifiedApprovalContext{}, err
	}
	return verifiedApprovalContext{Context: value, ContextSHA256: digest, Receipt: receipt, Workflow: wf}, nil
}

func approvalContextHeadReceipt(root string, value approvalcontext.Context) (outputbinding.AgentOutputReceipt, error) {
	receipts, err := outputbindingstore.New(root).Load()
	if err != nil {
		return outputbinding.AgentOutputReceipt{}, fmt.Errorf("verify approval context journal: %w", err)
	}
	if len(receipts) == 0 {
		return outputbinding.AgentOutputReceipt{}, fmt.Errorf("verify approval context journal: receipt journal is empty")
	}
	head := receipts[len(receipts)-1]
	if head.ReceiptSHA256 != value.AgentOutputReceiptSHA256 {
		return outputbinding.AgentOutputReceipt{}, fmt.Errorf("verify approval context journal: context receipt is not the current head")
	}
	if err := verifyContextReceiptFields(value, head); err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	if err := outputbindingstore.New(root).RequireReceiptClaim(head); err != nil {
		return outputbinding.AgentOutputReceipt{}, fmt.Errorf("verify approval context preflight claim: %w", err)
	}
	return head, nil
}

func verifyContextReceiptFields(value approvalcontext.Context, receipt outputbinding.AgentOutputReceipt) error {
	if receipt.RunID != value.RunID || receipt.Workflow != value.Workflow ||
		value.Stage != value.Workflow || receipt.RuntimePolicy.Stage != value.Stage {
		return fmt.Errorf("verify approval context journal: receipt run/workflow/stage identity differs")
	}
	if value.CreatedAtUnixMS < receipt.ObservedAtUnixMS {
		return fmt.Errorf("verify approval context journal: context predates its receipt")
	}
	if value.ArtifactInputsSHA256 != receipt.ArtifactInputsSHA256 ||
		value.ArtifactOutputsSHA256 != receipt.ArtifactOutputsSHA256 ||
		value.LocalRuntimePolicySHA256 != receipt.LocalRuntimePolicySHA256 ||
		value.PromptContextSHA256 != receipt.PromptContextSHA256 ||
		value.SourceAfterSHA256 != receipt.SourceAfterSHA256 ||
		value.WorkflowSHA256 != receipt.RuntimePolicy.WorkflowSHA256 {
		return fmt.Errorf("verify approval context journal: context digest fields differ from receipt")
	}
	return nil
}

func loadBoundApprovalWorkflow(root, stage string) (asset.Workflow, bool, error) {
	if !approvalContextStage(stage) {
		return asset.Workflow{}, false, nil
	}
	want := filepath.Join(".agent", "workflows", stage+".yml")
	path, relative, err := containedRepoPath(root, want)
	if err != nil {
		return asset.Workflow{}, false, fmt.Errorf("strict approval workflow path: %w", err)
	}
	if filepath.Clean(relative) != filepath.Clean(want) {
		return asset.Workflow{}, false, fmt.Errorf("strict approval workflow relative path differs")
	}
	if _, err := os.Lstat(path); err != nil {
		if os.IsNotExist(err) {
			return asset.Workflow{}, false, nil
		}
		return asset.Workflow{}, false, err
	}
	if err := validateReadonlyEmitIdentity(root, path); err != nil {
		return asset.Workflow{}, false, fmt.Errorf("strict approval workflow identity: %w", err)
	}
	source, _, present, err := statefs.ReadTracked(path, 4<<20)
	if err != nil || !present {
		return asset.Workflow{}, false, fmt.Errorf("strict approval workflow read: %w", err)
	}
	if err := validateReadonlyEmitIdentity(root, path); err != nil {
		return asset.Workflow{}, false, fmt.Errorf("strict approval workflow identity changed: %w", err)
	}
	wf, parsed, err := parseNativeWorkflow(source)
	if err != nil {
		return asset.Workflow{}, false, err
	}
	if !parsed || len(wf.Phases) == 0 {
		return asset.Workflow{}, false, fmt.Errorf("native approval workflow is not executable")
	}
	wf, err = validateWorkflowIdentity(stage, wf)
	if err != nil {
		return asset.Workflow{}, false, err
	}
	return wf, wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1, nil
}

func verifyApprovalWorkflow(value approvalcontext.Context, receipt outputbinding.AgentOutputReceipt, wf asset.Workflow) error {
	if wf.Stage != value.Stage || checkpointWorkflowDigest(wf) != value.WorkflowSHA256 {
		return fmt.Errorf("verify approval context workflow: native workflow digest differs")
	}
	if len(wf.Phases) == 0 || wf.Phases[len(wf.Phases)-1].Name != receipt.Phase {
		return fmt.Errorf("verify approval context workflow: receipt is not the terminal agent phase")
	}
	phase := wf.Phases[len(wf.Phases)-1]
	if err := verifyEmbeddedRuntimePolicy(receipt.RuntimePolicy, wf, phase); err != nil {
		return fmt.Errorf("verify approval context policy: %w", err)
	}
	return nil
}

func verifyEmbeddedRuntimePolicy(policy outputbinding.RuntimePolicyBinding, wf asset.Workflow,
	phase asset.Phase, materialities ...string) error {
	expectedMateriality := policy.Materiality
	if len(materialities) > 0 {
		expectedMateriality = materialities[0]
	}
	effective := materialityPolicy(wf, runOpts{materiality: expectedMateriality},
		mode.Effective(policy.Mode, policy.Lifecycle))
	if policy.WorkflowSHA256 != checkpointWorkflowDigest(wf) || policy.Stage != wf.Stage ||
		policy.OutputBindingContract != wf.OutputBindingContract || policy.Phase != phase.Name ||
		policy.Agent != phase.Agent || policy.Effect != phase.Effect ||
		policy.FreshContext != phase.FreshContext || policy.Readonly != phase.Readonly ||
		policy.VerdictContract != phase.VerdictContract || policy.Materiality != expectedMateriality {
		return fmt.Errorf("workflow/phase fields differ from embedded policy")
	}
	if policy.ADR != effective.ADR || policy.BuildHalt != effective.BuildHalt ||
		policy.DesignDepth != effective.DesignDepth || policy.DiscoverDepth != effective.DiscoverDepth ||
		policy.EvolveAuthority != effective.EvolveAuthority || policy.EvolveDepth != effective.EvolveDepth ||
		policy.ReviewDepth != effective.ReviewDepth || policy.Reviewer != effective.Reviewer ||
		!sameApprovalPolicyGates(policy.Gates, effective.Gates) {
		return fmt.Errorf("embedded mode/lifecycle policy is inconsistent")
	}
	return nil
}

func verifyApprovalLiveBytes(root string, value approvalcontext.Context, receipt outputbinding.AgentOutputReceipt) error {
	snapshot, err := productsource.Capture(context.Background(), root, productSourceEnvironment())
	if err != nil {
		return fmt.Errorf("verify approval context source: %w", err)
	}
	if snapshot.SHA256 != value.SourceAfterSHA256 ||
		snapshot.Manifest.SourceRevision != receipt.SourceRevision {
		return fmt.Errorf("verify approval context source: current product source or revision differs")
	}
	inputsWant, err := currentApprovalInputs(receipt.ArtifactInputs, receipt.ArtifactOutputs)
	if err != nil {
		return fmt.Errorf("verify approval context artifact inputs: %w", err)
	}
	inputs, err := recaptureApprovalManifest(snapshot, inputsWant)
	if err != nil {
		return fmt.Errorf("verify approval context artifact inputs: %w", err)
	}
	outputs, err := recaptureApprovalManifest(snapshot, receipt.ArtifactOutputs)
	if err != nil {
		return fmt.Errorf("verify approval context artifact outputs: %w", err)
	}
	if !reflect.DeepEqual(inputs, inputsWant) ||
		!reflect.DeepEqual(outputs, receipt.ArtifactOutputs) {
		return fmt.Errorf("verify approval context artifacts: current manifests differ")
	}
	return nil
}

// An accepted phase may read the old bytes of a path and then replace that
// same declared path. The receipt/preflight retain those historical input
// bytes; live freshness can only recapture non-overlapping inputs and every
// current output.
func currentApprovalInputs(inputs, outputs outputbinding.ArtifactManifest) (outputbinding.ArtifactManifest, error) {
	outputPaths := make(map[string]bool, len(outputs.Items))
	for _, item := range outputs.Items {
		outputPaths[item.Path] = true
	}
	items := make([]outputbinding.ManifestItem, 0, len(inputs.Items))
	for _, item := range inputs.Items {
		if !outputPaths[item.Path] {
			items = append(items, item)
		}
	}
	return outputbinding.SealManifest(items)
}

func sameApprovalPolicyGates(first, second []string) bool {
	second = append([]string(nil), second...)
	sort.Strings(second)
	return reflect.DeepEqual(first, second)
}

func recaptureApprovalManifest(snapshot productsource.Snapshot, want outputbinding.ArtifactManifest) (outputbinding.ArtifactManifest, error) {
	paths := make([]string, len(want.Items))
	for index, item := range want.Items {
		paths[index] = item.Path
	}
	return declaredartifact.Capture(context.Background(), snapshot, paths)
}
