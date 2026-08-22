package main

import (
	"context"
	"fmt"
	"reflect"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/persist"
	"forgeos/forge-core/internal/productsource"
)

func (runtime *outputBindingRuntime) commit(phaseName, raw, semantic string, _ time.Duration) error {
	runtime.mu.Lock()
	defer runtime.mu.Unlock()
	attempt, ok := runtime.pending[phaseName]
	if err := runtime.requireCommitAttempt(phaseName, attempt, ok); err != nil {
		return err
	}
	if err := runtime.validateCommitSemantic(attempt, semantic); err != nil {
		delete(runtime.pending, phaseName)
		return err
	}
	// These checks narrow the observation windows around S2 and publication;
	// they are repeated stable observations, not an atomic filesystem pin.
	if _, err := runtime.validateLiveWorkflow(attempt.workflowSHA); err != nil {
		delete(runtime.pending, phaseName)
		return err
	}
	sourceAfter, outputs, err := runtime.capturePostflight(attempt)
	if err != nil {
		delete(runtime.pending, phaseName)
		return err
	}
	if err := runtime.validateCommitPolicy(attempt); err != nil {
		delete(runtime.pending, phaseName)
		return err
	}
	if _, err := runtime.validateLiveWorkflow(attempt.workflowSHA); err != nil {
		delete(runtime.pending, phaseName)
		return err
	}
	verdict := boundVerdict(attempt.phase, semantic, attempt.preflight.BindingSHA256)
	draft := runtime.receiptDraft(attempt, sourceAfter, outputs, raw, semantic, verdict)
	sealed, err := outputbindingstore.New(runtime.root).Append(draft)
	if err != nil {
		delete(runtime.pending, phaseName)
		return fmt.Errorf("output binding: append accepted receipt: %w", err)
	}
	if err := runtime.persistApprovalContext(sealed, semantic); err != nil {
		delete(runtime.pending, phaseName)
		return fmt.Errorf("output binding: approval context commit: %w", err)
	}
	runtime.accepted[phaseName] = sealed
	runtime.acceptedSemantic[phaseName] = semantic
	runtime.journalHead = sealed.ReceiptSHA256
	delete(runtime.pending, phaseName)
	return nil
}

func (runtime *outputBindingRuntime) requireCommitAttempt(
	phaseName string, attempt outputBindingAttempt, present bool,
) error {
	if !present {
		return fmt.Errorf("output binding: phase %s has no finalized attempt", phaseName)
	}
	if !attempt.claimed {
		delete(runtime.pending, phaseName)
		return fmt.Errorf("output binding: phase %s has no durable pre-spawn claim", phaseName)
	}
	if err := outputbindingstore.New(runtime.root).RequirePreflightClaim(attempt.preflight); err != nil {
		delete(runtime.pending, phaseName)
		return fmt.Errorf("output binding: pre-spawn claim is no longer current: %w", err)
	}
	return nil
}

func (runtime *outputBindingRuntime) capturePostflight(attempt outputBindingAttempt) (
	productsource.Snapshot, outputbinding.ArtifactManifest, error,
) {
	after, err := productsource.Capture(context.Background(), runtime.root, productSourceEnvironment())
	if err != nil {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{}, err
	}
	if !productsource.SameCapturedRoot(attempt.sourceBefore, after) ||
		after.Manifest.SourceRevision != attempt.sourceBefore.Manifest.SourceRevision {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{},
			fmt.Errorf("output binding: repository identity or source revision changed during attempt")
	}
	outputPaths, err := runtime.outputPaths(attempt.phase)
	if err != nil {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{},
			fmt.Errorf("output binding: resolve artifact outputs: %w", err)
	}
	outputs, err := runtime.captureArtifacts(after, outputPaths)
	if err != nil {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{},
			fmt.Errorf("output binding: capture artifact outputs: %w", err)
	}
	if err := runtime.validateCapturedOutputs(attempt.phase, outputs); err != nil {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{},
			fmt.Errorf("output binding: validate captured artifact outputs: %w", err)
	}
	if err := runtime.validateCurrentArtifactInputs(
		after, attempt.artifactInputs, outputPaths,
	); err != nil {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{},
			fmt.Errorf("output binding: artifact inputs changed during attempt: %w", err)
	}
	if err := runtime.verifyInputProvenance(attempt.phase.Name, attempt.artifactInputs); err != nil {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{}, err
	}
	if attempt.phase.Readonly && !sameProductOutsideEmits(
		attempt.sourceBefore.Manifest, after.Manifest, outputPaths,
	) {
		return productsource.Snapshot{}, outputbinding.ArtifactManifest{},
			fmt.Errorf("output binding: readonly phase changed product source outside declared emits")
	}
	return after, outputs, nil
}

func (runtime *outputBindingRuntime) validateCapturedOutputs(
	phase asset.Phase, outputs outputbinding.ArtifactManifest,
) error {
	if runtime.validateOutputs == nil {
		return nil
	}
	return runtime.validateOutputs(phase, outputs)
}

func (runtime *outputBindingRuntime) validateCommitPolicy(attempt outputBindingAttempt) error {
	live := materialityPolicy(runtime.wf, runtime.opts,
		mode.Effective(runtime.opts.mode, resolveLifecycle(runtime.opts)))
	if !reflect.DeepEqual(live, runtime.policy) {
		return fmt.Errorf("output binding: effective runtime policy changed during attempt")
	}
	policy, err := runtime.sealPolicy(attempt.phase, attempt.model)
	if err != nil {
		return err
	}
	if policy.BindingSHA256 != attempt.policy.BindingSHA256 {
		return fmt.Errorf("output binding: effective runtime policy changed during attempt")
	}
	return nil
}

func sameProductOutsideEmits(first, second productsource.Manifest, emits []string) bool {
	if first.APIVersion != second.APIVersion || first.Canonicalization != second.Canonicalization ||
		first.ProfileID != second.ProfileID || first.SourceRevision != second.SourceRevision {
		return false
	}
	allowed := make(map[string]bool, len(emits))
	for _, path := range emits {
		allowed[path] = true
	}
	withoutAllowed := func(manifest productsource.Manifest) []any {
		entries := make([]any, 0, len(manifest.Entries))
		for _, entry := range manifest.Entries {
			if !allowed[entry.Path] {
				entries = append(entries, entry)
			}
		}
		return entries
	}
	return reflect.DeepEqual(withoutAllowed(first), withoutAllowed(second))
}

func (runtime *outputBindingRuntime) receiptDraft(attempt outputBindingAttempt,
	after productsource.Snapshot, outputs outputbinding.ArtifactManifest,
	raw, semantic string, verdict *string) outputbinding.AgentOutputReceipt {
	return outputbinding.AgentOutputReceipt{
		Agent: attempt.phase.Agent, ArtifactInputs: attempt.artifactInputs,
		ArtifactInputsSHA256: attempt.artifactInputs.ManifestSHA256,
		ArtifactOutputs:      outputs, ArtifactOutputsSHA256: outputs.ManifestSHA256,
		Attempt: attempt.attempt, BindingSHA256: attempt.preflight.BindingSHA256,
		Challenge: attempt.challenge, Executor: strings.TrimSpace(runtime.opts.agentCmd),
		FinalPromptSHA256:        attempt.finalPrompt,
		LocalRuntimePolicySHA256: attempt.policy.BindingSHA256, Model: attempt.model,
		ObservedAtUnixMS: runtime.now().UnixMilli(), Phase: attempt.phase.Name,
		PromptContextSHA256: attempt.promptContext,
		RawOutputBytes:      int64(len([]byte(raw))), RawOutputSHA256: outputbinding.SHA256([]byte(raw)),
		RunID: runtime.runID, RuntimePolicy: attempt.policy,
		SemanticOutputBytes: int64(len([]byte(semantic))), SemanticOutputSHA256: outputbinding.SHA256([]byte(semantic)),
		SourceAfterSHA256: after.SHA256, SourceBeforeSHA256: attempt.sourceBefore.SHA256,
		SourceRevision: after.Manifest.SourceRevision, Verdict: verdict,
		Workflow: runtime.wf.Stage,
	}
}

func (runtime *outputBindingRuntime) validateCommitSemantic(attempt outputBindingAttempt, semantic string) error {
	if phaseNeedsDurableSemantic(attempt.phase) &&
		len([]byte(semantic)) > persist.RecoverySemanticOutputMaxBytes {
		return fmt.Errorf("output binding: phase %s recovery semantic output exceeds %d bytes",
			attempt.phase.Name, persist.RecoverySemanticOutputMaxBytes)
	}
	if attempt.phase.VerdictContract != asset.VerdictContractReviewerV2 ||
		!requiredBuildReviewer(runtime.wf, runtime.opts, attempt.phase) {
		return nil
	}
	if _, ok := parseReviewerV2Verdict(semantic, attempt.preflight.BindingSHA256); !ok {
		return fmt.Errorf("output binding: reviewer_v2 requires exact binding echo and verdict")
	}
	return nil
}

func phaseNeedsDurableSemantic(phase asset.Phase) bool { return phase.FeedsForward }

func boundVerdict(phase asset.Phase, semantic, binding string) *string {
	if phase.VerdictContract != asset.VerdictContractReviewerV2 {
		return nil
	}
	verdict, ok := parseReviewerV2Verdict(semantic, binding)
	if !ok {
		return nil
	}
	return &verdict
}
