package main

import (
	"context"
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"sort"
	"strings"

	"forgeos/forge-core/internal/approvalcontext"
	"forgeos/forge-core/internal/approvalcontextstore"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/declaredartifact"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/productsource"
)

func fillCryptoRandom(value []byte) error {
	_, err := rand.Read(value)
	return err
}

func (runtime *outputBindingRuntime) prepare(phase asset.Phase, _ string) error {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	delete(runtime.pending, phase.Name)
	if runtime.attempts[phase.Name] == 0 {
		if err := runtime.seedAttemptFromJournal(phase.Name); err != nil {
			return err
		}
	}
	if runtime.attempts[phase.Name] >= 1<<53 {
		return fmt.Errorf("output binding: phase %s attempt limit reached", phase.Name)
	}
	runtime.invalidateFrom(phase.Name)
	attempt := outputBindingAttempt{phase: phase, attempt: runtime.attempts[phase.Name] + 1}
	if err := runtime.capturePreparation(&attempt); err != nil {
		return err
	}
	bytes := make([]byte, 32)
	if err := runtime.randBytes(bytes); err != nil {
		return fmt.Errorf("output binding: generate challenge: %w", err)
	}
	challenge := hex.EncodeToString(bytes)
	if runtime.challenges[challenge] {
		return fmt.Errorf("output binding: generated challenge reuses a prior receipt")
	}
	runtime.challenges[challenge] = true
	runtime.attempts[phase.Name]++
	attempt.challenge = challenge
	runtime.pending[phase.Name] = attempt
	return nil
}

func (runtime *outputBindingRuntime) seedAttemptFromJournal(phase string) error {
	store := outputbindingstore.New(runtime.root)
	claims, err := store.LoadPreflightClaims()
	if err != nil {
		return fmt.Errorf("output binding: load prior preflight claims: %w", err)
	}
	for _, claim := range claims {
		runtime.challenges[claim.Challenge] = true
		runtime.bindings[claim.BindingSHA256] = true
		if claim.RunID == runtime.runID && claim.Workflow == runtime.wf.Stage &&
			claim.Phase == phase && claim.Attempt > runtime.attempts[phase] {
			runtime.attempts[phase] = claim.Attempt
		}
	}
	receipts, err := store.Load()
	if err != nil {
		return fmt.Errorf("output binding: load prior receipts: %w", err)
	}
	for _, receipt := range receipts {
		runtime.challenges[receipt.Challenge] = true
		runtime.bindings[receipt.BindingSHA256] = true
		if receipt.RunID != runtime.runID || receipt.Workflow != runtime.wf.Stage {
			continue
		}
		if receipt.Phase == phase && receipt.Attempt > runtime.attempts[phase] {
			runtime.attempts[phase] = receipt.Attempt
		}
	}
	if len(receipts) > 0 {
		runtime.journalHead = receipts[len(receipts)-1].ReceiptSHA256
	}
	return nil
}

func (runtime *outputBindingRuntime) finalize(phase asset.Phase, _ string, argv []string) ([]string, error) {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	attempt, ok := runtime.pending[phase.Name]
	if !ok {
		return nil, fmt.Errorf("output binding: phase %s has no fresh attempt", phase.Name)
	}
	if attempt.buildErr != nil {
		delete(runtime.pending, phase.Name)
		return nil, fmt.Errorf("output binding: phase %s build observation: %w", phase.Name, attempt.buildErr)
	}
	promptIndex, err := boundPromptIndex(argv)
	if err != nil {
		return nil, err
	}
	if attempt.model == "" || attempt.promptContext == "" {
		return nil, fmt.Errorf("output binding: phase %s build observation is missing", phase.Name)
	}
	if attempt.promptContext != argv[promptIndex] {
		return nil, fmt.Errorf("output binding: finalized prompt differs from observed build prompt")
	}
	attempt.promptContext = outputbinding.SHA256([]byte(argv[promptIndex]))
	if err := runtime.capturePreflight(&attempt); err != nil {
		delete(runtime.pending, phase.Name)
		return nil, err
	}
	if runtime.bindings[attempt.preflight.BindingSHA256] {
		delete(runtime.pending, phase.Name)
		return nil, fmt.Errorf("output binding: preflight binding reuses a prior receipt")
	}
	if err := outputbindingstore.New(runtime.root).ClaimPreflight(attempt.preflight); err != nil {
		delete(runtime.pending, phase.Name)
		return nil, fmt.Errorf("output binding: persist pre-spawn claim: %w", err)
	}
	runtime.bindings[attempt.preflight.BindingSHA256] = true
	trailer := fmt.Sprintf(outputChallengeTrailer, attempt.challenge, attempt.preflight.BindingSHA256)
	argv[promptIndex] += trailer
	attempt.finalPrompt = outputbinding.SHA256([]byte(argv[promptIndex]))
	attempt.claimed = true
	runtime.pending[phase.Name] = attempt
	return argv, nil
}

func (runtime *outputBindingRuntime) recordBuild(
	phase asset.Phase, model, promptText, _ string, frozenReleaseInputs map[string]string,
) {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	attempt, ok := runtime.pending[phase.Name]
	if !ok {
		return
	}
	attempt.model, attempt.promptContext = model, promptText
	if frozenReleaseInputs != nil {
		attempt.buildErr = runtime.bindFrozenReleaseInputs(&attempt, frozenReleaseInputs)
	}
	runtime.pending[phase.Name] = attempt
}

func boundPromptIndex(argv []string) (int, error) {
	if len(argv) < 2 {
		return 0, fmt.Errorf("output binding: command has no prompt argument")
	}
	index := len(argv) - 1
	if argv[index-1] != "-p" && argv[index-1] != "--print" {
		return 0, fmt.Errorf("output binding: prompt must be the terminal -p argument")
	}
	return index, nil
}

func (runtime *outputBindingRuntime) capturePreflight(attempt *outputBindingAttempt) error {
	workflowSHA, snapshot, inputs, _, _, err := runtime.captureBoundary(
		attempt.phase.Name, attempt.artifactInputPaths,
	)
	if err != nil {
		return err
	}
	if workflowSHA != attempt.workflowSHA || !samePreparedSource(attempt.sourceBefore, snapshot) ||
		inputs.ManifestSHA256 != attempt.artifactInputs.ManifestSHA256 {
		return fmt.Errorf("output binding: workflow, product source, or artifact inputs changed between prepare and finalization")
	}
	policy, err := runtime.sealPolicy(attempt.phase, attempt.model)
	if err != nil {
		return err
	}
	preflight, err := outputbinding.SealPreflight(outputbinding.PreflightBinding{
		ArtifactInputsSHA256: inputs.ManifestSHA256, Attempt: attempt.attempt,
		Challenge: attempt.challenge, LocalRuntimePolicySHA256: policy.BindingSHA256,
		Phase: attempt.phase.Name, PromptContextSHA256: attempt.promptContext,
		RunID: runtime.runID, SourceBeforeSHA256: snapshot.SHA256,
		Workflow: runtime.wf.Stage, WorkflowSHA256: runtime.workflowSHA,
	})
	if err != nil {
		return fmt.Errorf("output binding: seal preflight: %w", err)
	}
	attempt.policy, attempt.preflight = policy, preflight
	return nil
}

func (runtime *outputBindingRuntime) capturePreparation(attempt *outputBindingAttempt) error {
	workflowSHA, snapshot, inputs, paths, blocks, err := runtime.captureBoundary(attempt.phase.Name)
	if err != nil {
		return err
	}
	attempt.workflowSHA = workflowSHA
	attempt.sourceBefore = snapshot
	attempt.artifactInputs = inputs
	attempt.artifactInputPaths = paths
	attempt.artifactInputBlocks = blocks
	return nil
}

// captureBoundary is one bounded observation: Prepare and Finalize compare two
// stable captures, but cannot claim an atomic repository pin between them. A
// mutation that persists to either observation fails closed.
func (runtime *outputBindingRuntime) captureBoundary(phase string, exactPaths ...[]string) (
	string, productsource.Snapshot, outputbinding.ArtifactManifest, []string, []string, error,
) {
	workflowSHA, err := runtime.validateLiveWorkflow(runtime.workflowSHA)
	if err != nil {
		return "", productsource.Snapshot{}, outputbinding.ArtifactManifest{}, nil, nil, err
	}
	snapshot, err := productsource.Capture(context.Background(), runtime.root, productSourceEnvironment())
	if err != nil {
		return "", productsource.Snapshot{}, outputbinding.ArtifactManifest{}, nil, nil, err
	}
	paths, err := runtime.presentPriorEmits(snapshot, phase)
	if err != nil {
		return "", productsource.Snapshot{}, outputbinding.ArtifactManifest{}, nil, nil, err
	}
	if len(exactPaths) > 0 {
		paths = append([]string{}, exactPaths[0]...)
	}
	inputs, blocks, err := runtime.capturePromptInputSet(snapshot, paths)
	if err != nil {
		return "", productsource.Snapshot{}, outputbinding.ArtifactManifest{}, nil, nil,
			fmt.Errorf("output binding: capture artifact inputs: %w", err)
	}
	if err := runtime.verifyInputProvenance(phase, inputs); err != nil {
		return "", productsource.Snapshot{}, outputbinding.ArtifactManifest{}, nil, nil, err
	}
	return workflowSHA, snapshot, inputs, paths, blocks, nil
}

func (runtime *outputBindingRuntime) validateLiveWorkflow(want string) (string, error) {
	loaded, err := loadWorkflowNativeOnly(runtime.root, runtime.wf.Stage)
	if err != nil {
		return "", fmt.Errorf("output binding: load live native workflow: %w", err)
	}
	digest := checkpointWorkflowDigest(loaded)
	if digest == "" || digest != want {
		return "", fmt.Errorf("output binding: live native workflow digest changed")
	}
	return digest, nil
}

func samePreparedSource(first, second productsource.Snapshot) bool {
	return productsource.SameCapturedRoot(first, second) && first.SHA256 == second.SHA256 &&
		first.Manifest.SourceRevision == second.Manifest.SourceRevision
}

func productSourceEnvironment() []string {
	return []string{"PATH=" + os.Getenv("PATH")}
}

func (runtime *outputBindingRuntime) captureArtifacts(snapshot productsource.Snapshot, paths []string) (outputbinding.ArtifactManifest, error) {
	paths = append([]string(nil), paths...)
	sort.Strings(paths)
	return declaredartifact.Capture(context.Background(), snapshot, paths)
}

func (runtime *outputBindingRuntime) semanticOutput(raw string) (string, error) {
	if runtime.isClaude {
		return successfulClaudeResultPayload(raw)
	}
	return raw, nil
}

func (runtime *outputBindingRuntime) hooks() executorHooks {
	if runtime == nil {
		return executorHooks{}
	}
	return executorHooks{
		PrepareCommand: runtime.prepare, FinalizeCommand: runtime.finalize,
		SemanticOutput: runtime.semanticOutput, CommitValidatedOutput: runtime.commit,
	}
}

func (runtime *outputBindingRuntime) sealPolicy(phase asset.Phase, model string) (outputbinding.RuntimePolicyBinding, error) {
	policy, err := outputbinding.SealRuntimePolicy(outputbinding.RuntimePolicyBinding{
		ADR: runtime.policy.ADR, Agent: phase.Agent, BuildHalt: runtime.policy.BuildHalt,
		DesignDepth: runtime.policy.DesignDepth, DiscoverDepth: runtime.policy.DiscoverDepth,
		Effect: phase.Effect, EvolveAuthority: runtime.policy.EvolveAuthority,
		EvolveDepth: runtime.policy.EvolveDepth, Executor: strings.TrimSpace(runtime.opts.agentCmd),
		FreshContext: phase.FreshContext, Gates: runtime.policy.Gates,
		Lifecycle: resolveLifecycle(runtime.opts), Materiality: runtime.opts.materiality,
		Mode: runtime.opts.mode, Model: model,
		OutputBindingContract: runtime.wf.OutputBindingContract, Phase: phase.Name,
		Readonly: phase.Readonly, ReviewDepth: runtime.policy.ReviewDepth,
		Reviewer: runtime.policy.Reviewer, Stage: runtime.wf.Stage,
		VerdictContract: phase.VerdictContract, WorkflowSHA256: runtime.workflowSHA,
	})
	if err != nil {
		return outputbinding.RuntimePolicyBinding{}, fmt.Errorf("output binding: seal runtime policy: %w", err)
	}
	return policy, nil
}

func approvalContextFromReceipt(receipt outputbinding.AgentOutputReceipt, createdAt int64) (approvalcontext.Context, error) {
	if err := outputbinding.ValidateReceipt(receipt); err != nil {
		return approvalcontext.Context{}, fmt.Errorf("build approval context: invalid receipt: %w", err)
	}
	if !approvalContextStage(receipt.Workflow) || receipt.RuntimePolicy.Stage != receipt.Workflow ||
		receipt.RuntimePolicy.OutputBindingContract != asset.OutputBindingContractLocalDigestV1 {
		return approvalcontext.Context{}, fmt.Errorf("build approval context: receipt is not a bound approval-stage observation")
	}
	if createdAt < receipt.ObservedAtUnixMS {
		return approvalcontext.Context{}, fmt.Errorf("build approval context: creation time predates receipt")
	}
	value := approvalcontext.Context{
		Format: approvalcontext.ContextFormat, AgentOutputReceiptSHA256: receipt.ReceiptSHA256,
		ArtifactInputsSHA256: receipt.ArtifactInputsSHA256, ArtifactOutputsSHA256: receipt.ArtifactOutputsSHA256,
		CreatedAtUnixMS: createdAt, LocalRuntimePolicySHA256: receipt.LocalRuntimePolicySHA256,
		PromptContextSHA256: receipt.PromptContextSHA256, RunID: receipt.RunID,
		SourceAfterSHA256: receipt.SourceAfterSHA256, Stage: receipt.Workflow,
		Workflow: receipt.Workflow, WorkflowSHA256: receipt.RuntimePolicy.WorkflowSHA256,
	}
	if err := approvalcontext.ValidateContext(value); err != nil {
		return approvalcontext.Context{}, fmt.Errorf("build approval context: %w", err)
	}
	return value, nil
}

func approvalContextStage(stage string) bool {
	return stage == "design" || stage == "deploy" || stage == "rollback"
}

func (runtime *outputBindingRuntime) persistApprovalContext(
	receipt outputbinding.AgentOutputReceipt, semantic string,
) error {
	if runtime == nil || !approvalContextStage(runtime.wf.Stage) || len(runtime.wf.Phases) == 0 ||
		runtime.wf.Phases[len(runtime.wf.Phases)-1].Name != receipt.Phase {
		return nil
	}
	if releaseApprovalStage(runtime.wf.Stage) {
		verdict, ok := parseReviewerVerdict(semantic)
		if !ok || verdict != VerdictApprove {
			return nil
		}
	}
	createdAt := runtime.now().UnixMilli()
	if createdAt < receipt.ObservedAtUnixMS {
		createdAt = receipt.ObservedAtUnixMS
	}
	value, err := approvalContextFromReceipt(receipt, createdAt)
	if err != nil {
		return err
	}
	if _, err := approvalcontextstore.Write(runtime.root, value); err != nil {
		return fmt.Errorf("persist approval context: %w", err)
	}
	return nil
}
