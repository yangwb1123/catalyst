// evolve.go — the autonomous-loop half of the forge CLI. `forge evolve` reuses
// the SAME workflow loading + agent executor + real harness gates as `forge run`
// (those shared pieces live in main.go), then wraps them in a convergence loop
// with checkpoint/resume, cross-session memory, and a trace audit trail under
// <root>/.forge. It runs a workflow until it converges, a tripwire fires, or the
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"syscall"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/memory"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/persist"
	"forgeos/forge-core/internal/trace"
)

// cmdEvolve loops a workflow until it converges, a tripwire fires, or the
// safety bound — the autonomous-loop entry point (real agents via --executor).
// The safety bound (--max-iter) is the THIRD subsystem the central knob drives:
// when the operator does NOT pass --max-iter, the loop's iteration budget comes
// from the mode's evolve depth (mode.Effective(...).EvolveMaxIter() — explorer's
// opportunistic→2 vs engineering's thorough→10), so a fast posture loops shallowly
// and a rigorous one loops deep. An EXPLICIT --max-iter still wins (back-compat):
// resolveMaxIter only fills the default the flag would otherwise have used.
func cmdEvolve(args []string) int {
	fs := flag.NewFlagSet("evolve", flag.ContinueOnError)
	var o runOpts
	bindRunOpts(fs, &o)
	// Default is the legacy 5; resolveMaxIter overrides it with the mode default
	// ONLY when the operator did not pass --max-iter (detected via fs.Visit).
	maxIter := fs.Int("max-iter", 5, "safety bound on loop iterations (default: from --mode's evolve depth; not the goal)")
	resume := fs.Bool("resume", false, "resume from <root>/.forge/checkpoint.json (errors out on a malformed checkpoint)")
	name, flagArgs := splitPositional(args)
	if name == "" {
		fmt.Fprintln(os.Stderr, "forge evolve: exactly one <workflow> required")
		usage()
		return 2
	}
	if err := fs.Parse(flagArgs); err != nil {
		return 2
	}
	o.root = gate.RepoRoot(o.root)
	if code := rejectPendingPromotionAtEntry("forge evolve", o.root); code != 0 {
		return code
	}
	autoSelected := name == "auto"
	if autoSelected {
		name = autoSelectWorkflow(o.root, fs, &o)
	}
	// Freeze lifecycle so policy, checkpoint identity, and every iteration agree
	// even if project.yml changes while this invocation is running.
	freezeRunOptions(fs, &o)
	wf, err := loadEvolveCommandWorkflow(o.root, name, o)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	if autoSelected && wf.Stage != "evolve" {
		return runAutoSelectedOneShot(wf, o, fs, *resume)
	}
	iter, src, code := validateEvolveEntry(wf, o, fs, *maxIter)
	if code != 0 {
		return code
	}
	// SIGINT/SIGTERM cancel the loop and its subprocesses gracefully.
	ctx, stop := withSignalCancellation()
	defer stop()
	return execLoop(ctx, wf, o, iter, src, *resume)
}

// runAutoSelectedOneShot preserves `forge evolve auto` as an executable entry
// point when detection chooses a non-loop spine stage (greenfield → discover).
// Explicit evolve-only flags fail with an actionable transfer instead of being
// silently ignored; otherwise execution uses the exact `forge run` engine.
func runAutoSelectedOneShot(wf asset.Workflow, o runOpts, fs *flag.FlagSet, resume bool) int {
	if resume || flagSet(fs, "max-iter") {
		fmt.Fprintf(os.Stderr,
			"forge evolve: auto selected one-shot stage %q; --resume/--max-iter apply only to evolve. Run `%s` instead.\n",
			wf.Stage, suggestionCommand(workflowSuggestion{
				Workflow: wf.Stage, Mode: o.mode, Lifecycle: o.lifecycle,
			}))
		return 2
	}
	if o.chain && o.maxChainStages < 1 {
		fmt.Fprintln(os.Stderr, "forge run: --max-chain-stages must be >= 1")
		return 2
	}
	fmt.Printf("forge evolve: auto routing stage=%s through one-shot `forge run` semantics\n", wf.Stage)
	ctx, stop := withSignalCancellation()
	defer stop()
	return execEngine(ctx, wf, o)
}

// resolveMaxIter picks the loop's safety bound and reports WHERE it came from.
// An EXPLICIT --max-iter always wins (back-compat — fs.Visit reports only flags
// the operator actually set, exactly as route.go's recordRiskFlagOrigins detects
// a deliberate flag); absent it, the bound is the mode's evolve-depth default
// (mode.Effective(mode, lifecycle).EvolveMaxIter()). The source string is purely
// for the run banner's honesty. lifecycle is resolved the same way every other
// `forge run`/`evolve` path resolves it (resolveLifecycle: flag, else project.yml,
// else mvp), so production's veto raises a shallow mode's loop here too.
func resolveMaxIter(fs *flag.FlagSet, flagVal int, o runOpts) (iter int, source string) {
	if flagSet(fs, "max-iter") {
		return flagVal, fmt.Sprintf("explicit --max-iter=%d", flagVal)
	}
	lifecycle := resolveLifecycle(o)
	n := mode.Effective(o.mode, lifecycle).EvolveMaxIter()
	return n, fmt.Sprintf("mode=%s lifecycle=%s evolve-depth default", o.mode, lifecycle)
}

// flagSet reports whether the named flag was explicitly passed on the command
// line (fs.Visit walks only set flags). This is the back-compat hinge: an
// explicit --max-iter must override the mode default, never the reverse.
func flagSet(fs *flag.FlagSet, name string) bool {
	seen := false
	fs.Visit(func(f *flag.Flag) {
		if f.Name == name {
			seen = true
		}
	})
	return seen
}

// rejectHumanGate fails closed when `forge evolve` is pointed at a human_gate
// workflow. A human_gate (design->build) is a SINGLE-SHOT human-approval gate —
// it is non-bypassable and semantically must never be driven by an autonomous
// convergence loop, which would otherwise spin it round after round and (absent
// the depth-two guard below) risk treating a stray satisfied all_of as
// "converged" without any approval. The only honest run path for a human_gate is
// `forge run`, where the operator supplies approval via --approved or an on-disk
// marker. This is the PRIMARY, outermost defense: a human_gate never enters the
// loop at all. Exit is non-zero (1) so a script/CI driving evolve cannot mistake
// the refusal for a clean convergence.
func rejectHumanGate(stage string, root string) int {
	// Check for existing checkpoint state to give helpful guidance.
	cpPath := filepath.Join(root, ".forge", "checkpoint.json")
	cpHint := ""
	if data, err := os.ReadFile(cpPath); err == nil {
		var cp struct {
			Iteration         int     `json:"iteration"`
			RoadmapCompletion float64 `json:"roadmap_completion"`
			GatesGreen        bool    `json:"gates_green"`
		}
		if json.Unmarshal(data, &cp) == nil {
			cpHint = fmt.Sprintf(
				"\n  Current state: checkpoint at iteration %d, roadmap=%.0f%%, gates=%s",
				cp.Iteration, cp.RoadmapCompletion*100,
				map[bool]string{true: "green", false: "red"}[cp.GatesGreen],
			)
		}
	}
	approvedPath := filepath.Join(root, ".forge", stage+".approved")
	approveHint := ""
	if _, err := os.Stat(approvedPath); err == nil {
		approveHint = "  Approval marker exists: " + approvedPath + "\n"
	}
	fmt.Fprintf(os.Stderr,
		"forge evolve: %q is a human_gate workflow — a single-shot approval gate "+
			"must not be driven by an autonomous loop.\n"+
			"  human_gate is a non-bypassable, one-time human-approval gate (design->build); "+
			"use `forge run %s [--approved]`, not `forge evolve`.%s\n"+
			"%s"+
			"  To approve and continue:\n"+
			"    forge run %s --approved\n"+
			"  To check pending approvals:\n"+
			"    forge approve list\n",
		stage, stage, cpHint, approveHint, stage)
	return 1
}

// execLoop wires the loop engine (real gates + selected executor + live signals)
// and runs it to convergence, a tripwire, or the safety bound, reporting how it
// ended. For an external-stop workflow (e.g. evolve), reaching the safety bound is
// the EXPECTED clean outcome and the CLI exits 0 — never a round-count failure.
// Resilience + memory wiring: each iteration's post-measurement hook persists a
// checkpoint, appends the round's trajectory to memory, and emits a trace event
// under <root>/.forge/ — so a crashed run can --resume, later rounds recall what
// happened, and the run stays auditable.
// invocationGateOptions resolves the invocation's gate options ONCE, BEFORE
// any spawn: the ONLY gate config source is ResolveOptions (from
// FORGE_GATE_TIMEOUT) — never o.timeout, the per-AGENT knob (regression pin).
// A bad env fails the loop up front, naming the variable and value.
func invocationGateOptions(o runOpts) (runOpts, error) {
	gateOpts, err := gate.ResolveOptions(gate.CLIInput{EnvTimeout: os.Getenv(gate.EnvTimeout)})
	if err != nil {
		return o, err
	}
	o.gateOpts = gateOpts
	return o, nil
}

func execLoop(ctx context.Context, wf asset.Workflow, o runOpts, maxIter int, maxIterSource string, resume bool) int {
	o, err := invocationGateOptions(o)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	lock := acquireRunLockForOptions(o, "forge evolve")
	if lock == nil {
		return 1
	}
	defer lock.Release()
	logln := func(s string) { fmt.Println(s) }
	resumed, err := prepareLoopResume(wf, &o, resume)
	if err != nil { // malformed checkpoint: fail closed, never silently restart.
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	tracer, closeTrace, err := openTracer(o.root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	defer closeTrace()
	// Auto-diagnostic: quick doctor checks before the evolve loop starts.
	quickDoctorCheck(o.root, tracer, logln)
	budget, err := newResumedRunBudget(o.runBudgetUSD, resumed)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	loop, verdicts, findings, phaseOut := buildTracedLoop(ctx, wf, o, maxIter, logln, tracer, budget)
	loop.OnIteration = checkpointHook(o, wf, tracer, budget, logln, verdicts, findings)
	loop.OnPhase = phaseCheckpointHook(o, wf, budget, phaseOut, logln)
	if err := applyLoopResume(&loop, wf, resumed); err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: --resume: %v\n", err)
		return 1
	}
	stageLine := fmt.Sprintf("stage=%s mode=%s max-iter=%d (%s) type=%s start-iter=%d (doom-loop tripwire=2)",
		wf.Stage, o.mode, maxIter, maxIterSource, stopTypeLabel(wf.Stop.Type), resumed.start)
	fmt.Println("forge evolve:", stageLine)

	outcome, runErr := loop.Run(wf, o.mode)
	return windDownAndReport(wf, o, logln, outcome, runErr, verdicts.wasReworked(), tracer.RunID)
}

// windDownAndReport attributes the loop's REAL billed cost into the scorecards
// BEFORE closeTrace (flush-ordering invariant), gate-on-real-cost +
// fail-loud-and-continue; dry/echo loops skip it, producer hiccups leave the
// outcome exactly as Run set it, and outcome.Iterations + verdicts.wasReworked()
// carry the attribution signals.
func windDownAndReport(wf asset.Workflow, o runOpts, logln func(string), outcome orchestrator.LoopOutcome, runErr error, wasReworked bool, runID string) int {
	if !o.evolveProposalOnly {
		windDownScorecardsForRun(wf, o, logln, outcome.Iterations, wasReworked, runID)
	}
	return reportLoop(outcome, runErr)
}

// buildTracedLoop resolves auto-risk, builds the loop engine (buildLoop), wires its
// gate results into the trace (mirroring execEngine's wireGateTrace in
// engine_build.go), and sets the ctx/--parallel opt-in. Split out of execLoop
// purely to keep that function under the per-function line budget.
func buildTracedLoop(ctx context.Context, wf asset.Workflow, o runOpts, maxIter int, logln func(string), tracer *trace.Tracer, budget *runBudget) (orchestrator.LoopEngine, *verdictLedger, *reviewFindingsLedger, *phaseOutputLedger) {
	var autoRisk string
	var autoRiskReasons []string
	if !proposalOnlyEvolve(wf, o, resolveLifecycle(o)) {
		autoRisk, autoRiskReasons = resolveAutoRisk(o.root)
		logAutoRisk(logln, "forge evolve", autoRisk, autoRiskReasons)
	}
	loop, verdicts, findings, phaseOut := buildLoop(ctx, wf, o, maxIter, logln, costEmitter(tracer, logln), budget, autoRisk, autoRiskReasons, tracer.RunID)
	wireGateTrace(&loop.Engine, tracer, logln)
	loop.Ctx = ctx
	loop.Parallel = parallelEnabled(o, wf, logln, "forge evolve") // depends_on-gated opt-in
	return loop, verdicts, findings, phaseOut
}

// buildLoop constructs the loop engine: real gates + selected executor + live
// signals, with one acceptance probe per iteration shared by that iteration's gate
// phases and convergence check (refresh-before-reuse, no double-spawn). The engine
// receives the FULL wf.Stop so convergence runs through converge.Converge (which
// honors every stop shape — a human_gate that somehow reaches the loop is judged
// by approval alone, never a satisfied all_of; depth-two defense, cmdEvolve already
// refuses one up front).
// costSink threads the SAME tracer execLoop already owns into the agent executor, so
// a real claude phase's billed cost+latency lands as a trace-visible `kind:"agent"`
// event, model-stamped and interleaved with the per-iteration events.
// budget is the loop-wide run budget (created in execLoop, reused here); buildRunEngine
// wraps costSink with budget.feed and wires budget.BudgetExhaustedFunc() into the Engine
// so the cumulative dollar cap meters spend across EVERY iteration (Engine built once,
// reused). Unset (--run-budget-usd empty) ⇒ a no-op accumulator + nil puller ⇒ unchanged.
// Returns the LoopEngine plus the verdict/findings ledgers buildRunEngine built, so
// execLoop can thread rework+trajectory into the scorecard wind-down and the Reflect
// memory step without rebuilding or re-exposing the Engine internals.
func buildLoop(ctx context.Context, wf asset.Workflow, o runOpts, maxIter int, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), budget *runBudget, autoRisk string, autoRiskReasons []string, runIDs ...string) (orchestrator.LoopEngine, *verdictLedger, *reviewFindingsLedger, *phaseOutputLedger) {
	probe := &loopProbe{root: o.root, ctx: ctx, opts: o.gateOpts}
	// The Engine — with its four prompt/feedback ledgers — is built by the SAME
	// buildRunEngine `forge run` uses, so the two paths never drift. The only
	// evolve-specific seam is RunGate: a per-iteration refreshing probe (each
	// iteration re-measures gate state, so the reviewer always sees the LATEST).
	lifecycle := resolveLifecycle(o)
	policy := mode.Effective(o.mode, lifecycle)
	autoDims, autoDimsReasons := resolveAutoDims(o.root)
	proposalOnly := policy.BuildHalted() || policy.EvolveProposalOnly()
	runGate := proposalEvolveGateRunner(ctx, o.root, probe, proposalOnly, o.gateOpts)
	phaseOut := newPhaseOutputLedger()
	eng, verdicts, findings := buildRunEngineWithPhaseOutput(wf, o, logln, costSink,
		runGate, policy, budget, autoRisk, autoRiskReasons, autoDims, autoDimsReasons,
		phaseOut, runIDs...)
	approved := humanApproved(o.root, wf.Stage, o.approved)
	signals := func() converge.Signals {
		statuses, categories := probe.current()
		return gatherSignals(ctx, o.gateOpts, o.root, wf, statuses, categories, lifecycle, approved, verdicts)
	}
	if proposalOnly {
		signals = func() converge.Signals {
			return proposalLoopSignals(o.root, wf, approved, verdicts)
		}
	}
	loop := orchestrator.NewLoopEngine(
		eng, wf.Stop, signals, maxIter, 2, logln,
	)
	return loop, verdicts, findings, phaseOut
}

// reportLoop prints how the loop ended and maps it to an exit code: a converged
// (or clean external-stop) outcome is exit 0; anything else is exit 1.
func reportLoop(out orchestrator.LoopOutcome, err error) int {
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	fmt.Printf("forge evolve: ended after %d iter — converged=%v (%s)\n", out.Iterations, out.Converged, out.Reason)
	if out.Converged {
		return 0
	}
	return 1
}

// checkpointHook builds the loop's post-measurement OnIteration callback: after
// each round's work AND its measurement, it persists a checkpoint, appends the
// round's trajectory to memory, and emits an iteration trace event carrying the
// iteration's measured wall-clock duration (durationMs from the loop) — the value
// scorecard p95_latency reads. A checkpoint failure is fail-closed and returned
// after emitting a failure trace: starting another iteration without durable
// counters/caps would let resume replay work and regain resources. Memory/trace
// append failures retain their independent fail-loud behavior. Accepts the
// verdict/findings ledgers so it can extract structured Reflect-step lessons
// alongside the trajectory entry.
func checkpointHook(o runOpts, wf asset.Workflow, tracer *trace.Tracer, budget *runBudget, logln func(string), verdicts *verdictLedger, findings *reviewFindingsLedger) func(int, converge.Signals, int64) error {
	workflowDigest := checkpointWorkflowDigest(wf)
	lifecycle := resolveLifecycle(o)
	return func(i int, sig converge.Signals, durationMs int64) error {
		// SpentUsdMicros records the run's cumulative billed cost AT THIS iteration so a
		// later --resume re-seeds the budget instead of restarting the cap from $0 (the
		// cross-resume overspend gap). budget owns the dollar->micro conversion; persist
		// stores only the opaque int. This per-iteration checkpoint records the spend through
		// the COMPLETED iteration; phaseCheckpointHook records a FINER mid-iteration spend after
		// each agent phase, so on resume the under-count is at most one in-flight phase's partial
		// (cost.go seed/SpentUsdMicros documents it). An unbudgeted run feeds nothing, so this
		// is 0; checkpoint v3 persists both the zero cap and zero spend explicitly.
		cp := persist.Checkpoint{
			Workflow: wf.Stage, WorkflowDigest: workflowDigest,
			Mode: o.mode, Lifecycle: lifecycle, Iteration: i,
			RoadmapCompletion: sig.RoadmapCompletion, GatesGreen: sig.GatesGreen,
			Reason: "iteration complete", UpdatedAtUnix: time.Now().Unix(),
			SpentUsdMicros: budget.SpentUsdMicros(), BudgetCapMicros: budget.CapUsdMicros(),
			MaxAgentCalls: o.maxAgentCalls, MaxLoopBacks: maxLoopBack,
		}
		status, detail := "ok", checkpointDetail(sig)
		saveErr := persist.Save(checkpointPath(o.root), cp, 5)
		if saveErr != nil {
			status = "checkpoint-write-failed"
			detail = saveErr.Error()
			logln(fmt.Sprintf("forge evolve: ERROR checkpoint write failed; stopping before another iteration (recovery state NOT durable): %v", saveErr))
		}
		recordMemory(o.root, wf, i, sig, verdicts, findings, logln)
		emitTrace(tracer, trace.Event{Kind: "iteration", Name: fmt.Sprintf("%d", i), Status: status, DurationMs: durationMs, Detail: detail}, logln)
		if saveErr != nil {
			return fmt.Errorf("persist iteration checkpoint: %w", saveErr)
		}
		return nil
	}
}

// recordMemory appends this iteration's knowledge entries to the cross-session store.
// Three entry types, each honest about its source:
//  1. Trajectory KindLesson (always): the iteration's measured convergence signals —
//     roadmap %, gate state. Real signal regardless of executor (dry or live).
//  2. Reviewer findings KindLesson (only when rework happened): structured lessons from
//     REQUEST_CHANGES findings, one per target phase. Only written when real agent output
//     exists (verdicts.wasReworked() is true — dry/echo runs never set this).
//  3. Gate-failure KindGap (when !sig.GatesGreen): surfaces the gate failure explicitly
//     so the next iteration's prompt context names it as an open gap.
//
// Fail-LOUD-and-continue on each append: a write failure is warned but never aborts
// the loop (enrichment, not correctness). Nil ledgers (dry/echo runs) produce only the
// trajectory entry.
func recordMemory(root string, wf asset.Workflow, i int, sig converge.Signals, verdicts *verdictLedger, findings *reviewFindingsLedger, logln func(string)) {
	appendEntry := func(e memory.Entry) {
		if err := memory.Append(memoryPath(root), e); err != nil {
			logln(fmt.Sprintf("forge evolve: WARNING memory append failed (entry NOT recorded): %v", err))
		}
	}
	now := time.Now().Unix()
	// 1. Trajectory (always)
	appendEntry(memory.Entry{
		Kind: memory.KindLesson, Topic: wf.Stage, Source: "evolve", Iteration: i, CreatedAtUnix: now,
		Detail: fmt.Sprintf("iter %d: roadmap=%.0f%%, gates_green=%v", i, sig.RoadmapCompletion*100, sig.GatesGreen),
	})
	// 2. Reviewer REQUEST_CHANGES findings → structured KindLesson per target phase
	if verdicts.wasReworked() {
		for target, text := range findings.allFindings() {
			if text == "" {
				continue
			}
			appendEntry(memory.Entry{
				Kind: memory.KindLesson, Topic: wf.Stage, Source: "reviewer", Iteration: i, CreatedAtUnix: now,
				Detail: fmt.Sprintf("reviewer requested changes for %s: %s", target, text),
			})
		}
	}
	recordGateFailureMemory(appendEntry, wf, i, sig, now)
	compactMemoryIfDue(root, i, logln)
}

// recordGateFailureMemory appends entry type 3 (gate failures → KindGap; iter 2+
// persistent failure → KindDecision with escalation hint), split out of recordMemory
// purely to keep that function under the per-function line budget.
func recordGateFailureMemory(appendEntry func(memory.Entry), wf asset.Workflow, i int, sig converge.Signals, now int64) {
	if sig.GatesGreen {
		return
	}
	appendEntry(memory.Entry{
		Kind: memory.KindGap, Topic: wf.Stage, Source: "evolve", Iteration: i, CreatedAtUnix: now,
		Detail: fmt.Sprintf("iter %d: gates not green at roadmap=%.0f%% — fix gate failures before claiming convergence", i, sig.RoadmapCompletion*100),
	})
	if i >= 2 {
		// Reflect self-analysis: loop has NOT fixed gate failures across multiple iterations.
		appendEntry(memory.Entry{
			Kind: memory.KindDecision, Topic: wf.Stage, Source: "evolve", Iteration: i, CreatedAtUnix: now,
			Detail: fmt.Sprintf("iter %d: RECURRING gate failure (gates not green across %d+ iterations) — self-analysis: current approach not converging; options: (1) review gate output for specific failing check, (2) escalate tier for failing phases, (3) reduce scope of this iteration's changes", i, i),
		})
	}
}

// compactMemoryIfDue compacts the memory store every 10 iterations when it exceeds
// the threshold (groups old entries by kind, keeps the most recent keepPerKind per
// kind, replaces the rest with summaries) — split out of recordMemory purely to
// keep that function under the per-function line budget.
func compactMemoryIfDue(root string, i int, logln func(string)) {
	if i%10 != 0 {
		return
	}
	removed, compacted, err := memory.Compact(memoryPath(root), memory.DefaultCompactThreshold, memory.DefaultCompactKeepPerKind, memory.CompactAgeSeconds)
	if err != nil {
		logln(fmt.Sprintf("forge evolve: WARNING memory compaction failed: %v", err))
	} else if compacted {
		logln(fmt.Sprintf("forge evolve: memory compaction removed %d entries", removed))
	}
}

// checkpointDetail renders the measured signals for the iteration trace line.
func checkpointDetail(sig converge.Signals) string {
	return fmt.Sprintf("roadmap=%.0f%% gates_green=%v", sig.RoadmapCompletion*100, sig.GatesGreen)
}

// emitTrace writes one trace event, logging (never swallowing) a write failure.
func emitTrace(tracer *trace.Tracer, ev trace.Event, logln func(string)) {
	if err := tracer.Emit(ev); err != nil {
		logln(fmt.Sprintf("forge evolve: WARNING trace emit failed: %v", err))
	}
}

// stopTypeLabel renders the stop type for the run banner, "(none)" when unset.
func stopTypeLabel(t string) string {
	if t == "" {
		return "(none)"
	}
	return t
}

// checkpointPath is where the per-iteration resume snapshot is persisted.
func checkpointPath(root string) string { return filepath.Join(forgeDir(root), "checkpoint.json") }

// withSignalCancellation returns a context cancelled by SIGINT/SIGTERM.
// The returned stop must be called (via defer) to restore default signal handling.
func withSignalCancellation() (context.Context, func()) {
	return signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
}
