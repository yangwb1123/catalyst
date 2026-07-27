package main

import (
	"fmt"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

// validateChainRunOptionConflicts runs before persisted workflow reconstruction
// so a conflicting invocation cannot reach repository-owned executable fallbacks.
func validateChainRunOptionConflicts(o runOpts, state chainState, resolvedLifecycle string) error {
	maxAgentCallsSet := o.maxAgentCalls != 0
	modeSet := o.mode != "balanced"
	lifecycleSet := o.lifecycle != ""
	maxChainStagesSet := o.maxChainStages != defaultMaxChainStages
	runBudgetSet := strings.TrimSpace(o.runBudgetUSD) != ""
	if o.runFlagsCaptured {
		maxAgentCallsSet = o.maxAgentCallsExplicit
		modeSet = o.modeExplicit
		lifecycleSet = o.lifecycleExplicit
		maxChainStagesSet = o.maxChainStagesExplicit
		runBudgetSet = o.runBudgetExplicit
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

type chainStatusDisplay struct {
	Status          string   `json:"status,omitempty"`
	CurrentStage    string   `json:"current_stage,omitempty"`
	CompletedStages []string `json:"completed_stages,omitempty"`
	Reason          string   `json:"reason,omitempty"`
	AgentCalls      int      `json:"agent_calls,omitempty"`
	RunID           string   `json:"run_id,omitempty"`
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
		AgentCalls: state.AgentCalls, RunID: state.RunID, UpdatedAtUnix: state.UpdatedAtUnix,
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
	fmt.Printf("  chain: status=%s current=%s completed=[%s] agent_calls=%d run_id=%s%s\n",
		state.Status, current, completed, state.AgentCalls, state.RunID, age)
	if state.Reason != "" {
		fmt.Printf("    reason: %s\n", state.Reason)
	}
}

// reportConvergence evaluates the stop condition against the same live signals
// used by gate phases and returns whether the workflow may advance.
func reportConvergence(wf asset.Workflow, root string, probe, categories map[string]string, lifecycle string, approvedFlag bool, verdicts *verdictLedger, gateSet ...[]string) bool {
	if wf.Stop.Type == "" {
		return false
	}
	approved := humanApproved(root, wf.Stage, approvedFlag)
	signals := gatherSignals(root, wf, probe, categories, lifecycle, approved, verdicts, gateSet...)
	return reportConvergenceSignals(wf, signals)
}

func reportStageConvergence(wf asset.Workflow, root string, probe, categories map[string]string, lifecycle string, approvedFlag bool, verdicts *verdictLedger, gates []string, proposalStage, releaseStage bool) bool {
	switch {
	case proposalStage:
		approved := humanApproved(root, wf.Stage, approvedFlag)
		return reportConvergenceSignals(wf, proposalLoopSignals(root, wf, approved, verdicts))
	case releaseStage:
		return reportConvergenceSignals(wf, converge.Signals{
			HumanApproved: humanApproved(root, wf.Stage, approvedFlag),
		})
	default:
		return reportConvergence(wf, root, probe, categories, lifecycle, approvedFlag, verdicts, gates)
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
