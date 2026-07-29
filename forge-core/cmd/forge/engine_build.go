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
	"sync"
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
// costSink records real per-phase dollar cost AND measured latency attributed to the
// routed model: for claude the generic Observe sink is pointed at parseClaudeCostUsd,
// and a parsed cost — paired with the BUDGET-ADJUSTED routed model AND the executor's
// measured latency — is forwarded to costSink. The generic executor stays claude-free.
// tierOf is the ONE shared per-phase tier resolver: the SINGLE source `claude --model`,
// the cost stamp, and the prompt's stated tier all read, so the three never drift apart.
// phaseModel is its NAME-keyed face for the cost Observe seam. Both nil-safe.
// gates carries this run's gate verdicts into each phase's prompt. phaseOut carries a
// prior feeds_forward phase's output into later prompts; feedsForward (injected, since
// Observe gets only a phase name) reports whether to remember it. verdicts records each
// reviewer's parsed VERDICT for Engine.AgentVerdict's loop-back; findings stashes a
// REQUEST_CHANGES review's notes for the loop-back target; onFailTarget routes them;
// priorEmits (priorEmitsOf) resolves earlier phases' emits: content into the prompt. All
// nil-safe; the generic executor stays oblivious to all of them.

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
func buildRunEngine(wf asset.Workflow, o runOpts, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), runGate func(name string) gate.Result, pol mode.Policy, budget *runBudget, autoRisk string, autoRiskReasons []string, runIDs ...string) (orchestrator.Engine, *verdictLedger, *reviewFindingsLedger) {
	o.workflowStage = wf.Stage
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
	o.evolveProposalOnly = wf.Stage == "evolve" && (pol.BuildHalted() || pol.EvolveProposalOnly())
	cards := loadRunScorecards(wf, o.root, logln)
	tierOf := taskAwareTierResolver(
		phaseTierResolver(o.mode, budget.SpendRatio, cards, logln, autoRisk, autoRiskReasons),
		phaseOut, logln)
	provenance := newArtifactProvenance(o.root, wf.Stage, firstRunID(runIDs), o.releaseAgentSHA256)
	return orchestrator.Engine{
		Exec: agentExecutor(o, logln, budget.feed(costSink), tierOf, phaseTierByName(wf, tierOf),
			ctxCache, gates, phaseOut, feedsForwardOf(wf), verdicts, findings,
			onFailTargetOf(wf), priorEmitsOf(wf), executorHooks{
				ValidateOutput:     phaseOutputContract(o.root, wf, provenance),
				ValidateRawOutput:  releaseRawOutputContract(wf),
				OnBuild:            provenance.recordBuild,
				ModelFor:           provenance.modelFor,
				VerdictContractFor: verdictContractOf(wf),
			}),
		RunGate:      runGate,
		Log:          logln,
		OnGateResult: gates.record,
		AgentVerdict: loopbackVerdict(verdicts),
		RequireAgentVerdict: func(p asset.Phase) bool {
			return releaseValidationPhase(wf.Stage, p)
		},
		OnRequiredVerdictApproved: releaseVerdictCommit(wf.Stage, provenance),
		BudgetExhausted:           budget.BudgetExhaustedFunc(),
		MaxRetries:                o.maxRetries,
		MaxLoopBack:               maxLoopBack,
		MaxAgentCalls:             o.maxAgentCalls,
		ModePolicy:                pol,
	}, verdicts, findings
}

func releaseVerdictCommit(stage string, provenance *artifactProvenance) func(asset.Phase) error {
	return func(p asset.Phase) error {
		if !releaseValidationPhase(stage, p) {
			return nil
		}
		return provenance.writeValidationReceipt(p)
	}
}

func loadRunScorecards(wf asset.Workflow, root string, logln func(string)) []routing.Scorecard {
	if releaseApprovalStage(wf.Stage) {
		return nil
	}
	cards, err := routing.LoadScorecards(scorecardPath(root))
	if err == nil {
		return cards
	}
	// History only affects observability. Keep the cold-start route, but make
	// the broken Eval producer visible instead of silently accepting bad data.
	logln(fmt.Sprintf("forge: WARNING scorecards unreadable (%v) — continuing with no history (routing unaffected; learning-loop read-back skipped)", err))
	return nil
}

func firstRunID(runIDs []string) string {
	if len(runIDs) > 0 {
		return runIDs[0]
	}
	return ""
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
func openRunResources(root, runBudgetUSD string, logln func(string), runIDs ...string) (*trace.Tracer, func(), *runBudget, error) {
	tracer, closeTrace, err := openTracer(root)
	if err != nil {
		return nil, nil, nil, err
	}
	if len(runIDs) > 0 && runIDs[0] != "" {
		tracer.RunID = runIDs[0]
	}
	quickDoctorCheck(root, tracer, logln)
	budget, err := newRunBudget(runBudgetUSD)
	if err != nil {
		closeTrace()
		return nil, nil, nil, err
	}
	return tracer, closeTrace, budget, nil
}

// runProbe owns one stage's acceptance snapshot and the exact gate set the
// orchestrator actually executes after mode filtering. Agent success invalidates
// the snapshot; the next gate (or convergence, when no later gate exists) refreshes
// it. Convergence reuses the gate snapshot when no agent wrote after that gate.
type runProbe struct {
	mu         sync.Mutex
	root       string
	load       func(string) (map[string]string, map[string]string, error)
	statuses   map[string]string
	categories map[string]string
	gates      []string
	seen       map[string]bool
	primed     bool
}

func newRunProbe(root string) *runProbe {
	return &runProbe{root: root, load: gate.ProbeAll, seen: map[string]bool{}}
}

func (p *runProbe) refresh() map[string]string {
	p.mu.Lock()
	defer p.mu.Unlock()
	if !p.primed {
		statuses, categories, err := p.load(p.root)
		if err != nil {
			fmt.Fprintf(os.Stderr, "forge run: acceptance probe unavailable (%v); gates degrade to N/A\n", err)
			statuses, categories = nil, nil
		}
		p.statuses, p.categories, p.primed = statuses, categories, true
	}
	return p.statuses
}

func (p *runProbe) invalidate() {
	p.mu.Lock()
	p.primed = false
	p.mu.Unlock()
}

func (p *runProbe) runGate(name string) gate.Result {
	p.mu.Lock()
	if !p.seen[name] {
		p.seen[name] = true
		p.gates = append(p.gates, name)
	}
	p.mu.Unlock()
	return gate.ResolveGate(p.root, name, p.refresh())
}

func (p *runProbe) current() (map[string]string, map[string]string) {
	p.refresh()
	p.mu.Lock()
	defer p.mu.Unlock()
	return p.statuses, p.categories
}

func (p *runProbe) actualGates() []string {
	p.mu.Lock()
	defer p.mu.Unlock()
	return append([]string(nil), p.gates...)
}

// runProbeExecutor marks the acceptance snapshot stale after a successful agent
// phase. It wraps the same executor in serial and dependency-wave parallel runs.
type runProbeExecutor struct {
	next  orchestrator.AgentExecutor
	probe *runProbe
}

func (e runProbeExecutor) Execute(ctx context.Context, p asset.Phase, mode string) error {
	if err := e.next.Execute(ctx, p, mode); err != nil {
		return err
	}
	e.probe.invalidate()
	return nil
}

// execEngine wires the real harness gates + the selected agent executor and
// runs the workflow, returning 0 on a clean run and 1 on the first failure.
// ctx carries cancellation so the engine can abort cleanly on SIGINT.
//
// Honesty: each stage probes only after agent work, then shares that snapshot
// between its gate phase and convergence until another agent succeeds. Chained
// stages therefore never inherit the prior stage's acceptance result.
func execEngine(ctx context.Context, firstWf asset.Workflow, o runOpts) int {
	lock := acquireRunLock(o.root, "forge run")
	if lock == nil {
		return 1
	}
	defer lock.Release()
	logln := func(s string) { fmt.Println(s) }
	lifecycle := resolveLifecycle(o)
	firstWf, resume, err := prepareChainResume(firstWf, o)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: cannot resume chain: %v\n", err)
		return 1
	}
	runID := ""
	if resume != nil {
		runID = resume.RunID
	}
	tracer, closeTrace, budget, err := openRunResources(o.root, o.runBudgetUSD, logln, runID)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	defer closeTrace()
	lifecycle, err = restoreChainRunOptions(&o, budget, resume, lifecycle)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: cannot resume chain: %v\n", err)
		return 1
	}
	if resume != nil {
		logln(fmt.Sprintf("forge run: resuming chain run_id=%s at stage=%s after completed=%v",
			resume.RunID, resume.CurrentStage, resume.CompletedStages))
	}
	return runStageChain(ctx, firstWf, o, logln, lifecycle, tracer, budget, resume)
}

// execOneStage runs a single workflow stage and reports convergence. Returns
// (met, rejected, 0) on success. rejected tells the caller that a durable
// rejection was acted on and may be consumed only after its own state commit.
func execOneStage(ctx context.Context, wf asset.Workflow, o runOpts, logln func(string), lifecycle string, tracer *trace.Tracer, budget *runBudget, chargeAgentCall func(int) (int, bool)) (bool, bool, int) {
	pol := mode.Effective(o.mode, lifecycle)
	boundary := resolveStageHostBoundary(wf, o, lifecycle, logln)
	eng, verdicts, _ := buildRunEngine(wf, o, logln, costEmitter(tracer, logln),
		boundary.runGate, pol, budget, boundary.autoRisk, boundary.autoRiskReasons, tracer.RunID)
	eng.ChargeAgentCall = chargeAgentCall
	if boundary.hostCommands {
		eng.Exec = runProbeExecutor{next: eng.Exec, probe: boundary.probe}
	}
	wireGateTrace(&eng, tracer, logln)
	logRunBanner(wf, o, lifecycle, pol)

	startPhase, rejected, err := resolveRejectionStartPhase(wf, o.root, logln)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return false, false, 1
	}
	if err := runWorkflow(ctx, eng, wf, o, logln, startPhase); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return false, rejected, 1
	}
	fmt.Printf("forge run: stage=%s workflow completed\n", wf.Stage)
	var probe, categories map[string]string
	var actualGates []string
	if boundary.hostCommands {
		probe, categories = boundary.probe.current()
		actualGates = boundary.probe.actualGates()
	}
	met := reportStageConvergence(
		wf, o.root, probe, categories, lifecycle, o.approved,
		verdicts, actualGates, boundary.proposalStage, boundary.releaseStage,
	)
	if boundary.hostCommands {
		windDownScorecardsForRun(wf, o, logln, 1, verdicts.wasReworked(), tracer.RunID)
	}
	return met, rejected, 0
}
