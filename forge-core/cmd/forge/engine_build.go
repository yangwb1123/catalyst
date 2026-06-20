// engine_build.go — the orchestrator.Engine ASSEMBLY for the forge CLI: it selects
// the agent-phase executor (agentExecutor) and wires the shared Engine (buildRunEngine)
// that BOTH `forge run` (execEngine, main.go) and `forge evolve` (buildLoop, evolve.go)
// drive, so the two entry points never drift. Split out of main.go so that file stays
// under the harness's file-size budget while this keeps the wiring it owns in one place.
package main

import (
	"fmt"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/routing"
)

// agentExecutor selects the agent-phase executor. "command" builds a per-phase
// prompt and drives o.agentCmd with it (real execution when agent-cmd is `claude`;
// `echo` inspects the plumbing safely); anything else is the no-LLM DryRunExecutor.
//
// costSink is how this CLI (the ONLY layer that knows the claude JSON shape) records
// real per-phase dollar cost ATTRIBUTED to the routed model: for a claude command the
// executor's generic Observe sink is pointed at parseClaudeCostUsd, and a parsed cost —
// paired with the phase's BUDGET-ADJUSTED routed model — is forwarded to costSink (which
// the caller wires to a model-stamped trace event). The generic executor stays claude-free
// — all claude-JSON knowledge lives in the helpers below, never in orchestrator.
//
// tierOf is the ONE shared per-phase tier resolver (built in buildRunEngine): it computes
// orchestrator.PhaseTier post-filtered by routing.BudgetAdjustTier and is the SINGLE source
// the three tier consumers read, so they can never drift apart —
//   - `claude --model` here in Build (the model the run actually spawns),
//   - the cost stamp (observeFor → costSink, the model the bill is attributed to),
//   - the prompt's stated tier (buildPrompt).
//
// All three resolve the IDENTICAL adjusted tier for a given phase + spend ratio. phaseModel
// is the NAME-keyed face of that SAME tierOf (built by phaseTierByName in buildRunEngine,
// which holds the workflow): the Observe seam is handed only a phase NAME, so the cost path
// looks the Phase up and runs the same tierOf — name- and Phase-keyed lookups agree by
// construction. Both are nil-safe (a nil resolver -> un-attributed cost / un-adjusted tier).
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
func agentExecutor(o runOpts, logln func(string), costSink func(phase, model string, usd float64), tierOf func(p asset.Phase) string, phaseModel func(phase string) string, gates *gateLedger, phaseOut *phaseOutputLedger, feedsForward func(phase string) bool, verdicts *verdictLedger, findings *reviewFindingsLedger, onFailTarget func(phase string) (string, bool)) orchestrator.AgentExecutor {
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
				// override, then any near-budget down-tier); without --model, claude
				// ignores routing and uses its default. The tier comes from the shared
				// tierOf (read at THIS spawn, reflecting spend so far), the SAME value
				// the cost stamp and the prompt below use — one resolver, no drift.
				// --output-format json makes claude emit the cost-bearing envelope this
				// CLI parses (total_cost_usd) — ONLY claude gets it; echo/stubs would
				// choke on a flag they don't know and must stay plain.
				if isClaude {
					if o.agentPermission != "" {
						argv = append(argv, "--permission-mode", o.agentPermission)
					}
					argv = append(argv, "--model", tierOf(p))
					if o.agentMaxBudgetUSD != "" {
						argv = append(argv, "--max-budget-usd", o.agentMaxBudgetUSD)
					}
					argv = append(argv, "--output-format", "json")
				}
				return append(argv, "-p", buildPrompt(o.root, p, mode, tierOf, gates, phaseOut, findings))
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
// budget is the run-scoped dollar accumulator (cost.go). It is wired in THREE places so the
// run-level cap holds end to end: budget.feed WRAPS costSink so every billed phase also
// tallies the run total; budget.BudgetExhaustedFunc() supplies the engine's opaque
// BudgetExhausted puller (the PR4 hard-stop, nil when --run-budget-usd is unset → byte-for-byte
// the prior path); and budget.SpendRatio is closed over by the shared tier resolver below for
// the near-budget DOWN-TIER (PR6). The caller creates ONE budget per run (BEFORE the loop for
// evolve), so the SAME accumulator is reused across all iterations and the cap meters the whole
// run — and the spend ratio the resolver reads reflects every prior phase's spend.
//
// SHARED TIER RESOLVER (PR6, the drift-kill): phaseTierResolver builds the ONE tierOf the run
// uses everywhere a phase's model tier is needed — `claude --model`, the cost stamp, the
// prompt — so those three can never disagree (run model X / prompt says Y / cost charged Z).
// It reads budget.SpendRatio at SPAWN time (each phase, not engine-build time) so a phase that
// crosses into the near-budget band mid-run is down-tiered from that point on. phaseTierByName
// is its name-keyed face for the cost Observe seam (handed only a phase NAME), built over the
// SAME wf+resolver so the two agree by construction.
func buildRunEngine(wf asset.Workflow, o runOpts, logln func(string), costSink func(phase, model string, usd float64), runGate func(name string) gate.Result, pol mode.Policy, budget *runBudget) orchestrator.Engine {
	gates := newGateLedger()
	phaseOut := newPhaseOutputLedger()
	verdicts := newVerdictLedger()
	findings := newReviewFindingsLedger()
	tierOf := phaseTierResolver(o.mode, budget.SpendRatio, logln)
	return orchestrator.Engine{
		Exec:            agentExecutor(o, logln, budget.feed(costSink), tierOf, phaseTierByName(wf, tierOf), gates, phaseOut, feedsForwardOf(wf), verdicts, findings, onFailTargetOf(wf)),
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

// phaseTierResolver builds the ONE per-phase tier resolver (tierOf) that EVERY tier
// consumer in a run shares — `claude --model`, the cost stamp, and the prompt — so the
// three can never drift apart. For a phase it returns routing.BudgetAdjustTier applied to
// orchestrator.PhaseTier: the routed tier (Opus floor + per-phase model_tier override),
// then the near-budget down-tier (PR6) keyed on the CURRENT spend ratio.
//
// spendRatio is a PULLER (budget.SpendRatio), not a snapshot value: it is invoked afresh
// on EACH resolve, so the ratio reflects spend accumulated up to THIS spawn — a phase that
// pushes the run into the [0.80,1.00) near-budget band is down-tiered from then on, not
// frozen at the engine-build-time (zero) ratio. (PR4's hard-stop fires at ratio>=1.00
// BEFORE the spawn, so this resolver only ever sees <1.00; the >=1.00 band never reaches it.)
//
// HONESTY: when the adjustment actually lowers the tier, it logs the down-tier with the
// ratio and both tiers, naming the quality trade-off and the safety-floor exemption — a real
// down-tier is never silent. When nothing changes (in budget, or a floor agent, or already
// Haiku) it logs nothing, so an un-budgeted run's log is byte-for-byte unchanged.
func phaseTierResolver(mode string, spendRatio func() float64, logln func(string)) func(p asset.Phase) string {
	return func(p asset.Phase) string {
		base := orchestrator.PhaseTier(p, mode)
		ratio := spendRatio()
		adj := routing.BudgetAdjustTier(base, p.Agent, ratio)
		if adj != base && logln != nil {
			logln(fmt.Sprintf("phase %s: near budget (spend-ratio %.2f) — downtiering %s→%s to extend runway (cheaper model, lower quality; safety-floor agents exempt)", p.Name, ratio, base, adj))
		}
		return adj
	}
}

// phaseTierByName is the NAME-keyed face of a phaseTierResolver tierOf, for the cost Observe
// seam (which is handed only a phase NAME, never the Phase — exactly as feedsForwardOf /
// onFailTargetOf are). It looks the Phase up in wf and runs the SAME tierOf, so the cost
// stamp resolves byte-for-byte the tier `--model` got. An unknown name yields "" (omitempty
// drops it downstream), matching the prior phaseModelResolver's miss behavior.
//
// SAME-RATIO GUARANTEE (why the stamp matches `--model`, not a ratio that moved):
// observeFor resolves this stamp as an ARGUMENT to the feed-wrapped cost sink —
// costSink(phase, phaseModelOf(...), usd) — and Go evaluates arguments left-to-right
// BEFORE the call, so the stamp's SpendRatio() read happens BEFORE feed adds THIS phase's
// usd to spent. Phases are serial (no other phase bills in between), so the ratio the stamp
// sees is identical to the one Build saw at this phase's spawn: requested == billed ==
// stamped holds even when the phase's own cost would cross the 0.80 band. The down-tier log
// may print a second time on this path; that is cosmetic — the tiers are provably equal
// (the drift-guard test pins it).
func phaseTierByName(wf asset.Workflow, tierOf func(p asset.Phase) string) func(name string) string {
	return func(name string) string {
		for _, p := range wf.Phases {
			if p.Name == name {
				return tierOf(p)
			}
		}
		return ""
	}
}
