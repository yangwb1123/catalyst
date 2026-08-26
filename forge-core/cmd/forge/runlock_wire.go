package main

import (
	"bytes"
	"context"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"forgeos/forge-core/internal/approvalcontext"
	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/orchestrator"
	"forgeos/forge-core/internal/routing"
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

func execEngine(ctx context.Context, first asset.Workflow, o runOpts) int {
	if err := normalizeRunMateriality(&o); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: materiality: %v\n", err)
		return 1
	}
	if err := validateMaterialityWorkflow(first, o); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	gateOpts, err := gate.ResolveOptions(gate.CLIInput{EnvTimeout: os.Getenv(gate.EnvTimeout)})
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return 1
	}
	o.gateOpts = gateOpts
	lock := acquireRunLockForOptions(o, "forge run")
	if lock == nil {
		return 1
	}
	defer func() { _ = lock.Release() }()
	return execLockedEngine(ctx, first, o)
}

func execLockedEngine(ctx context.Context, first asset.Workflow, o runOpts) int {
	logln := func(s string) { fmt.Println(s) }
	lifecycle := resolveLifecycle(o)
	first, resume, err := prepareChainResume(first, o)
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
		logln(fmt.Sprintf("forge run: resuming chain run_id=%s at stage=%s after completed=%v", resume.RunID, resume.CurrentStage, resume.CompletedStages))
	}
	return runStageChain(ctx, first, o, logln, lifecycle, tracer, budget, resume)
}

func execOneStage(ctx context.Context, wf asset.Workflow, o runOpts, logln func(string), lifecycle string, tracer *trace.Tracer, budget *runBudget, charge func(int) (int, bool)) (bool, bool, int, func() error) {
	if err := validateMaterialityWorkflow(wf, o); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return false, false, 1, nil
	}
	pol := materialityPolicy(wf, o, mode.Effective(o.mode, lifecycle))
	boundary := resolveStageHostBoundary(ctx, wf, o, lifecycle, logln)
	eng, verdicts, _, validateCompletion, _ := buildRunEngineWithPhaseOutput(wf, o, logln,
		costEmitter(tracer, logln), boundary.runGate, pol, budget,
		boundary.autoRisk, boundary.autoRiskReasons, boundary.autoDims, boundary.autoDimsReasons,
		newPhaseOutputLedger(), tracedEngineBuildOptions(tracer, logln)...)
	eng.ChargeAgentCall = charge
	if boundary.hostCommands {
		eng.Exec = runProbeExecutor{next: eng.Exec, probe: boundary.probe}
	}
	wireEngineTrace(&eng, tracer, logln)
	logRunBanner(wf, o, lifecycle, pol)
	start, rejected, err := resolveRejectionStartPhase(wf, o.root, logln)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return false, false, 1, validateCompletion
	}
	if err := runWorkflow(ctx, eng, wf, o, logln, start); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
		return false, rejected, 1, validateCompletion
	}
	met, rejected, code := finishOneStage(ctx, wf, o, lifecycle, boundary, verdicts, tracer, rejected, validateCompletion)
	return met, rejected, code, validateCompletion
}

func finishOneStage(ctx context.Context, wf asset.Workflow, o runOpts, lifecycle string, boundary stageHostBoundary, verdicts *verdictLedger, tracer *trace.Tracer, rejected bool, validateCompletion func() error) (bool, bool, int) {
	fmt.Printf("forge run: stage=%s workflow completed\n", wf.Stage)
	var probe, categories map[string]string
	var actualGates []string
	if boundary.hostCommands {
		probe, categories = boundary.probe.current()
		actualGates = boundary.probe.actualGates()
	}
	met := reportStageConvergence(ctx, o.gateOpts, wf, o.root, probe, categories, lifecycle, o.approved,
		verdicts, actualGates, boundary.proposalStage, boundary.releaseStage)
	if boundary.hostCommands && allowScorecardWindDown(wf, o) {
		windDownScorecardsForRun(wf, o, func(s string) { fmt.Println(s) }, 1, verdicts.wasReworked(), tracer.RunID)
	}
	if err := runStageCompletionValidator(validateCompletion); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: stage completion freshness: %v\n", err)
		return false, rejected, 1
	}
	return met, rejected, 0
}

func runStageCompletionValidator(validate func() error) error {
	if validate == nil {
		return nil
	}
	return validate()
}

func allowScorecardWindDown(wf asset.Workflow, opts runOpts) bool {
	return wf.OutputBindingContract != asset.OutputBindingContractLocalDigestV1
}

// stampRunID gives a fresh trace a run-correlation ID. Resume wiring replaces
// it with the checkpoint's durable ID before the first event is emitted.
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
		} else if err := validateTraceAppendFraming(tp, st.Size()); err != nil {
			return nil, func() {}, fmt.Errorf("secure trace framing: %w", err)
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
	fmt.Printf("forge run: stage=%s mode=%s lifecycle=%s materiality=%s strict_reviewer=%v executor=%s gates=%v reviewer=%v discover=%s design=%s adr=%v review=%s build_halt=%v (%d phases)\n",
		wf.Stage, o.mode, lifecycle, o.materiality, strictBuildReview(wf, o), o.executor, pol.Gates, pol.Reviewer,
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

func tryWriteBoundPositiveApproval(root, stage string) (int, bool) {
	bound, err := boundApprovalSelection(root, stage)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge approved: cannot establish approval binding: %v\n", err)
		return 1, true
	}
	if !bound {
		return 0, false
	}
	lock, err := runlock.Acquire(root)
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge approved: %v\n", err)
		return 1, true
	}
	defer func() { _ = lock.Release() }()
	if err = rejectTrackedForgeControlState(root); err == nil {
		err = installBoundPositiveApproval(root, stage, approvalActorHint(), time.Now())
	}
	if err != nil {
		fmt.Fprintf(os.Stderr, "forge approved: cannot install digest-bound approval: %v\n", err)
		return 1, true
	}
	path, _ := approvalMarkerPath(root, stage, ".approved")
	colorlessBoundApprovalNarration(stage, path)
	return 0, true
}

func boundApprovalSelection(root, stage string) (bool, error) {
	_, bound, err := loadBoundApprovalWorkflow(root, stage)
	return bound, err
}

func requiresBoundApproval(root, stage string) bool {
	if !approvalContextStage(stage) {
		return false
	}
	bound, err := boundApprovalSelection(root, stage)
	if err == nil {
		return bound
	}
	_, statErr := os.Lstat(filepath.Join(root, ".agent", "workflows", stage+".yml"))
	return statErr == nil || !os.IsNotExist(statErr)
}

func installBoundPositiveApproval(root, stage, actor string, now time.Time) error {
	verified, err := verifyBoundApprovalContext(root, stage)
	if err != nil {
		return err
	}
	if releaseApprovalStage(stage) {
		if err := verifyBoundReleaseValidationReceipt(root, stage); err != nil {
			return fmt.Errorf("positive release approval requires current v2 validation receipt: %w", err)
		}
	}
	marker := approvalcontext.PositiveMarkerFromContext(
		verified.Context, verified.ContextSHA256, actor, now.UnixMilli(),
	)
	if marker.CreatedAtUnixMS < verified.Context.CreatedAtUnixMS {
		return fmt.Errorf("positive approval marker time predates its context")
	}
	data, err := approvalcontext.CanonicalMarkerJSON(marker)
	if err != nil {
		return err
	}
	approved, err := approvalMarkerPath(root, stage, ".approved")
	if err != nil {
		return err
	}
	rejected, err := approvalMarkerPath(root, stage, ".rejected")
	if err != nil {
		return err
	}
	if err := installCanonicalPositiveMarker(forgeDir(root), approved, rejected, data); err != nil {
		return err
	}
	if err := verifyBoundPositiveMarker(root, stage); err != nil {
		return fmt.Errorf("installed positive marker did not remain current: %w", err)
	}
	return nil
}

func installCanonicalPositiveMarker(directory, approved, rejected string, data []byte) error {
	if err := statefs.EnsurePrivateDir(directory); err != nil {
		return fmt.Errorf("secure marker directory: %w", err)
	}
	for _, path := range []string{approved, rejected} {
		if _, _, err := statefs.InspectRegular(path); err != nil {
			return fmt.Errorf("secure marker %s: %w", path, err)
		}
	}
	if err := statefs.AtomicWrite(approved, data, 0o600); err != nil {
		return fmt.Errorf("install positive marker: %w", err)
	}
	if err := statefs.RemoveRegular(rejected); err != nil {
		return fmt.Errorf("positive marker installed but cannot supersede rejection: %w", err)
	}
	return persistApprovalMarkerRemoval(directory, statefs.SyncDir)
}

func persistApprovalMarkerRemoval(directory string, syncDir func(string) error) error {
	if err := syncDir(directory); err != nil {
		return fmt.Errorf("persist superseded rejection marker: %w", err)
	}
	return nil
}

func verifyBoundPositiveMarker(root, stage string) error {
	if !releaseRejectionMarkerAbsent(root, stage) {
		return fmt.Errorf("positive and negative approval markers conflict")
	}
	first, marker, err := readBoundPositiveMarker(root, stage)
	if err != nil {
		return err
	}
	verified, err := verifyBoundApprovalContext(root, stage)
	if err != nil {
		return err
	}
	if err := approvalcontext.ValidateMarkerContext(marker, verified.Context); err != nil {
		return err
	}
	if releaseApprovalStage(stage) {
		if err := verifyBoundReleaseValidationReceipt(root, stage); err != nil {
			return fmt.Errorf("positive release approval v2 receipt: %w", err)
		}
	}
	if marker.Stage != stage || marker.CreatedAtUnixMS < verified.Context.CreatedAtUnixMS {
		return fmt.Errorf("positive approval marker identity or time is invalid")
	}
	second, _, err := readBoundPositiveMarker(root, stage)
	if err != nil || !bytes.Equal(first, second) {
		return fmt.Errorf("positive approval marker changed while being verified")
	}
	if !releaseRejectionMarkerAbsent(root, stage) {
		return fmt.Errorf("positive and negative approval markers conflict")
	}
	return nil
}

func readBoundPositiveMarker(root, stage string) ([]byte, approvalcontext.PositiveMarker, error) {
	path, err := approvalMarkerPath(root, stage, ".approved")
	if err != nil {
		return nil, approvalcontext.PositiveMarker{}, err
	}
	info, present, err := statefs.InspectRegular(path)
	if err != nil {
		return nil, approvalcontext.PositiveMarker{}, err
	}
	if !present || info.Mode().Perm() != 0o600 {
		return nil, approvalcontext.PositiveMarker{}, fmt.Errorf("positive approval marker must be a private regular single-link file")
	}
	data, present, err := statefs.ReadRegularUnmodified(path, 64<<10)
	if err != nil {
		return nil, approvalcontext.PositiveMarker{}, fmt.Errorf("read positive approval marker: %w", err)
	}
	if !present {
		return nil, approvalcontext.PositiveMarker{}, fmt.Errorf("positive approval marker disappeared while being read")
	}
	marker, err := approvalcontext.DecodeCanonicalMarker(data)
	if err != nil {
		return nil, approvalcontext.PositiveMarker{}, err
	}
	return data, marker, nil
}

func humanApproved(root, stage string, flag bool) bool {
	if requiresBoundApproval(root, stage) {
		return validBoundApproval(root, stage)
	}
	if releaseApprovalStage(stage) {
		return validReleaseApproval(root, stage)
	}
	if flag {
		return true
	}
	present, err := markerExists(approvalPath(root, stage))
	return err == nil && present
}

func validBoundApproval(root, stage string) bool {
	return verifyBoundPositiveMarker(root, stage) == nil
}

func colorlessBoundApprovalNarration(stage, path string) {
	fmt.Printf("forge approved: stage %s (%s)\n", stage, path)
	fmt.Println("  Next: forge run <next-workflow> --chain (or manually)")
	fmt.Println("  Approval remains valid only while its exact receipt, workflow, source, policy, and artifacts remain current")
}
