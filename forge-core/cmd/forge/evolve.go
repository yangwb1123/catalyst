// evolve.go — the autonomous-loop half of the forge CLI. `forge evolve` reuses
// the SAME workflow loading + agent executor + real harness gates as `forge run`
// (those shared pieces live in main.go), then wraps them in a convergence loop
// with checkpoint/resume, cross-session memory, and a trace audit trail under
// <root>/.forge. It runs a workflow until it converges, a tripwire fires, or the
// safety bound — never on round count alone.
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
	if name == "auto" {
		name = autoSelectWorkflow(o.root, fs, &o)
	}
	wf, err := loadWorkflow(o.root, name)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	if converge.IsHumanGate(wf.Stop) {
		return rejectHumanGate(wf.Stage, o.root)
	}
	iter, src := resolveMaxIter(fs, *maxIter, o)
	// Signal-aware context: SIGINT/SIGTERM cancel the context, triggering
	// graceful shutdown of the loop engine and its subprocesses.
	ctx, stop := withSignalCancellation()
	defer stop()
	return execLoop(ctx, wf, o, iter, src, *resume)
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
func execLoop(ctx context.Context, wf asset.Workflow, o runOpts, maxIter int, maxIterSource string, resume bool) int {
	lock := acquireRunLock(o.root, "forge evolve")
	if lock == nil {
		return 1
	}
	defer lock.Release()
	logln := func(s string) { fmt.Println(s) }
	start, prev, spentMicros, phaseStart, gatesGreen, err := resumeStart(o.root, resume)
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
	// Auto-diagnostic: run quick doctor checks and emit results as trace events.
	// This detects common issues before the evolve loop starts.
	quickDoctorCheck(o.root, tracer, logln)
	budget, err := newRunBudget(o.runBudgetUSD)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	budget.seed(spentMicros)
	loop, verdicts, findings := buildTracedLoop(ctx, wf, o, maxIter, logln, tracer, budget)
	loop.StartIter, loop.ResumePrev, loop.StartPhase, loop.ResumeGatesGreen = start, prev, phaseStart, gatesGreen
	loop.OnIteration = checkpointHook(o, wf, tracer, budget, logln, verdicts, findings)
	loop.OnPhase = phaseCheckpointHook(o, wf, budget, logln)
	fmt.Printf("forge evolve: stage=%s mode=%s max-iter=%d (%s) type=%s start-iter=%d (doom-loop tripwire=2)\n",
		wf.Stage, o.mode, maxIter, maxIterSource, stopTypeLabel(wf.Stop.Type), start)
	outcome, runErr := loop.Run(wf, o.mode)
	// Learning-loop wind-down: attribute the loop's REAL billed cost into the
	// scorecards. It runs BEFORE the deferred closeTrace() (the trace file is still
	// open, per scorecard_wind.go's flush-ordering invariant) and is gate-on-real-cost +
	// fail-loud-and-continue — so a dry/echo loop (the v1 default) skips it, and a
	// producer hiccup leaves the loop's outcome (reportLoop below) exactly as Run set it.
	// It runs whether the loop converged or hit the safety bound: real cost billed across
	// the iterations is attributable regardless of how the loop ended.
	// outcome.Iterations carries the real loop count; verdicts.wasReworked() the rework signal.
	windDownScorecards(wf, o, logln, outcome.Iterations, verdicts.wasReworked())
	return reportLoop(outcome, runErr)
}

// buildTracedLoop resolves auto-risk, builds the loop engine (buildLoop), wires its
// gate results into the trace (mirroring execEngine's wireGateTrace in
// engine_build.go), and sets the ctx/--parallel opt-in. Split out of execLoop
// purely to keep that function under the per-function line budget.
func buildTracedLoop(ctx context.Context, wf asset.Workflow, o runOpts, maxIter int, logln func(string), tracer *trace.Tracer, budget *runBudget) (orchestrator.LoopEngine, *verdictLedger, *reviewFindingsLedger) {
	autoRisk, autoRiskReasons := resolveAutoRisk(o.root)
	logAutoRisk(logln, "forge evolve", autoRisk, autoRiskReasons)
	loop, verdicts, findings := buildLoop(wf, o, maxIter, logln, costEmitter(tracer, logln), budget, autoRisk, autoRiskReasons)
	wireGateTrace(&loop.Engine, tracer, logln)
	loop.Ctx = ctx
	loop.Parallel = parallelEnabled(o, wf, logln, "forge evolve") // depends_on-gated opt-in
	return loop, verdicts, findings
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
func buildLoop(wf asset.Workflow, o runOpts, maxIter int, logln func(string), costSink func(phase, model string, usd float64, latency time.Duration), budget *runBudget, autoRisk string, autoRiskReasons []string) (orchestrator.LoopEngine, *verdictLedger, *reviewFindingsLedger) {
	probe := &loopProbe{root: o.root}
	// The Engine — with its four prompt/feedback ledgers — is built by the SAME
	// buildRunEngine `forge run` uses, so the two paths never drift. The only
	// evolve-specific seam is RunGate: a per-iteration refreshing probe (each
	// iteration re-measures gate state, so the reviewer always sees the LATEST).
	lifecycle := resolveLifecycle(o)
	eng, verdicts, findings := buildRunEngine(wf, o, logln, costSink,
		func(name string) gate.Result { return gate.ResolveGate(o.root, name, probe.refresh()) },
		mode.Effective(o.mode, lifecycle), budget, autoRisk, autoRiskReasons)
	approved := humanApproved(o.root, wf.Stage, o.approved)
	return orchestrator.NewLoopEngine(
		eng, wf.Stop,
		func() converge.Signals {
			statuses, categories := probe.current()
			return gatherSignals(o.root, wf, statuses, categories, lifecycle, approved, verdicts)
		},
		maxIter, 2, logln), verdicts, findings
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

// resumeStart resolves the loop's first iteration, seed completion, and persisted
// run spend. Without --resume it is a fresh run (0, -1.0, 0): the engine begins at
// iteration 1 with the -1.0 sentinel and a zero-spend seed. With --resume it loads
// the checkpoint and continues at cp.Iteration+1, seeding prev with the persisted
// completion so the stale/tripwire math is continuous AND returning cp.SpentUsdMicros
// (opaque micro-dollars) so execLoop can re-seed the budget — without which the
// run-level cost cap would restart at $0 across the resume and overspend. A MALFORMED
// checkpoint is a hard error (honesty-first): resuming must never silently degrade to
// a from-scratch rerun. A MISSING checkpoint with --resume is reported but tolerated
// as a fresh start. An old checkpoint without spent_usd_micros decodes that field to
// 0 (omitempty back-compat), so its resume seeds zero spend and behaves as before.
// phaseStart is cp.PhaseIndex — the phase the FIRST resumed iteration begins at when
// the crash happened mid-iteration (0 for a clean iteration-boundary checkpoint, so
// the iteration replays in full exactly as before phase-granular checkpointing).
func resumeStart(root string, resume bool) (start int, prev float64, spentMicros int64, phaseStart int, gatesGreen bool, err error) {
	if !resume {
		return 0, -1.0, 0, 0, false, nil
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil {
		return 0, 0, 0, 0, false, fmt.Errorf("--resume: malformed checkpoint at %s: %w", checkpointPath(root), err)
	}
	if !found {
		fmt.Fprintf(os.Stderr, "forge evolve: --resume found no checkpoint at %s; starting fresh\n", checkpointPath(root))
		return 0, -1.0, 0, 0, false, nil
	}
	at := ""
	if cp.PhaseIndex > 0 {
		at = fmt.Sprintf(", phase %d", cp.PhaseIndex)
	}
	fmt.Printf("forge evolve: resuming from iteration %d%s (roadmap=%.0f%%, last reason: %s)\n",
		cp.Iteration+1, at, cp.RoadmapCompletion*100, cp.Reason)
	// GatesGreen threads through too, so LoopEngine.ResumeGatesGreen keeps the
	// resumed stale detector continuous on both axes (see its doc comment).
	return cp.Iteration + 1, cp.RoadmapCompletion, cp.SpentUsdMicros, cp.PhaseIndex, cp.GatesGreen, nil
}

// checkpointHook builds the loop's post-measurement OnIteration callback: after
// each round's work AND its measurement, it persists a checkpoint, appends the
// round's trajectory to memory, and emits an iteration trace event carrying the
// iteration's measured wall-clock duration (durationMs from the loop) — the value
// scorecard p95_latency reads. Fail-LOUD-and-continue (deliberately NOT fail-closed):
// a checkpoint/memory write failure is a loud stderr WARNING (+ trace status
// "checkpoint-write-failed") but the loop keeps running — a 24h run must not abort
// on a transient disk hiccup, and the failure is never swallowed. Contrast
// openTracer, which DOES fail-closed (losing the audit trail is the blind spot
// trace prevents). Accepts the verdict/findings ledgers so it can extract
// structured Reflect-step lessons alongside the trajectory entry.
func checkpointHook(o runOpts, wf asset.Workflow, tracer *trace.Tracer, budget *runBudget, logln func(string), verdicts *verdictLedger, findings *reviewFindingsLedger) func(int, converge.Signals, int64) {
	return func(i int, sig converge.Signals, durationMs int64) {
		// SpentUsdMicros records the run's cumulative billed cost AT THIS iteration so a
		// later --resume re-seeds the budget instead of restarting the cap from $0 (the
		// cross-resume overspend gap). budget owns the dollar->micro conversion; persist
		// stores only the opaque int. This per-iteration checkpoint records the spend through
		// the COMPLETED iteration; phaseCheckpointHook records a FINER mid-iteration spend after
		// each agent phase, so on resume the under-count is at most one in-flight phase's partial
		// (cost.go seed/SpentUsdMicros documents it). An unbudgeted run feeds nothing, so this
		// is 0 and the checkpoint's spent_usd_micros stays omitempty (byte-identical).
		cp := persist.Checkpoint{
			Workflow: wf.Stage, Mode: o.mode, Iteration: i,
			RoadmapCompletion: sig.RoadmapCompletion, GatesGreen: sig.GatesGreen,
			Reason: "iteration complete", UpdatedAtUnix: time.Now().Unix(),
			SpentUsdMicros: budget.SpentUsdMicros(),
		}
		status, detail := "ok", checkpointDetail(sig)
		if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil {
			status = "checkpoint-write-failed"
			detail = err.Error()
			logln(fmt.Sprintf("forge evolve: WARNING checkpoint write failed (recovery state NOT durable): %v", err))
		}
		recordMemory(o.root, wf, i, sig, verdicts, findings, logln)
		emitTrace(tracer, trace.Event{Kind: "iteration", Name: fmt.Sprintf("%d", i), Status: status, DurationMs: durationMs, Detail: detail}, logln)
	}
}

// phaseCheckpointHook builds the loop's iteration-aware OnPhase callback: after each
// agent phase completes MID-iteration, it persists a PHASE-granular checkpoint so a
// crash resumes at the next unstarted phase instead of replaying every completed
// (billed) agent phase. It PRESERVES the last completed iteration's measured signals
// — read from the on-disk checkpoint the per-iteration hook wrote — advancing only
// PhaseIndex (phaseIdx+1) and the up-to-the-phase cumulative spend (a FINER budget
// re-seed than the iteration-granular one). At a clean iteration boundary
// checkpointHook overwrites this with PhaseIndex=0. Fail-LOUD-and-continue like
// checkpointHook: a write failure WARNs but never aborts the loop.
func phaseCheckpointHook(o runOpts, wf asset.Workflow, budget *runBudget, logln func(string)) func(iter, phaseIdx int) {
	return func(iter, phaseIdx int) {
		prev, _, _ := persist.Load(checkpointPath(o.root)) // not-found/malformed -> zero signals
		cp := persist.Checkpoint{
			Workflow: wf.Stage, Mode: o.mode, Iteration: iter - 1,
			RoadmapCompletion: prev.RoadmapCompletion, GatesGreen: prev.GatesGreen,
			PhaseIndex: phaseIdx + 1, Reason: "phase complete (mid-iteration)",
			UpdatedAtUnix: time.Now().Unix(), SpentUsdMicros: budget.SpentUsdMicros(),
		}
		if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil {
			logln(fmt.Sprintf("forge evolve: WARNING phase checkpoint write failed (recovery state NOT durable): %v", err))
		}
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

// openTracer creates an append-mode tracer to <root>/.forge/trace.jsonl, rotating
// the file when it exceeds 10MB (analysis §2.1) to prevent unbounded growth.
// It also returns a closer. A dir/open failure is returned (fail-closed) — the loop
// must not run blind on observability.
func openTracer(root string) (*trace.Tracer, func(), error) {
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		return nil, func() {}, fmt.Errorf("create .forge dir: %w", err)
	}
	tp := filepath.Join(forgeDir(root), "trace.jsonl")
	// Rotate trace if it exceeds 10 MB: rename to trace.jsonl.1, start fresh. Callers
	// always hold the run.lock (runlock.Acquire) before reaching here, so no other
	// forge process can be rotating concurrently; a stale .1 backup from an old,
	// unlocked binary would just be overwritten by the next rotation regardless.
	const maxTraceBytes int64 = 10 << 20 // 10 MB
	if st, err := os.Stat(tp); err == nil && st.Size() > maxTraceBytes {
		os.Rename(tp, tp+".1") // best-effort; ignore error (rotation is optimization, not correctness)
	}
	f, err := os.OpenFile(tp, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o644)
	if err != nil {
		return nil, func() {}, fmt.Errorf("open trace file: %w", err)
	}
	t := trace.NewTracer(f)
	stampRunID(t)
	return t, func() { f.Close() }, nil
}

// withSignalCancellation returns a context cancelled by SIGINT/SIGTERM.
// The returned stop must be called (via defer) to restore default signal handling.
func withSignalCancellation() (context.Context, func()) {
	return signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
}
