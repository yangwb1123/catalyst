// engine_build.go — the orchestrator.Engine ASSEMBLY for the forge CLI: it selects
// the agent-phase executor (agentExecutor) and wires the shared Engine (buildRunEngine)
// that BOTH `forge run` (execEngine, main.go) and `forge evolve` (buildLoop, evolve.go)
// drive, so the two entry points never drift. Split out of main.go to stay under the
// harness's file-size budget while keeping the wiring it owns in one place.
package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/attribution"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/prompt"
	"forgeos/forge-core/internal/risk"
	"forgeos/forge-core/internal/routing"
	"forgeos/forge-core/internal/trace"
)

// agentExecutor selects the agent-phase executor. "command" builds a per-phase
// prompt and drives o.agentCmd with it (real execution when agent-cmd is `claude`;
// `echo` inspects the plumbing safely); anything else is the no-LLM DryRunExecutor.
//
// costSink records real per-phase dollar cost AND measured latency attributed to the
// routed model: for claude the generic Observe sink is pointed at parseClaudeCostUsd,
// and a parsed cost — paired with the BUDGET-ADJUSTED routed model AND the executor's
// measured latency — is forwarded to costSink. The generic executor stays claude-free.
//
// tierOf is the ONE shared per-phase tier resolver (buildRunEngine): the SINGLE source
// `claude --model`, the cost stamp, and the prompt's stated tier all read, so the three
// never drift apart. phaseModel is its NAME-keyed face (phaseTierByName) for the cost
// Observe seam, which is handed only a phase NAME. Both nil-safe.
//
// gates carries this run's gate verdicts into each phase's prompt. phaseOut carries a
// prior feeds_forward phase's output into later prompts; feedsForward (injected, since
// Observe gets only a phase name) reports whether to remember it. verdicts records
// each reviewer's parsed VERDICT for Engine.AgentVerdict's loop-back; findings stashes
// a REQUEST_CHANGES review's notes for the loop-back target; onFailTarget routes them.
// All nil-safe (prompt_context.go); the generic executor stays oblivious to all of them.
func agentExecutor(o runOpts, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), tierOf func(p asset.Phase) string, phaseModel func(phase string) string, ctxCache *prompt.ContextCache, gates *gateLedger, phaseOut *phaseOutputLedger, feedsForward func(phase string) bool, verdicts *verdictLedger, findings *reviewFindingsLedger, onFailTarget func(phase string) (string, bool)) orchestrator.AgentExecutor {
	if o.executor == "command" {
		isClaude := strings.Contains(o.agentCmd, "claude")
		ex := orchestrator.CommandExecutor{
			Build: func(p asset.Phase, mode string) []string {
				narrateReadonly(logln, p)
				argv := claudeArgv(o, isClaude, tierOf(p), p)
				return append(argv, "-p", requiresToolsGuard(p, true, isClaude, o.agentAllowedTools, logln, buildPrompt(o.root, p, mode, tierOf, ctxCache, gates, phaseOut, findings)))
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

// claudeArgv builds the leading argv (everything before `-p <prompt>`) for an agent
// command: [agentCmd] for a non-claude command (echo/stubs, back-compat), else the
// print-mode flags echo/stubs don't understand:
//   - --permission-mode: USE tools headlessly (else `claude -p` only DESCRIBES edits);
//   - --disallowedTools "Edit Write" for a p.Readonly phase (readonlyToolScope): real
//     path-scoped write enforcement (narrateReadonly only narrates it);
//   - --allowedTools: the operator's self-verification whitelist (o.agentAllowedTools,
//     e.g. `node --test`/`gate.mjs`; NEVER whitelist `forge` — fork-bomb past
//     FORGE_AGENT_DEPTH) MERGED (mergeToolList) with any readonly Edit()/Write() re-open
//     patterns into ONE flag occurrence — `claude --help` (local, zero API spend)
//     confirms each of --allowedTools/--disallowedTools takes ONE comma-or-space-
//     separated <tools...> list; a second occurrence's merge-vs-override semantics were
//     never exercised here. Omitted entirely when the merged value is empty (opt-out);
//   - --model <tier>: the shared routed tier — the SAME value the cost stamp and
//     prompt use, no drift;
//   - --max-budget-usd: the per-call dollar ceiling (omitted when unset);
//   - --output-format json: the cost-bearing envelope this CLI parses (total_cost_usd).
func claudeArgv(o runOpts, isClaude bool, tier string, p asset.Phase) []string {
	argv := []string{o.agentCmd}
	if !isClaude {
		return argv
	}
	if o.agentPermission != "" {
		argv = append(argv, "--permission-mode", o.agentPermission)
	}
	deny, allowExtra := readonlyToolScope(p)
	if deny != "" {
		argv = append(argv, "--disallowedTools", deny)
	}
	if allowed := mergeToolList(o.agentAllowedTools, allowExtra); allowed != "" {
		argv = append(argv, "--allowedTools", allowed)
	}
	argv = append(argv, "--model", tier)
	if o.agentMaxBudgetUSD != "" {
		argv = append(argv, "--max-budget-usd", o.agentMaxBudgetUSD)
	}
	return append(argv, "--output-format", "json")
}

// mergeToolList joins the operator whitelist (base) and a readonly phase's Edit/Write
// re-open patterns (extra) into ONE space-separated --allowedTools value (see
// claudeArgv). Either half may be empty; "" only when both are.
func mergeToolList(base, extra string) string {
	switch {
	case base == "":
		return extra
	case extra == "":
		return base
	default:
		return base + " " + extra
	}
}

// readonlyAgentWriteScope maps an agent to the claude permission-specifier PATTERN(s)
// (gitignore-style, project-root-relative — code.claude.com/docs/en/permissions.md)
// its OWN card (.agent/agents/<agent>.md "硬边界", "不在 X 之外写文件") documents as
// its sole write target. Keyed by AGENT, not stage: cto's boundary applies whether it
// runs as design.yml's proposal-generator OR review.yml's executive-review (cto.md's
// "Review 阶段" section is ADDITIVE, never widens the boundary above it). An agent
// ABSENT here has NO documented target — reviewer/qa ("不写...代码文件"), explorer
// ("零写入"), non-LLM `harness` — and stays FULLY denied when readonly, matching its
// card (readonlyToolScope's zero-pattern branch). planner is a FILE not a dir:
// planner.md names `.agent/CURRENT_SPRINT.md` itself — build.yml's `emits:
// task-plan.md` there is a declared-artifact LABEL (read back by emitsContext as a
// bare repo-root filename), not the real write target; the card wins per this task's
// brief ("use the convention the project already documents").
var readonlyAgentWriteScope = map[string][]string{
	"product-manager":      {"/docs/discovery/**"},
	"researcher":           {"/docs/discovery/**"},
	"architect":            {"/docs/design/**"}, // + docs/adr/** via WritesADR below
	"cto":                  {"/docs/design/**"}, // Design AND Review-stage responsibilities
	"planner":              {"/.agent/CURRENT_SPRINT.md"},
	"security-engineer":    {"/docs/review/**"},
	"distributed-engineer": {"/docs/review/**"},
	"performance-engineer": {"/docs/review/**"},
}

// readonlyToolScope returns the claude --disallowedTools value and any --allowedTools
// re-open patterns for a phase — REAL enforcement of asset.Phase.Readonly (narrateReadonly
// only narrates it). Non-readonly -> ("", ""): claudeArgv adds nothing, byte-identical.
// A readonly phase ALWAYS gets "Edit Write" denied, THEN — if its agent has a
// documented target (readonlyAgentWriteScope), plus docs/adr/** when THIS phase
// declares writes_adr (WritesADR.Target decoded off the asset, never hardcoded) —
// Edit()/Write() reopen for that pattern. No documented target -> denial only: fully
// read-only, matching its card — a correct zero-write phase, not a gap.
func readonlyToolScope(p asset.Phase) (deny, allow string) {
	if !p.Readonly {
		return "", ""
	}
	patterns := append([]string(nil), readonlyAgentWriteScope[p.Agent]...)
	if p.WritesADR != nil && p.WritesADR.Target != "" {
		patterns = append(patterns, "/"+strings.Trim(p.WritesADR.Target, "/")+"/**")
	}
	if len(patterns) == 0 {
		return "Edit Write", ""
	}
	specs := make([]string, 0, len(patterns)*2)
	for _, pat := range patterns {
		specs = append(specs, "Edit("+pat+")", "Write("+pat+")")
	}
	return "Edit Write", strings.Join(specs, " ")
}

// narrateReadonly logs a decision-narration line every time a phase declaring
// readonly: true is spawned under --executor=command. PURELY observational — the
// actual restriction is claudeArgv's readonlyToolScope (--disallowedTools "Edit
// Write" + a path-scoped --allowedTools re-open), wired right after this call.
//
// HONESTY: readonly is NOT "writes nothing" — several readonly phases still declare
// `emits:` they must write. readonlyToolScope re-opens Edit/Write for exactly the
// dir/file each phase's AGENT CARD documents — never a blanket re-open. ENFORCED BY
// UNIT TEST AGAINST THE DOCUMENTED CLAUDE CLI CONTRACT ONLY (path-scoped Edit()/
// Write() specifiers, code.claude.com/docs/en/permissions.md; confirmed via a local
// `claude --help`, zero API spend) — NOT live-verified against a running claude
// process; no budget was authorized for that.
//
// logln nil (a quiet caller) is a no-op; a non-readonly phase is a no-op.
func narrateReadonly(logln func(string), p asset.Phase) {
	if !p.Readonly || logln == nil {
		return
	}
	emits := "none declared"
	if len(p.Emits) > 0 {
		emits = strings.Join(p.Emits, ", ")
	}
	logln(fmt.Sprintf("phase %s: readonly=true (analysis-only — must not modify existing source/product code; MAY still write its declared emits: %s) — ENFORCED: Edit/Write denied via --disallowedTools except for this agent's documented write target, if any (see readonlyToolScope)", p.Name, emits))
}

// buildRunEngine assembles the orchestrator.Engine shared by `forge run` (execEngine)
// and `forge evolve` (buildLoop): the SAME four prompt/feedback ledgers wired to the same
// seams, so the two entry points never drift. The FOUR ledgers, all per-run/iteration and
// all nil-safe (prompt_context.go): gates (OnGateResult writes each gate's verdict, the
// prompt reads it); phaseOut (the Observe sink writes a feeds_forward phase's output, a
// later prompt reads it); verdicts (the Observe sink writes each reviewer's parsed VERDICT,
// AgentVerdict reads it back for the directed loop-back); findings (on a REQUEST_CHANGES,
// the Observe sink stashes the review notes for the loop-back target). feedsForwardOf/
// onFailTargetOf close over wf so the Observe seam (handed only a phase NAME) can look
// them up. runGate is injected (run uses gate.HarnessRunner, evolve a refreshing probe).
//
// budget (cost.go) is wired in THREE places: budget.feed WRAPS costSink so every billed
// phase tallies the run total; BudgetExhaustedFunc() supplies the engine's hard-stop (nil
// when unset); budget.SpendRatio feeds the shared tier resolver's near-budget down-tier.
// ONE budget per run so the cap meters the whole run.
//
// SHARED TIER RESOLVER (the drift-kill): phaseTierResolver builds the ONE tierOf the run
// uses everywhere a tier is needed — `claude --model`, the cost stamp, the prompt — so the
// three can never disagree; it reads budget.SpendRatio at SPAWN time so a phase crossing
// the near-budget band mid-run is down-tiered from that point on. LEARNING-LOOP READ-BACK:
// scorecards (scorecard_wind.go) are loaded ONCE here and fed to the resolver, driving
// routing.HistoryTiebreak for OBSERVABILITY only — FAIL-LOUD-AND-CONTINUE, a malformed
// scorecards.json WARNs and continues on empty cards.
//
// Returns the assembled Engine plus the verdict/findings ledgers for callers to thread
// rework+trajectory signals into wind-down/Reflect without re-building.
func buildRunEngine(wf asset.Workflow, o runOpts, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), runGate func(name string) gate.Result, pol mode.Policy, budget *runBudget, autoRisk string, autoRiskReasons []string) (orchestrator.Engine, *verdictLedger, *reviewFindingsLedger) {
	gates := newGateLedger()
	phaseOut := newPhaseOutputLedger()
	verdicts := newVerdictLedger()
	findings := newReviewFindingsLedger()
	// Per-run invariant-context memo (prompt-cache, ROADMAP direction five). Created HERE,
	// alongside the four ledgers and with the SAME per-run lifetime, so it is reused across
	// every phase of one run — and, for evolve, across iterations of the SAME Engine (the
	// invariant lanes are stable for the whole run; the ROADMAP it pointedly does NOT cache
	// is re-read each phase, so an implementer's mid-run [x] is seen next iteration). It is a
	// FUNCTION-LOCAL value, never a package global: a singleton would let one run's memoized
	// ADR/AGENTS snapshot escape into a later unrelated run. nil-safe downstream (buildPrompt
	// falls back to prompt.Gather on nil), so this is purely additive — the prompt bytes are
	// unchanged. HONESTY: saves local readdir/readFile, NOT claude tokens (see cache.go).
	ctxCache := prompt.NewContextCache()
	cards, err := routing.LoadScorecards(scorecardPath(o.root))
	if err != nil {
		// Malformed scorecards.json: fail loud, continue empty. Honesty over convenience —
		// the WARNING names the broken Eval producer; the run proceeds on no history (the
		// cold-start path), since the read-back is observability, not run correctness.
		logln(fmt.Sprintf("forge: WARNING scorecards unreadable (%v) — continuing with no history (routing unaffected; learning-loop read-back skipped)", err))
		cards = nil
	}
	tierOf := phaseTierResolver(o.mode, budget.SpendRatio, cards, logln, autoRisk, autoRiskReasons)
	return orchestrator.Engine{
		Exec:            agentExecutor(o, logln, budget.feed(costSink), tierOf, phaseTierByName(wf, tierOf), ctxCache, gates, phaseOut, feedsForwardOf(wf), verdicts, findings, onFailTargetOf(wf)),
		RunGate:         runGate,
		Log:             logln,
		OnGateResult:    gates.record,
		AgentVerdict:    verdicts.get,
		BudgetExhausted: budget.BudgetExhaustedFunc(),
		MaxRetries:      o.maxRetries,
		MaxLoopBack:     maxLoopBack,
		MaxAgentCalls:   o.maxAgentCalls,
		ModePolicy:      pol,
	}, verdicts, findings
}

// phaseTierResolver builds the ONE per-phase tier resolver (tierOf) that EVERY tier
// consumer in a run shares — `claude --model`, the cost stamp, and the prompt — so the
// three can never drift apart. For a phase it applies: (1) orchestrator.PhaseTier (Opus
// floor + per-phase model_tier override); (2) riskAdjustedTier (auto-risk escalation,
// critical -> Opus, high -> Sonnet); (3) routing.BudgetAdjustTier (near-budget
// down-tier, clamped not to undo risk escalation). autoRisk is the auto-derived risk
// level from git diff (empty = no changes/no git); autoRiskReasons are the human-
// readable path-matching hits, logged when escalation fires.
func phaseTierResolver(mode string, spendRatio func() float64, cards []routing.Scorecard, logln func(string), autoRisk string, autoRiskReasons []string) func(p asset.Phase) string {
	return func(p asset.Phase) string {
		base := orchestrator.PhaseTier(p, mode)

		// Step 2: risk-based escalation (before budget, so risk can't be down-tiered).
		tier := riskAdjustedTier(base, autoRisk)
		if tier != base && logln != nil {
			reasons := ""
			if len(autoRiskReasons) > 0 {
				reasons = " (" + strings.Join(autoRiskReasons, "; ") + ")"
			}
			logln(fmt.Sprintf("phase %s: risk auto-detected=%s — escalating %s→%s%s",
				p.Name, autoRisk, base, tier, reasons))
		}

		// Step 3: near-budget down-tier (can only lower risk-escalated tier when
		// the run is very near its cap — risk escalation is raise-only, but if budget
		// pressure forces a Haiku below Opus for a critical change, the hard stop fires
		// before the spawn anyway).
		ratio := spendRatio()
		adj := routing.BudgetAdjustTier(tier, p.Agent, ratio)
		if adj != tier && logln != nil {
			logln(fmt.Sprintf("phase %s: near budget (spend-ratio %.2f) — downtiering %s→%s to extend runway (cheaper model, lower quality; safety-floor agents exempt)", p.Name, ratio, tier, adj))
		}
		picked := logPhaseHistory(p, adj, cards, logln)
		return picked
	}
}

// logPhaseHistory queries HistoryTiebreak for this phase and returns the selected
// model, which phaseTierResolver uses as the actual routing decision for
// non-safety-floor agents (v1.5 upgrade):
//   - non-floor agents: candidates = routing.CandidatesForTier(adj) = [adj,
//     ...cheaper]. HistoryTiebreak selects the highest-quality qualifying candidate;
//     a cheaper model wins only with sufficient history AND better quality_score.
//     Cold start / thin data -> falls back to adj (candidates[0]);
//   - safety-floor agents (reviewer/architect/cto): candidates stay [adj] — Opus can
//     never be overridden by scorecard data;
//   - an UNMAPPED agent (harness/gate phase) is SKIPPED, adj returned unchanged.
//
// logln nil -> no log output, picked still returned. cards empty (cold start) ->
// falls back to adj with an honest "no scorecard -> tier_default" reason.
func logPhaseHistory(p asset.Phase, adj string, cards []routing.Scorecard, logln func(string)) string {
	taskType, ok := attribution.TaskTypeForAgent(p.Agent)
	if !ok {
		return adj // unmapped (harness/gate) phase: no task_type, return adj unchanged
	}
	candidates := routing.CandidatesForTier(adj)
	suffix := "[v1.5 multi-candidate: history may pick cheaper model]"
	if routing.IsOpusFloorAgent(p.Agent) {
		candidates = []string{adj} // safety-floor agents: single-candidate passthrough
		suffix = "[safety-floor: opus locked, history observability only]"
	}
	picked, reason := routing.HistoryTiebreak(candidates, taskType, cards, historyMinSamples)
	if logln != nil {
		logln(fmt.Sprintf("phase %s: tier=%s (task=%s) — history: %s %s", p.Name, adj, taskType, reason, suffix))
	}
	return picked
}

// phaseTierByName is the NAME-keyed face of a phaseTierResolver tierOf, for the cost Observe
// seam (handed only a phase NAME, never the Phase). It looks the Phase up in wf and runs the
// SAME tierOf, so the cost stamp resolves byte-for-byte the tier `--model` got. Unknown name
// -> "" (omitempty drops it downstream).
//
// SAME-RATIO GUARANTEE: observeFor resolves this stamp as an ARGUMENT to the feed-wrapped
// cost sink, and Go evaluates arguments left-to-right BEFORE the call, so the stamp's
// SpendRatio() read happens BEFORE feed adds THIS phase's usd to spent — requested == billed
// == stamped holds even when this phase's own cost crosses the 0.80 band (drift-guard tested).
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

// ── Risk auto-extraction (wiring for run/evolve) ──────────────────────────
//
// resolveAutoRisk derives the risk level of the pending change set from the git
// working tree, using the SAME path-substring heuristics as `forge route --from-git`,
// so every `forge run`/`forge evolve` benefits, not just the standalone CLI.
//
// Honesty (mirrors risk_diff.go): path-substring matching is a COARSE heuristic (WILL
// miss/over-match); ProdTraffic is NEVER auto-set; no git repo/changes -> empty result.
func resolveAutoRisk(root string) (level string, reasons []string) {
	// Reuse gitChangedPaths from route.go (same package) — it resolves root
	// through gate.RepoRoot, which is a no-op on an already-resolved path.
	paths := gitChangedPaths(root)
	if len(paths) == 0 {
		return "", nil
	}
	sig, reasons := risk.FromChangedPaths(paths)
	level, _ = risk.Classify(sig)
	return level, reasons
}

// logAutoRisk prints the auto-detected risk level and its path-matching reasons
// to logln, if a non-empty risk level was detected. Returns the risk reasons
// slice (callers that need it for the tier resolver can capture it).
func logAutoRisk(logln func(string), prefix, autoRisk string, autoRiskReasons []string) {
	if autoRisk == "" {
		return
	}
	rs := ""
	if len(autoRiskReasons) > 0 {
		rs = " (" + strings.Join(autoRiskReasons, "; ") + ")"
	}
	logln(fmt.Sprintf("%s: auto-detected risk=%s%s", prefix, autoRisk, rs))
}

// riskAdjustedTier applies risk-based tier escalation on top of the base routed tier,
// BEFORE the budget down-tier so critical/high-risk work cannot be down-tiered by
// budget pressure. RAISE-ONLY: never lowers. Critical (irreversible + payment/blast-
// radius/prod) -> force Opus; High (payment/auth/secrets) -> raise to at least Sonnet;
// Medium/Low/unset -> no change.
func riskAdjustedTier(base, riskLevel string) string {
	switch riskLevel {
	case risk.Critical:
		return routing.Higher(base, routing.Opus)
	case risk.High:
		return routing.Higher(base, routing.Sonnet)
	default:
		return base
	}
}

// ── execEngine — `forge run`'s engine wiring + drive (moved from main.go so that
// file stays under the harness's file-size budget; engine_build.go already owns
// every other piece of Engine assembly, so this keeps it in one place) ─────────

// openRunResources opens the trace/doctor/budget resources execEngine needs before
// building the engine — extracted purely to keep execEngine under the harness's
// per-function line budget, no behavior change. Opens the run's trace (append, same
// .forge/trace.jsonl evolve uses, git-ignored, so real claude cost is never billed
// unseen), runs the doctor's pre-run diagnostics into it, then opens the run-level
// budget — fail-closed on either step. The returned closeTrace is nil only alongside
// a non-nil err.
func openRunResources(root, runBudgetUSD string, logln func(string)) (*trace.Tracer, func(), *runBudget, error) {
	tracer, closeTrace, err := openTracer(root)
	if err != nil {
		return nil, nil, nil, err
	}
	quickDoctorCheck(root, tracer, logln)
	budget, err := newRunBudget(runBudgetUSD)
	if err != nil {
		closeTrace()
		return nil, nil, nil, err
	}
	return tracer, closeTrace, budget, nil
}

// execEngine wires the real harness gates + the selected agent executor and
// runs the workflow, returning 0 on a clean run and 1 on the first failure.
// ctx carries cancellation so the engine can abort cleanly on SIGINT.
//
// Honesty: acceptance is probed ONCE per run (gate.ProbeAll), and that single
// map backs BOTH the per-gate verdicts (harnessRunner) and convergence
// (gatherSignals) — never double-spawned, never inconsistent within a run. An
// N/A gate does NOT fail the run (it completes, exit 0); only a real FAIL does.
func execEngine(ctx context.Context, wf asset.Workflow, o runOpts) int {
	logln := func(s string) { fmt.Println(s) }
	probe, categories := probeStatuses(o.root)
	lifecycle := resolveLifecycle(o)
	pol := mode.Effective(o.mode, lifecycle)
	autoRisk, autoRiskReasons := resolveAutoRisk(o.root) // auto-detect risk for tier escalation
	logAutoRisk(logln, "forge run", autoRisk, autoRiskReasons)
	tracer, closeTrace, budget, err := openRunResources(o.root, o.runBudgetUSD, logln)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	defer closeTrace()
	eng, verdicts, _ := buildRunEngine(wf, o, logln, costEmitter(tracer, logln), gate.HarnessRunner(o.root, probe), pol, budget, autoRisk, autoRiskReasons)
	wireGateTrace(&eng, tracer, logln)
	// Learning-loop wind-down: attribute this run's REAL billed cost into the scorecards
	// regardless of outcome (a REJECTED build is the most useful quality sample), DEFERRED
	// after `defer closeTrace()` so it runs BEFORE it (LIFO) — the trace it reads is still
	// open. iterations=1: a single `forge run` is one execution; verdicts.wasReworked()
	// carries the real reviewer-bounce signal into avg_iterations / rework_rate.
	defer func() { windDownScorecards(wf, o, logln, 1, verdicts.wasReworked()) }()
	logRunBanner(wf, o, lifecycle, pol)
	// human_gate REJECTION loop-back (design.yml's on_rejected): a filed
	// .forge/<stage>.rejected marker redirects this run to target_phase instead
	// of phase 0 — full contract in resolveRejectionStartPhase (gates.go). 0 is
	// byte-for-byte the prior always-phase-0 behavior.
	startPhase := resolveRejectionStartPhase(wf, o.root, logln)
	if err := runWorkflow(ctx, eng, wf, o, logln, startPhase); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	fmt.Println("forge run: workflow completed")
	reportConvergence(wf, o.root, probe, categories, lifecycle, o.approved, verdicts)
	return 0
}

// wireGateTrace composes the engine's existing gate-ledger recording (eng.OnGateResult)
// with trace emission so every gate result is observable in trace.jsonl. A nil tracer
// (never actually returned by openTracer, but defensive) is a no-op.
func wireGateTrace(eng *orchestrator.Engine, tracer *trace.Tracer, logln func(string)) {
	if tracer == nil {
		return
	}
	origOnGate := eng.OnGateResult
	eng.OnGateResult = func(name, status string) {
		if origOnGate != nil {
			origOnGate(name, status)
		}
		emitTrace(tracer, trace.GateEvent(name, status, ""), logln)
	}
}

// logRunBanner prints the "decisions are narrated even in dry-run" summary line for
// `forge run`, naming every workflow_depth dimension the central knob resolved
// (discover/design/review/adr) alongside the run's mode/lifecycle/executor/gates.
func logRunBanner(wf asset.Workflow, o runOpts, lifecycle string, pol mode.Policy) {
	fmt.Printf("forge run: stage=%s mode=%s lifecycle=%s executor=%s gates=%v reviewer=%v discover=%s design=%s adr=%v review=%s (%d phases)\n",
		wf.Stage, o.mode, lifecycle, o.executor, pol.Gates, pol.Reviewer,
		pol.DiscoverDepth, pol.DesignDepth, pol.ADR, pol.ReviewDepth, len(wf.Phases))
}
