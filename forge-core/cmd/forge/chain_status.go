package main

import (
	"bytes"
	"context"
	"fmt"
	"strings"
	"time"

	"forgeos/forge-core/internal/approvalcontext"
	"forgeos/forge-core/internal/approvalcontextstore"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/materiality"
	"forgeos/forge-core/internal/outputbindingstore"
)

func resumeMaterialityExplicit(o runOpts) bool {
	if o.runFlagsCaptured {
		return o.materialityExplicit
	}
	return o.materiality != "" && o.materiality != materiality.Unbound
}

// validateChainRunOptionConflicts runs before persisted workflow reconstruction
// so a conflicting invocation cannot reach repository-owned executable fallbacks.
func validateChainRunOptionConflicts(o runOpts, state chainState, resolvedLifecycle string) error {
	maxAgentCallsSet := o.maxAgentCalls != 0
	modeSet := o.mode != "balanced"
	lifecycleSet := o.lifecycle != ""
	materialitySet := resumeMaterialityExplicit(o)
	maxChainStagesSet := o.maxChainStages != defaultMaxChainStages
	runBudgetSet := strings.TrimSpace(o.runBudgetUSD) != ""
	if o.runFlagsCaptured {
		maxAgentCallsSet = o.maxAgentCallsExplicit
		modeSet = o.modeExplicit
		lifecycleSet = o.lifecycleExplicit
		maxChainStagesSet = o.maxChainStagesExplicit
		runBudgetSet = o.runBudgetExplicit
		if err := validateStaleChainSelectors(o, state); err != nil {
			return err
		}
	}
	if maxAgentCallsSet && o.maxAgentCalls != state.MaxAgentCalls {
		return fmt.Errorf("persisted --max-agent-calls=%d conflicts with requested %d", state.MaxAgentCalls, o.maxAgentCalls)
	}
	if modeSet && o.mode != state.Mode {
		return fmt.Errorf("persisted --mode=%s conflicts with requested %s", state.Mode, o.mode)
	}
	if lifecycleSet && resolvedLifecycle != state.Lifecycle {
		return fmt.Errorf("persisted --lifecycle=%s conflicts with requested %s", state.Lifecycle, resolvedLifecycle)
	}
	if materialitySet && o.materiality != state.Materiality {
		return fmt.Errorf("persisted --materiality=%s conflicts with requested %s", state.Materiality, o.materiality)
	}
	if maxChainStagesSet && o.maxChainStages != state.MaxChainStages {
		return fmt.Errorf("persisted --max-chain-stages=%d conflicts with requested %d", state.MaxChainStages, o.maxChainStages)
	}
	if runBudgetSet {
		requested, err := newRunBudget(o.runBudgetUSD)
		if err != nil {
			return err
		}
		if capMicros := requested.CapUsdMicros(); capMicros != state.BudgetCapMicros {
			return fmt.Errorf(
				"persisted run budget cap is %d micro-USD, but requested --run-budget-usd resolves to %d",
				state.BudgetCapMicros, capMicros,
			)
		}
	}
	return nil
}

func validateStaleChainSelectors(o runOpts, state chainState) error {
	persistedMode := projectYAMLValue(o.root, "mode")
	if !o.modeExplicit && persistedMode != "" && persistedMode != state.Mode {
		return fmt.Errorf(
			"persisted chain mode=%q is stale against current project mode=%q; "+
				"retry with an explicit matching --mode to resume intentionally",
			state.Mode, persistedMode,
		)
	}
	persistedLifecycle := projectYAMLValue(o.root, "lifecycle")
	if !o.lifecycleExplicit &&
		persistedLifecycle != "" &&
		persistedLifecycle != state.Lifecycle {
		return fmt.Errorf(
			"persisted chain lifecycle=%q is stale against current project lifecycle=%q; "+
				"retry with an explicit matching --lifecycle to resume intentionally",
			state.Lifecycle, persistedLifecycle,
		)
	}
	return nil
}

type chainStatusDisplay struct {
	Status          string   `json:"status,omitempty"`
	CurrentStage    string   `json:"current_stage,omitempty"`
	CompletedStages []string `json:"completed_stages,omitempty"`
	Reason          string   `json:"reason,omitempty"`
	AgentCalls      int      `json:"agent_calls,omitempty"`
	RunID           string   `json:"run_id,omitempty"`
	Materiality     string   `json:"materiality,omitempty"`
	UpdatedAtUnix   int64    `json:"updated_at_unix,omitempty"`
	Error           string   `json:"error,omitempty"`
}

func chainStatusForDisplay(root string) *chainStatusDisplay {
	if err := rejectTrackedForgeControlState(root); err != nil {
		return &chainStatusDisplay{Error: err.Error()}
	}
	state, found, err := loadChainState(root)
	if err != nil {
		return &chainStatusDisplay{Error: err.Error()}
	}
	if !found {
		return nil
	}
	return &chainStatusDisplay{
		Status: state.Status, CurrentStage: state.CurrentStage,
		CompletedStages: state.CompletedStages, Reason: state.Reason,
		AgentCalls: state.AgentCalls, RunID: state.RunID, Materiality: state.Materiality,
		UpdatedAtUnix: state.UpdatedAtUnix,
	}
}

func printChainStatusText(root string) {
	state := chainStatusForDisplay(root)
	if state == nil {
		return
	}
	if state.Error != "" {
		fmt.Printf("  chain: unreadable (%s)\n", state.Error)
		return
	}
	current, completed := state.CurrentStage, "-"
	if current == "" {
		current = "-"
	}
	if len(state.CompletedStages) > 0 {
		completed = strings.Join(state.CompletedStages, " → ")
	}
	age := ""
	if state.UpdatedAtUnix > 0 {
		age = fmt.Sprintf(" updated=%s", time.Unix(state.UpdatedAtUnix, 0).Format(time.RFC3339))
	}
	materiality := state.Materiality
	if materiality == "" {
		materiality = "unknown (diagnostic-only)"
	}
	fmt.Printf("  chain: status=%s current=%s completed=[%s] materiality=%s agent_calls=%d run_id=%s%s\n",
		state.Status, current, completed, materiality, state.AgentCalls, state.RunID, age)
	if state.Reason != "" {
		fmt.Printf("    reason: %s\n", state.Reason)
	}
}

// reportConvergence evaluates the stop condition against the same live signals
// used by gate phases and returns whether the workflow may advance. ctx/opts
// bound the live gate spawns gatherSignals can trigger (R6 site 4).
func reportConvergence(ctx context.Context, opts gate.Options, wf asset.Workflow, root string, probe, categories map[string]string, lifecycle string, approvedFlag bool, verdicts *verdictLedger, gateSet ...[]string) bool {
	if wf.Stop.Type == "" {
		return false
	}
	approved := humanApproved(root, wf.Stage, approvedFlag)
	signals := gatherSignals(ctx, opts, root, wf, probe, categories, lifecycle, approved, verdicts, gateSet...)
	return reportConvergenceSignals(wf, signals)
}

func reportStageConvergence(ctx context.Context, opts gate.Options, wf asset.Workflow, root string, probe, categories map[string]string, lifecycle string, approvedFlag bool, verdicts *verdictLedger, gates []string, proposalStage, releaseStage bool) bool {
	switch {
	case proposalStage:
		approved := humanApproved(root, wf.Stage, approvedFlag)
		return reportConvergenceSignals(wf, proposalLoopSignals(root, wf, approved, verdicts))
	case releaseStage:
		return reportConvergenceSignals(wf, converge.Signals{
			HumanApproved: humanApproved(root, wf.Stage, approvedFlag),
		})
	default:
		return reportConvergence(ctx, opts, wf, root, probe, categories, lifecycle, approvedFlag, verdicts, gates)
	}
}

func reportConvergenceSignals(wf asset.Workflow, signals converge.Signals) bool {
	if wf.Stop.Type == "" {
		return false
	}
	results, met := converge.Converge(wf.Stop, signals)
	if converge.IsHumanGate(wf.Stop) {
		reportHumanGate(wf, met)
		return met
	}
	fmt.Printf("convergence: %s (%s)\n", verdict(met), wf.Stop.Type)
	for _, result := range results {
		fmt.Printf("  [%s] %s — %s\n", mark(result.Met), result.Expr, result.Detail)
	}
	return met
}

func reportHumanGate(wf asset.Workflow, approved bool) {
	if !approved {
		fmt.Printf("convergence: NOT MET (human_gate) — awaiting human approval (non-bypassable)\n")
		if releaseApprovalStage(wf.Stage) {
			fmt.Println("  run `forge approve " + wf.Stage + "` after verifying external execution evidence; --approved is not accepted for delivery stages")
		} else {
			fmt.Println("  pass --approved or run `forge approve " + wf.Stage + "` to grant approval")
		}
		return
	}
	fmt.Printf("convergence: MET (human_gate) — approved → unlocks %s\n", nextStageLabel(wf.Stop))
}

func nextStageLabel(stop asset.StopCondition) string {
	if stop.OnApproved.NextStage == "" {
		return "(no next_stage declared)"
	}
	return "next_stage=" + stop.OnApproved.NextStage
}

func verdict(met bool) string { return pick(met, "MET", "NOT MET") }
func mark(met bool) string    { return pick(met, "x", " ") }
func pick(condition bool, yes, no string) string {
	if condition {
		return yes
	}
	return no
}

func validateBoundChainRecovery(root string, state chainState) error {
	if state.Format != chainStateFormat {
		return nil
	}
	if len(state.BoundStages) == 0 {
		if len(state.PhaseReceipts)+len(state.StageReceipts)+len(state.ApprovalContexts) != 0 {
			return fmt.Errorf("unbound chain state carries bound recovery references")
		}
		return nil
	}
	index, err := loadRecoveryReceiptIndex(root)
	if err != nil {
		return err
	}
	if state.ReceiptHead != index.head {
		return fmt.Errorf("persisted receipt journal head differs from the complete live journal")
	}
	workflows, err := loadRecoveryWorkflows(root, state)
	if err != nil {
		return err
	}
	reworking, err := recoveryReworkStage(root, state)
	if err != nil {
		return err
	}
	if err := validateRecoveryReferenceSet(root, index, state, workflows, reworking); err != nil {
		return err
	}
	return validateRecoveryApprovalSet(root, state, workflows, index, reworking)
}

func recoveryReworkStage(root string, state chainState) (string, error) {
	if state.Status != "waiting_approval" || state.CurrentStage == "" {
		return "", nil
	}
	rejected, err := rejectionMarkerExists(root, state.CurrentStage)
	if err != nil {
		return "", err
	}
	if rejected {
		return state.CurrentStage, nil
	}
	return "", nil
}

func loadRecoveryWorkflows(root string, state chainState) (map[string]asset.Workflow, error) {
	stages := state.BoundStages
	workflows := make(map[string]asset.Workflow, len(stages))
	for _, stage := range stages {
		wf, err := loadWorkflowNativeOnly(root, stage)
		if err != nil {
			return nil, fmt.Errorf("load recovery workflow %q: %w", stage, err)
		}
		if checkpointWorkflowDigest(wf) != state.WorkflowDigests[stage] {
			return nil, fmt.Errorf("recovery workflow %q digest differs", stage)
		}
		if wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1 &&
			state.Format != chainStateFormat {
			return nil, fmt.Errorf("chain state %s is diagnostic-only for bound workflow %q", state.Format, stage)
		}
		workflows[stage] = wf
	}
	return workflows, nil
}

func recoveryStages(state chainState) []string {
	result := make([]string, 0, len(state.CompletedStages)+len(state.InheritedStages)+1)
	seen := map[string]bool{}
	stages := append(append([]string{}, state.CompletedStages...), state.InheritedStages...)
	for _, stage := range stages {
		if !seen[stage] {
			seen[stage], result = true, append(result, stage)
		}
	}
	if state.CurrentStage != "" && !seen[state.CurrentStage] {
		result = append(result, state.CurrentStage)
	}
	return result
}

func validateRecoveryReferenceSet(root string, index recoveryReceiptIndex, state chainState,
	workflows map[string]asset.Workflow, reworking string) error {
	wantPhase, wantStage := map[string]bool{}, map[string]bool{}
	for stage, wf := range workflows {
		complete := recoveryStageMustBeComplete(state, stage) && stage != reworking
		if wf.OutputBindingContract != asset.OutputBindingContractLocalDigestV1 || !complete {
			continue
		}
		phases, err := expectedBoundCommandPhases(wf, state.Mode, state.Lifecycle, state.Materiality)
		if err != nil || len(phases) == 0 {
			return fmt.Errorf("bound recovery stage %s has no recoverable command traversal", stage)
		}
		if err := validateStageReferenceSet(root, index, state, wf, phases, wantPhase); err != nil {
			return err
		}
		if state.Status == "waiting_approval" && state.CurrentStage == stage {
			terminal := index.byDigest[state.StageReceipts[stage]]
			if err := verifyRecoveryReceiptLive(root, terminal); err != nil {
				return fmt.Errorf("current stage %s terminal receipt is stale: %w", stage, err)
			}
		}
		wantStage[stage] = true
	}
	phaseRefs := recoveryRefsWithoutStage(state.PhaseReceipts, reworking)
	stageRefs := recoveryRefsWithoutStage(state.StageReceipts, reworking)
	if !sameRecoveryKeys(phaseRefs, wantPhase) || !sameRecoveryKeys(stageRefs, wantStage) {
		return fmt.Errorf("persisted receipt reference maps are not exact for completed bound stages")
	}
	return nil
}

func recoveryStageMustBeComplete(state chainState, stage string) bool {
	if state.Status == "waiting_approval" && state.CurrentStage == stage {
		return true
	}
	stages := append(append([]string{}, state.CompletedStages...), state.InheritedStages...)
	for _, candidate := range stages {
		if candidate == stage {
			return true
		}
	}
	return false
}

func validateStageReferenceSet(root string, index recoveryReceiptIndex, state chainState,
	wf asset.Workflow, phases []asset.Phase, want map[string]bool) error {
	var prior int64
	for _, phase := range phases {
		key := phaseReceiptKey(wf.Stage, phase.Name)
		want[key] = true
		digest, ok := state.PhaseReceipts[key]
		receipt, present := index.byDigest[digest]
		latest, latestOK := index.latest(state.RunID, wf.Stage, phase.Name)
		if !ok || !present || !latestOK || latest.ReceiptSHA256 != digest {
			return fmt.Errorf("persisted phase receipt %q is absent, stale, or not latest", key)
		}
		if err := verifyRecoveryReceiptIdentity(root, receipt, wf, phase, state); err != nil {
			return fmt.Errorf("persisted phase receipt %q: %w", key, err)
		}
		if receipt.LedgerSequence <= prior {
			return fmt.Errorf("persisted phase receipts for %s are not in traversal order", wf.Stage)
		}
		prior = receipt.LedgerSequence
	}
	terminal := state.PhaseReceipts[phaseReceiptKey(wf.Stage, phases[len(phases)-1].Name)]
	if state.StageReceipts[wf.Stage] != terminal {
		return fmt.Errorf("persisted stage receipt %q is not its terminal phase receipt", wf.Stage)
	}
	return nil
}

func sameRecoveryKeys(values map[string]string, want map[string]bool) bool {
	if len(values) != len(want) {
		return false
	}
	for key := range values {
		if !want[key] {
			return false
		}
	}
	return true
}

func validateRecoveryApprovalSet(root string, state chainState,
	workflows map[string]asset.Workflow, index recoveryReceiptIndex, reworking string) error {
	want := map[string]bool{}
	for stage, wf := range workflows {
		bound := wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1
		complete := recoveryStageMustBeComplete(state, stage)
		if !bound || !approvalContextStage(stage) || !complete || stage == reworking {
			continue
		}
		want[stage] = true
		if state.Status == "waiting_approval" && state.CurrentStage == stage {
			verified, err := verifyBoundApprovalContext(root, stage)
			if err != nil || verified.ContextSHA256 != state.ApprovalContexts[stage] {
				return fmt.Errorf("current approval context for %s is absent or stale", stage)
			}
			continue
		}
		if err := verifyHistoricalApprovalContext(root, state, stage, index); err != nil {
			return err
		}
	}
	contexts := recoveryRefsWithoutStage(state.ApprovalContexts, reworking)
	if !sameRecoveryKeys(contexts, want) {
		return fmt.Errorf("persisted approval context map is not exact for bound approval stages")
	}
	return nil
}

func recoveryRefsWithoutStage(values map[string]string, stage string) map[string]string {
	if stage == "" {
		return values
	}
	return filterStringMap(values, func(key string) bool {
		return key != stage && !strings.HasPrefix(key, stage+"/")
	})
}

func verifyHistoricalApprovalContext(root string, state chainState, stage string,
	index recoveryReceiptIndex) error {
	value, digest, err := approvalcontextstore.Load(root, stage)
	if err != nil || digest != state.ApprovalContexts[stage] {
		return fmt.Errorf("historical approval context for %s is absent or changed", stage)
	}
	receipt, ok := index.byDigest[state.StageReceipts[stage]]
	if !ok || value.AgentOutputReceiptSHA256 != receipt.ReceiptSHA256 {
		return fmt.Errorf("historical approval context for %s does not reference its stage receipt", stage)
	}
	if err := verifyContextReceiptFields(value, receipt); err != nil {
		return err
	}
	if err := outputbindingstore.New(root).RequireReceiptClaim(receipt); err != nil {
		return fmt.Errorf("historical approval receipt claim for %s: %w", stage, err)
	}
	if !releaseRejectionMarkerAbsent(root, stage) {
		return fmt.Errorf("historical positive approval marker for %s conflicts with rejection", stage)
	}
	markerBytes, marker, err := readBoundPositiveMarker(root, stage)
	if err != nil || len(markerBytes) == 0 {
		return fmt.Errorf("historical positive approval marker for %s is unavailable", stage)
	}
	if err := approvalcontext.ValidateMarkerContext(marker, value); err != nil {
		return fmt.Errorf("historical positive approval marker for %s: %w", stage, err)
	}
	if marker.CreatedAtUnixMS < value.CreatedAtUnixMS {
		return fmt.Errorf("historical positive approval marker for %s predates its context", stage)
	}
	if releaseApprovalStage(stage) {
		if err := verifyHistoricalBoundReleaseValidationReceipt(root, stage, value, digest, receipt); err != nil {
			return fmt.Errorf("historical release approval for %s: %w", stage, err)
		}
	}
	current, currentDigest, err := approvalcontextstore.Load(root, stage)
	secondMarkerBytes, _, markerErr := readBoundPositiveMarker(root, stage)
	if err != nil || currentDigest != digest || current != value || markerErr != nil ||
		!bytes.Equal(markerBytes, secondMarkerBytes) || !releaseRejectionMarkerAbsent(root, stage) {
		return fmt.Errorf("historical approval context or marker for %s changed while being verified", stage)
	}
	return nil
}
