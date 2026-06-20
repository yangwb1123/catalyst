// evolve.go — the autonomous-loop half of the forge CLI. `forge evolve` reuses
// the SAME workflow loading + agent executor + real harness gates as `forge run`
// (those shared pieces live in main.go), then wraps them in a convergence loop
// with checkpoint/resume, cross-session memory, and a trace audit trail under
// <root>/.forge. It runs a workflow until it converges, a tripwire fires, or the
// safety bound — never on round count alone.
package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
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
//
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
	wf, err := loadWorkflow(o.root, name)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge evolve: %v\n", err)
		return 1
	}
	if converge.IsHumanGate(wf.Stop) {
		return rejectHumanGate(wf.Stage)
	}
	iter, src := resolveMaxIter(fs, *maxIter, o)
	return execLoop(wf, o, iter, src, *resume)
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
func rejectHumanGate(stage string) int {
	fmt.Fprintf(os.Stderr,
		"forge evolve: %q is a human_gate workflow — a single-shot approval gate "+
			"must not be driven by an autonomous loop.\n"+
			"  human_gate is a non-bypassable, one-time human-approval gate (design->build); "+
			"use `forge run %s [--approved]`, not `forge evolve`.\n",
		stage, stage)
	return 1
}

// execLoop wires the loop engine (real gates + selected executor + live signals)
// and runs it to convergence, a tripwire, or the safety bound, reporting how it
// ended. For an external-stop workflow (e.g. evolve), reaching the safety bound is
// the EXPECTED clean outcome and the CLI exits 0 — never a round-count failure.
//
// Resilience + memory wiring: each iteration's post-measurement hook persists a
// checkpoint, appends the round's trajectory to memory, and emits a trace event
// under <root>/.forge/ — so a crashed run can --resume, later rounds recall what
// happened, and the run stays auditable.
func execLoop(wf asset.Workflow, o runOpts, maxIter int, maxIterSource string, resume bool) int {
	logln := func(s string) { fmt.Println(s) }
	start, prev, err := resumeStart(o.root, resume)
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
	loop := buildLoop(wf, o, maxIter, logln, costEmitter(tracer, logln))
	loop.StartIter, loop.ResumePrev = start, prev
	loop.OnIteration = checkpointHook(o, wf, tracer, logln)
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
	windDownScorecards(wf, o, logln)
	return reportLoop(outcome, runErr)
}

// buildLoop constructs the loop engine: real gates + selected executor + live
// signals, with one acceptance probe per iteration shared by that iteration's gate
// phases and convergence check (refresh-before-reuse, no double-spawn).
//
// The engine receives the FULL wf.Stop so convergence runs through
// converge.Converge (which honors every stop shape), and the per-iteration Signals
// closure fills HumanApproved from the resolved approval signal (reusing
// gates.go's humanApproved). For a conjunction/external stop this changes nothing
// — Converge delegates to Evaluate(all_of) and HumanApproved is irrelevant — but
// it makes the loop's convergence check honest even if a human_gate ever reached
// it (depth-two defense; cmdEvolve already refuses one up front).
//
// costSink threads the SAME tracer execLoop already owns into the agent executor, so
// a real claude phase's billed cost lands as a `kind:"agent"` cost event — now ALSO
// stamped with the routed model — interleaved (Seq-ordered) with the per-iteration events
// in trace.jsonl. It does NOT go through checkpointHook: cost is per-PHASE (emitted inside
// RunFrom when a phase bills), not per-iteration, so the iteration-event assertions are
// untouched.
func buildLoop(wf asset.Workflow, o runOpts, maxIter int, logln func(string), costSink func(phase, model string, usd float64)) orchestrator.LoopEngine {
	probe := &loopProbe{root: o.root}
	// The Engine — with its four prompt/feedback ledgers (gate verdicts, feeds_forward
	// output, reviewer verdicts driving loop-back, and REQUEST_CHANGES findings) — is built
	// by the SAME buildRunEngine `forge run` uses, so the two paths never drift. The only
	// evolve-specific seam is the RunGate: a per-iteration refreshing probe (each iteration
	// re-measures gate state, so the reviewer always sees the LATEST). The ledgers live for
	// the whole loop and update in place across iterations — the right converging-loop
	// semantics (latest gate state, latest plan, latest verdict).
	eng := buildRunEngine(wf, o, logln, costSink,
		func(name string) gate.Result { return resolveGate(o.root, name, probe.refresh()) },
		mode.Effective(o.mode, resolveLifecycle(o)))
	approved := humanApproved(o.root, wf.Stage, o.approved)
	return orchestrator.NewLoopEngine(
		eng, wf.Stop,
		func() converge.Signals { return gatherSignals(o.root, wf, probe.current(), approved) },
		maxIter, 2, logln)
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

// resumeStart resolves the loop's first iteration and seed completion. Without
// --resume it is a fresh run (0, -1.0): the engine begins at iteration 1 with the
// -1.0 sentinel. With --resume it loads the checkpoint and continues at
// cp.Iteration+1, seeding prev with the persisted completion so the stale/tripwire
// math is continuous. A MALFORMED checkpoint is a hard error (honesty-first):
// resuming must never silently degrade to a from-scratch rerun. A MISSING
// checkpoint with --resume is reported but tolerated as a fresh start.
func resumeStart(root string, resume bool) (start int, prev float64, err error) {
	if !resume {
		return 0, -1.0, nil
	}
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil {
		return 0, 0, fmt.Errorf("--resume: malformed checkpoint at %s: %w", checkpointPath(root), err)
	}
	if !found {
		fmt.Fprintf(os.Stderr, "forge evolve: --resume found no checkpoint at %s; starting fresh\n", checkpointPath(root))
		return 0, -1.0, nil
	}
	fmt.Printf("forge evolve: resuming from iteration %d (roadmap=%.0f%%, last reason: %s)\n",
		cp.Iteration+1, cp.RoadmapCompletion*100, cp.Reason)
	return cp.Iteration + 1, cp.RoadmapCompletion, nil
}

// checkpointHook builds the loop's post-measurement OnIteration callback: after
// each round's work AND its measurement, it persists a checkpoint, appends the
// round's trajectory to memory, and emits an iteration trace event carrying the
// iteration's measured wall-clock duration (durationMs from the loop) — the value
// scorecard p95_latency reads, so the trace records the iteration's real cost
// instead of a misleading 0. The snapshot time is injected here (main owns the
// clock; persist/trace/memory don't).
// Fail-LOUD-and-continue (deliberately NOT fail-closed): a checkpoint (or memory)
// write failure is made loudly visible — a stderr WARNING plus the trace status
// flipped to "checkpoint-write-failed" — but the loop keeps running. For a 24h run
// that is the correct trade: a transient disk hiccup must not abort hours of work,
// and the failure is never SWALLOWED, so we never pretend recovery state is durable
// when it is not. Contrast openTracer, which DOES fail-closed: losing the audit
// trail entirely is the blind spot trace prevents.
func checkpointHook(o runOpts, wf asset.Workflow, tracer *trace.Tracer, logln func(string)) func(int, converge.Signals, int64) {
	return func(i int, sig converge.Signals, durationMs int64) {
		cp := persist.Checkpoint{
			Workflow: wf.Stage, Mode: o.mode, Iteration: i,
			RoadmapCompletion: sig.RoadmapCompletion, GatesGreen: sig.GatesGreen,
			Reason: "iteration complete", UpdatedAtUnix: time.Now().Unix(),
		}
		status, detail := "ok", checkpointDetail(sig)
		if err := persist.Save(checkpointPath(o.root), cp); err != nil {
			status = "checkpoint-write-failed"
			detail = err.Error()
			logln(fmt.Sprintf("forge evolve: WARNING checkpoint write failed (recovery state NOT durable): %v", err))
		}
		recordMemory(o.root, wf, i, sig, logln)
		emitTrace(tracer, trace.Event{Kind: "iteration", Name: fmt.Sprintf("%d", i), Status: status, DurationMs: durationMs, Detail: detail}, logln)
	}
}

// recordMemory appends this iteration's trajectory to the cross-session store so a
// later round (or --resume) reads where the loop stood. HONESTY: under v1 evolve the
// executor defaults to dry-run, so the only thing this round TRULY produced is its
// trajectory / convergence signal (roadmap %, gate state) — real, and what we record
// (KindLesson, topic=stage). It is NOT a semantic gap/decision a real agent found:
// those exist only once the loop fires a real agent (--executor=command
// --agent-cmd=claude); a later wave will parse real-agent output into gap/decision
// entries (today we record only the dry-run run trajectory). We do not fabricate
// findings the dry loop never made. Fail-LOUD-and-continue: a failed append is
// warned, never aborts (enrichment, not correctness).
func recordMemory(root string, wf asset.Workflow, i int, sig converge.Signals, logln func(string)) {
	e := memory.Entry{
		Kind: memory.KindLesson, Topic: wf.Stage, Iteration: i, CreatedAtUnix: time.Now().Unix(),
		Detail: fmt.Sprintf("iter %d: roadmap=%.0f%%, gates_green=%v (dry-run trajectory)", i, sig.RoadmapCompletion*100, sig.GatesGreen),
	}
	if err := memory.Append(memoryPath(root), e); err != nil {
		logln(fmt.Sprintf("forge evolve: WARNING memory append failed (trajectory NOT recorded): %v", err))
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

// openTracer creates <root>/.forge and returns a Tracer APPENDING JSONL to
// trace.jsonl (append, not truncate, lets a --resume continue the same audit
// trail), plus a closer. A dir/open failure is returned (fail-closed) — the loop
// must not run blind on observability.
func openTracer(root string) (*trace.Tracer, func(), error) {
	if err := os.MkdirAll(forgeDir(root), 0o755); err != nil {
		return nil, func() {}, fmt.Errorf("create .forge dir: %w", err)
	}
	f, err := os.OpenFile(filepath.Join(forgeDir(root), "trace.jsonl"),
		os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o644)
	if err != nil {
		return nil, func() {}, fmt.Errorf("open trace file: %w", err)
	}
	return trace.NewTracer(f), func() { f.Close() }, nil
}
