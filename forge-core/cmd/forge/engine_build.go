// engine_build.go — the orchestrator.Engine ASSEMBLY for the forge CLI: it selects
// the agent-phase executor (agentExecutor) and wires the shared Engine (buildRunEngine)
// that BOTH `forge run` (execEngine, main.go) and `forge evolve` (buildLoop, evolve.go)
// drive, so the two entry points never drift. Split out of main.go so that file stays
// under the harness's file-size budget while this keeps the wiring it owns in one place.
package main

import (
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
)

// agentExecutor selects the agent-phase executor. "command" builds a per-phase
// prompt and drives o.agentCmd with it (real execution when agent-cmd is `claude`;
// `echo` inspects the plumbing safely); anything else is the no-LLM DryRunExecutor.
//
// costSink is how this CLI (the ONLY layer that knows the claude JSON shape) records
// real per-phase dollar cost ATTRIBUTED to the routed model: for a claude command the
// executor's generic Observe sink is pointed at parseClaudeCostUsd, and a parsed cost —
// paired with the phase's routed model (phaseModel, the SAME orchestrator.PhaseTier
// Build hands to `claude --model`) — is forwarded to costSink (which the caller wires to a
// model-stamped trace event). The generic executor stays claude-free — all claude-JSON
// knowledge lives in the helpers below, never in orchestrator. phaseModel is an injected
// lookup (not read off the Phase) because Observe receives only a phase NAME, exactly as
// feedsForward/onFailTarget are; it is nil-safe (a nil resolver -> un-attributed cost).
//
// gates carries this run's gate verdicts into each phase's prompt so a downstream agent
// is told what the gates found, not made to re-run them. phaseOut carries the output of
// any prior feeds_forward phase (the planner's task split) into later prompts, and
// feedsForward reports whether a just-finished phase's output should be remembered there
// — it is injected (not derived from asset.Phase) because the executor's generic Observe
// sink receives only (phase name, output), never the Phase, so the FeedsForward lookup
// must come from the caller, which holds the workflow. verdicts records each reviewer's
// parsed VERDICT (the Observe sink writes it; the engine's AgentVerdict reads it back to
// drive a directed loop-back); findings stashes a REQUEST_CHANGES review's notes for the
// loop-back target (the implementer) and is the ONLY of these injected back into a prompt
// by name; onFailTarget is the data-driven (phase -> loop-back target) lookup that routes
// those findings. All are nil-safe (see prompt_context.go); the generic executor stays
// oblivious to every one of them.
func agentExecutor(o runOpts, logln func(string), costSink func(phase, model string, usd float64), phaseModel func(phase string) string, gates *gateLedger, phaseOut *phaseOutputLedger, feedsForward func(phase string) bool, verdicts *verdictLedger, findings *reviewFindingsLedger, onFailTarget func(phase string) (string, bool)) orchestrator.AgentExecutor {
	if o.executor == "command" {
		isClaude := strings.Contains(o.agentCmd, "claude")
		ex := orchestrator.CommandExecutor{
			Build: func(p asset.Phase, mode string) []string {
				argv := []string{o.agentCmd}
				// claude print mode needs flags echo/stubs don't understand, so gate
				// on a claude-family command: --permission-mode to USE tools (write
				// files) headlessly — without it the agent only DESCRIBES edits it
				// can't apply — and --model so a real run honors ForgeOS's ROUTED tier
				// (the opus floor for reviewer/architect/cto + any per-phase model_tier
				// override); without --model, claude ignores routing and uses its default.
				// --output-format json makes claude emit the cost-bearing envelope this
				// CLI parses (total_cost_usd) — ONLY claude gets it; echo/stubs would
				// choke on a flag they don't know and must stay plain.
				if isClaude {
					if o.agentPermission != "" {
						argv = append(argv, "--permission-mode", o.agentPermission)
					}
					argv = append(argv, "--model", orchestrator.PhaseTier(p, mode))
					if o.agentMaxBudgetUSD != "" {
						argv = append(argv, "--max-budget-usd", o.agentMaxBudgetUSD)
					}
					argv = append(argv, "--output-format", "json")
				}
				return append(argv, "-p", buildPrompt(o.root, p, mode, gates, phaseOut, findings))
			},
			Dir:            o.root,
			Timeout:        o.timeout,
			MaxDepth:       o.maxAgentDepth,
			MaxOutputBytes: o.maxOutputBytes,
			Log:            logln,
		}
		ex.Observe = observeFor(isClaude, costSink, phaseModel, phaseOut, feedsForward, verdicts, findings, onFailTarget)
		// Only claude emits the cost JSON, so only claude gets the result-unwrapping log
		// renderer; echo/stubs stay nil -> the generic executor logs raw output verbatim.
		if isClaude {
			ex.RenderLog = unwrapClaudeResult
			// Only claude returns the 529 overloaded_error envelope, so only the claude path
			// gets the overload recognizer; echo/stubs stay nil -> a failing stub is never
			// mistaken for a transient overload and keeps its terminal KindFailed (back-compat).
			ex.ClassifyOverload = classifyClaudeOverload
		}
		return ex
	}
	return orchestrator.DryRunExecutor{Log: logln}
}

// buildRunEngine assembles the orchestrator.Engine shared by `forge run` (execEngine)
// and `forge evolve` (buildLoop): the SAME four prompt/feedback ledgers wired to the same
// seams, so the two entry points never drift. The FOUR ledgers, all per-run/iteration and
// all nil-safe (prompt_context.go):
//   - gates (gateLedger): OnGateResult writes each gate's objective verdict; the prompt reads it;
//   - phaseOut (phaseOutputLedger): the Observe sink writes a feeds_forward phase's output (the
//     planner's task split); a later prompt reads it;
//   - verdicts (verdictLedger): the Observe sink writes each reviewer's parsed VERDICT;
//     Engine.AgentVerdict reads it back to drive the directed reviewer loop-back;
//   - findings (reviewFindingsLedger): on a REQUEST_CHANGES, the Observe sink stashes the
//     review notes for the loop-back target (the implementer), read into THAT prompt.
//
// feedsForwardOf/onFailTargetOf close over wf so the Observe seam (handed only a phase
// NAME) can look up FeedsForward and the on_fail loop-back target. runGate is injected
// because run uses harnessRunner(probe) while evolve uses a per-iteration refreshing
// probe; pol/cost/log are likewise the caller's. Only the claude executor path activates
// the verdict/findings sinks (dry/echo leave AgentVerdict effectively inert).
//
// budget is the run-scoped dollar accumulator (cost.go). It is wired in two symmetric
// places so the run-level cap holds end to end: budget.feed WRAPS costSink so every billed
// phase also tallies the run total, and budget.BudgetExhaustedFunc() supplies the engine's
// opaque BudgetExhausted puller (nil when --run-budget-usd is unset → byte-for-byte the
// prior path). The caller creates ONE budget per run (BEFORE the loop for evolve), so the
// SAME accumulator is reused across all iterations and the cap meters the whole run.
func buildRunEngine(wf asset.Workflow, o runOpts, logln func(string), costSink func(phase, model string, usd float64), runGate func(name string) gate.Result, pol mode.Policy, budget *runBudget) orchestrator.Engine {
	gates := newGateLedger()
	phaseOut := newPhaseOutputLedger()
	verdicts := newVerdictLedger()
	findings := newReviewFindingsLedger()
	return orchestrator.Engine{
		Exec:            agentExecutor(o, logln, budget.feed(costSink), phaseModelResolver(wf, o.mode), gates, phaseOut, feedsForwardOf(wf), verdicts, findings, onFailTargetOf(wf)),
		RunGate:         runGate,
		Log:             logln,
		OnGateResult:    gates.record,
		AgentVerdict:    verdicts.get,
		BudgetExhausted: budget.BudgetExhaustedFunc(),
		MaxRetries:      o.maxRetries,
		MaxLoopBack:     maxLoopBack,
		MaxAgentCalls:   o.maxAgentCalls,
		ModePolicy:      pol,
	}
}
