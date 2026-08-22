// engine_build.go — the orchestrator.Engine ASSEMBLY for the forge CLI: it selects
// the agent-phase executor (agentExecutor) and wires the shared Engine (buildRunEngine)
// that BOTH `forge run` (execEngine, main.go) and `forge evolve` (buildLoop, evolve.go)
// drive, so the two entry points never drift. Split out of main.go to stay under the
// harness's file-size budget while keeping the wiring it owns in one place.
package main

import (
	"context"
	"flag"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/attribution"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/materiality"
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

// buildRunEngine assembles the shared run/evolve Engine and its prompt,
// gate, verdict, finding, output-binding and provenance ledgers.
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
func buildRunEngine(wf asset.Workflow, o runOpts, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), runGate func(name string) gate.Result, pol mode.Policy, budget *runBudget, autoRisk string, autoRiskReasons []string, autoDims map[string]float64, autoReasons []string, runIDs ...string) (orchestrator.Engine, *verdictLedger, *reviewFindingsLedger) {
	engine, verdicts, findings, _, _ := buildRunEngineWithPhaseOutput(wf, o, logln, costSink, runGate, pol,
		budget, autoRisk, autoRiskReasons, autoDims, autoReasons,
		newPhaseOutputLedger(), runIDs...)
	return engine, verdicts, findings
}

func buildRunEngineWithPhaseOutput(wf asset.Workflow, o runOpts, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), runGate func(name string) gate.Result, pol mode.Policy, budget *runBudget, autoRisk string, autoRiskReasons []string, autoDims map[string]float64, autoReasons []string, phaseOut *phaseOutputLedger, runIDs ...string) (orchestrator.Engine, *verdictLedger, *reviewFindingsLedger, func() error, *outputBindingRuntime) {
	o.workflowStage = wf.Stage
	gates := newGateLedger()
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
		phaseTierResolver(o.mode, budget.SpendRatio, cards, logln, autoRisk, autoRiskReasons, autoDims, autoReasons),
		phaseOut, logln)
	wiring := buildEngineRuntimeWiring(wf, o, pol, logln, budget.feed(costSink), tierOf,
		phaseOut, enginePromptLedgers{context: ctxCache, gates: gates, verdicts: verdicts, findings: findings},
		firstRunID(runIDs))
	engine := orchestrator.Engine{
		Exec:                 wiring.exec,
		RunGate:              runGate,
		Log:                  logln,
		OnGateResult:         gates.record,
		AgentVerdict:         loopbackVerdict(verdicts),
		PhaseStart:           wiring.bindingExec.phaseStart,
		ValidateAgentSpawn:   wiring.bindingExec.agentSpawn,
		PhaseComplete:        wiring.bindingExec.phaseComplete,
		ValidateAgentVerdict: wiring.bindingExec.validateVerdict,
		WorkflowComplete:     wiring.bindingExec.workflowComplete,
		RequireAgentVerdict: func(p asset.Phase) bool {
			return releaseValidationPhase(wf.Stage, p) || requiredBuildReviewer(wf, o, p)
		},
		OnRequiredVerdictApproved: releaseVerdictCommit(wf.Stage, wiring.provenance),
		BudgetExhausted:           budget.BudgetExhaustedFunc(),
		MaxRetries:                o.maxRetries,
		MaxLoopBack:               maxLoopBack,
		MaxAgentCalls:             o.maxAgentCalls,
		ModePolicy:                pol,
	}
	return engine, verdicts, findings, bindingCompletionValidator(wiring, wf), wiring.binding
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
func phaseTierResolver(mode string, spendRatio func() float64, cards []routing.Scorecard, logln func(string), autoRisk string, autoRiskReasons []string, autoDims map[string]float64, autoReasons []string) func(p asset.Phase) string {
	return func(p asset.Phase) string {
		base := orchestrator.PhaseTier(p, mode)

		// Step 2: multi-dimensional score escalation (before budget, so a
		// scored-complex change can't be down-tiered). Mirrors risk escalation:
		// raise-only, capped at Opus, never lowering the base tier.
		tier := scoreAdjustedTier(base, autoDims)
		if tier != base && logln != nil {
			rs := ""
			if len(autoReasons) > 0 {
				rs = " (" + strings.Join(autoReasons, "; ") + ")"
			}
			logln(fmt.Sprintf("phase %s: auto-score=%.2f — escalating %s→%s%s",
				p.Name, routing.Score(autoDims, dimWeights), base, tier, rs))
		}

		// Step 3: risk-based escalation (before budget, so risk can't be down-tiered).
		tier = riskAdjustedTier(tier, autoRisk)
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
// non-safety-floor agents (v1.5 upgrade): non-floor agents pick the best candidate
// from [adj, ...cheaper] (cheaper wins only with sufficient history AND better
// quality); safety-floor agents (reviewer/architect/cto) stay on [adj]; an
// UNMAPPED agent (harness/gate phase) is SKIPPED, adj returned unchanged.
// logln nil -> no log output; cards empty (cold start) -> adj with an honest
// "no scorecard -> tier_default" reason.
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
// budget — fail-closed on either step.
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
// ctx/opts ride the probe so every live spawn (ProbeAll + complexity/arch gates)
// is bounded by the invocation's gate deadline/output cap.
type runProbe struct {
	mu         sync.Mutex
	ctx        context.Context
	root       string
	opts       gate.Options
	load       func(context.Context, string, gate.Options) (map[string]string, map[string]string, error)
	statuses   map[string]string
	categories map[string]string
	gates      []string
	seen       map[string]bool
	primed     bool
}

func newRunProbe(ctx context.Context, root string, opts gate.Options) *runProbe {
	return &runProbe{ctx: ctx, root: root, opts: opts, load: gate.ProbeAllWith, seen: map[string]bool{}}
}

func (p *runProbe) refresh() map[string]string {
	p.mu.Lock()
	defer p.mu.Unlock()
	if !p.primed {
		statuses, categories, err := p.load(p.ctx, p.root, p.opts)
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
	return gate.ResolveGateWith(p.ctx, p.root, name, p.refresh(), p.opts)
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

func freezeRunMateriality(fs *flag.FlagSet, o *runOpts) error {
	o.materialityExplicit = flagSet(fs, "materiality")
	value, err := materiality.FromCLI(o.materiality, o.materialityExplicit)
	if err != nil {
		return err
	}
	o.materiality = value
	return nil
}

func normalizeRunMateriality(o *runOpts) error {
	value, err := materiality.Normalize(o.materiality)
	if err != nil {
		return err
	}
	o.materiality = value
	return nil
}

func strictBuildReview(wf asset.Workflow, o runOpts) bool {
	return wf.Stage == "build" && materiality.RequiresStrictReview(o.materiality)
}

func materialityPolicy(wf asset.Workflow, o runOpts, policy mode.Policy) mode.Policy {
	if strictBuildReview(wf, o) {
		policy.Reviewer = true
	}
	return policy
}

func validateMaterialityWorkflow(wf asset.Workflow, o runOpts) error {
	if err := validateOutputBindingHost(wf, o); err != nil {
		return err
	}
	if !strictBuildReview(wf, o) {
		return nil
	}
	if err := asset.ValidateWorkflowStructure(wf); err != nil {
		return fmt.Errorf("materiality %s strict Build workflow: %w", o.materiality, err)
	}
	reviewers, qaPhases := 0, 0
	for _, phase := range wf.Phases {
		if phase.VerdictContract == strictReviewerContract(wf) {
			reviewers++
		}
		if phase.VerdictContract == asset.VerdictContractQAV1 {
			qaPhases++
		}
	}
	if reviewers != 1 {
		return fmt.Errorf("materiality %s requires exactly one Build %s phase (found %d)",
			o.materiality, strictReviewerContract(wf), reviewers)
	}
	if qaPhases == 0 {
		return fmt.Errorf("materiality %s strict Build requires at least one %s phase",
			o.materiality, asset.VerdictContractQAV1)
	}
	if o.parallel && declaresDependsOn(wf) {
		return fmt.Errorf("materiality %s %s requires serial directed loop-back orchestration",
			o.materiality, strictReviewerContract(wf))
	}
	return nil
}

func requiredBuildReviewer(wf asset.Workflow, o runOpts, phase asset.Phase) bool {
	return strictBuildReview(wf, o) && phase.VerdictContract == strictReviewerContract(wf)
}

func strictReviewerContract(wf asset.Workflow) string {
	if wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1 {
		return asset.VerdictContractReviewerV2
	}
	return asset.VerdictContractReviewerV1
}

func effectiveVerdictContractOf(wf asset.Workflow, o runOpts) func(string) string {
	return func(name string) string {
		for _, phase := range wf.Phases {
			if phase.Name != name {
				continue
			}
			if (phase.VerdictContract == asset.VerdictContractReviewerV1 ||
				phase.VerdictContract == asset.VerdictContractReviewerV2) && !strictBuildReview(wf, o) {
				return ""
			}
			return phase.VerdictContract
		}
		return ""
	}
}
