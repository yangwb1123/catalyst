package main

import (
	"context"
	"fmt"
	"forgeos/forge-core/internal/routing"
	"os"
	"path/filepath"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/runlock"
	"forgeos/forge-core/internal/statefs"
	"forgeos/forge-core/internal/trace"
)

// acquireRunLock claims the process-level advisory lock on root's .forge/
// (see internal/runlock) so two concurrent `forge run`/`forge evolve`
// invocations against the same root can never race on shared .forge/ state.
// On contention (or any other Acquire failure) it prints an actionable error
// and returns nil — callers must treat a nil return as "exit 1", never retry.
// cmd names the calling command ("forge run"/"forge evolve") for the error
// prefix, matching every other top-level error message in this package.
func acquireRunLock(root, cmd string) *runlock.Lock {
	lock, err := runlock.Acquire(root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "%s: %v\n", cmd, err)
		return nil
	}
	if err := rejectTrackedForgeControlState(root); err != nil {
		fmt.Fprintf(os.Stderr, "%s: %v\n", cmd, err)
		_ = lock.Release()
		return nil
	}
	return lock
}

func acquireRunLockForOptions(o runOpts, cmd string) *runlock.Lock {
	lock := acquireRunLock(o.root, cmd)
	if lock == nil {
		return nil
	}
	if err := validateFrozenProjectSelectors(o); err != nil {
		fmt.Fprintf(os.Stderr, "%s: %v\n", cmd, err)
		_ = lock.Release()
		return nil
	}
	return lock
}

// stampRunID sets t's process-correlation RunID (internal/runlock.NewRunID,
// trace.Tracer.RunID) so every trace.jsonl line this process emits can be
// attributed to this run. Called once by openTracer, the single shared
// constructor for both `forge run` (via openRunResources) and `forge evolve`.
func stampRunID(t *trace.Tracer) {
	t.RunID = runlock.NewRunID()
}

// openTracer creates the shared, permission-hardened append tracer. Callers
// already hold run.lock, so trace rotation cannot race another Forge process.
func openTracer(root string) (*trace.Tracer, func(), error) {
	if err := statefs.EnsurePrivateDir(forgeDir(root)); err != nil {
		return nil, func() {}, fmt.Errorf("secure .forge dir: %w", err)
	}
	tp := filepath.Join(forgeDir(root), "trace.jsonl")
	const maxTraceBytes int64 = 10 << 20
	st, present, err := statefs.InspectRegular(tp)
	if err != nil {
		return nil, func() {}, fmt.Errorf("secure trace file: %w", err)
	}
	if present {
		if st.Size() > maxTraceBytes {
			if _, _, err := statefs.InspectRegular(tp + ".1"); err != nil {
				return nil, func() {}, fmt.Errorf("secure trace backup: %w", err)
			}
			if err := os.Rename(tp, tp+".1"); err != nil {
				return nil, func() {}, fmt.Errorf("rotate trace file: %w", err)
			}
		}
	}
	f, err := statefs.OpenRegular(tp, os.O_WRONLY|os.O_CREATE|os.O_APPEND, 0o600)
	if err != nil {
		return nil, func() {}, fmt.Errorf("open trace file: %w", err)
	}
	t := trace.NewTracer(f)
	stampRunID(t)
	return t, func() { _ = f.Close() }, nil
}

type stageHostBoundary struct {
	releaseStage    bool
	proposalStage   bool
	hostCommands    bool
	autoRisk        string
	autoRiskReasons []string
	autoDims        map[string]float64
	autoDimsReasons []string
	probe           *runProbe
	runGate         func(string) gate.Result
}

// resolveStageHostBoundary decides how this stage's gates run: restricted
// (release/proposal) stages get a fail-closed runner that never spawns; host
// stages get the live probe. ctx is the invocation's signal-aware context and
// opts the resolved gate options — both ride the probe via closure capture so
// every live spawn (ProbeAll + the complexity/arch gates ResolveGateWith
// runs) is bounded and Ctrl-C reaches the process group, with ZERO signature
// change to orchestrator.Engine.RunGate.
func resolveStageHostBoundary(ctx context.Context, wf asset.Workflow, o runOpts, lifecycle string, logln func(string)) stageHostBoundary {
	boundary := stageHostBoundary{
		releaseStage:  releaseApprovalStage(wf.Stage),
		proposalStage: proposalOnlyEvolve(wf, o, lifecycle),
	}
	boundary.hostCommands = !boundary.releaseStage && !boundary.proposalStage
	boundary.runGate = func(name string) gate.Result {
		return gate.Result{
			Status: gate.StatusFail,
			Output: fmt.Sprintf("restricted workflow unexpectedly requested harness gate %q", name),
		}
	}
	if !boundary.hostCommands {
		return boundary
	}
	boundary.autoRisk, boundary.autoRiskReasons = resolveAutoRisk(o.root)
	logAutoRisk(logln, "forge run", boundary.autoRisk, boundary.autoRiskReasons)
	boundary.autoDims, boundary.autoDimsReasons = resolveAutoDims(o.root)
	boundary.probe = newRunProbe(ctx, o.root, o.gateOpts)
	boundary.runGate = boundary.probe.runGate
	return boundary
}

// proposalEvolveGateRunner builds the evolve loop's gate runner: a refreshing
// probe when the loop may run host commands, else the fail-closed restricted
// runner. ctx/opts ride the closure so live spawns are bounded and cancellable.
func proposalEvolveGateRunner(ctx context.Context, root string, probe *loopProbe, restricted bool, opts gate.Options) func(string) gate.Result {
	if !restricted {
		return func(name string) gate.Result {
			return gate.ResolveGateWith(ctx, root, name, probe.refresh(), opts)
		}
	}
	return func(name string) gate.Result {
		return gate.Result{
			Status: gate.StatusFail,
			Output: fmt.Sprintf("proposal-only workflow unexpectedly requested harness gate %q", name),
		}
	}
}

// wireGateTrace composes gate-ledger recording with trace emission.
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

func logRunBanner(wf asset.Workflow, o runOpts, lifecycle string, pol mode.Policy) {
	fmt.Printf("forge run: stage=%s mode=%s lifecycle=%s executor=%s gates=%v reviewer=%v discover=%s design=%s adr=%v review=%s build_halt=%v (%d phases)\n",
		wf.Stage, o.mode, lifecycle, o.executor, pol.Gates, pol.Reviewer,
		pol.DiscoverDepth, pol.DesignDepth, pol.ADR, pol.ReviewDepth, pol.BuildHalted(), len(wf.Phases))
}

func resolveAutoDims(root string) (map[string]float64, []string) {
	paths := gitChangedPaths(root)
	if len(paths) == 0 {
		return routing.FromChangedPaths(nil), nil
	}
	dims := routing.FromChangedPaths(paths)
	reasons := make([]string, 0, 3)
	for _, dim := range []string{
		"complexity", "context_size", "dependency_change",
		"business_impact", "risk",
	} {
		if dims[dim] > 0 {
			reasons = append(reasons, fmt.Sprintf("%s=%.2f", dim, dims[dim]))
		}
	}
	return dims, reasons
}
