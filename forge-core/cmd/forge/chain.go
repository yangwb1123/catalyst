// chain.go — spine workflow chaining for `forge run --chain`.
//
// After a workflow converges MET and declares on_met.next_stage (conjunction)
// or on_approved.next_stage (human_gate), forge run --chain auto-advances to
// the next spine stage. This connects discover→design→review→build→deploy→evolve
// into a single autonomous spine run.
package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"sync"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/trace"
)

const (
	defaultMaxChainStages = 8
	// exitChainIncomplete distinguishes a correctly-held convergence/human gate
	// from both success (0) and execution failure (1) for automation.
	exitChainIncomplete = 3
)

// chainNextStage returns the next spine stage name from a stop condition, or
// "" if the stop does not declare a next stage. Conjunction stops use on_met
// (discover.yml→design, build.yml→evolve); human_gate stops use on_approved
// (design.yml→review, review.yml→build).
func chainNextStage(stop asset.StopCondition) string {
	if stop.OnMet != nil && stop.OnMet.NextStage != "" {
		return stop.OnMet.NextStage
	}
	if stop.OnApproved.NextStage != "" {
		return stop.OnApproved.NextStage
	}
	return ""
}

// isAutoChainable reports whether a workflow can be auto-advanced through the
// spine. Conjunction and human_gate stops are chainable; external stops
// (evolve loop) are not — they are standing loops, not spine transitions.
func isAutoChainable(stop asset.StopCondition) bool {
	return stop.Type == "conjunction" || stop.Type == "human_gate"
}

// chainGuard bounds one --chain traversal and rejects repeated stages. Workflow
// assets are configuration, but a typo such as build→design or a self-loop must
// never turn a CLI invocation into an unbounded autonomous process.
type chainGuard struct {
	seen map[string]struct{}
	max  int
}

func newChainGuard(max int) *chainGuard {
	if max < 1 {
		max = defaultMaxChainStages
	}
	return &chainGuard{seen: make(map[string]struct{}), max: max}
}

// Enter records a stage before it executes. The first duplicate is a cycle; an
// otherwise-unique traversal beyond max is also rejected.
func (g *chainGuard) Enter(stage string) error {
	if _, exists := g.seen[stage]; exists {
		return fmt.Errorf("cycle detected at stage %q", stage)
	}
	if len(g.seen) >= g.max {
		return fmt.Errorf("stage limit %d exceeded before %q", g.max, stage)
	}
	g.seen[stage] = struct{}{}
	return nil
}

func (g *chainGuard) seed(stages []string) error {
	for _, stage := range stages {
		if err := g.Enter(stage); err != nil {
			return err
		}
	}
	return nil
}

// chainPolicyAllows enforces mode-level stage boundaries independently of an
// asset's next_stage declaration. In particular, cto is an analysis-only mode:
// even an approved Design/Review asset cannot make it enter Build.
func chainPolicyAllows(policy mode.Policy, nextStage string) (bool, string) {
	switch nextStage {
	case "build", "deploy", "rollback":
		if policy.BuildHalted() {
			return false, "effective mode policy requires workflow_depth.build=halt; action stages are disabled"
		}
	}
	return true, ""
}

// chainAgentCounter is shared by every Engine in one chain. charge is injected
// into orchestrator.Engine as a closure, avoiding a CLI-specific exported type.
type chainAgentCounter struct {
	mu   sync.Mutex
	used int
}

func (c *chainAgentCounter) charge(max int) (int, bool) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if max > 0 && c.used >= max {
		return c.used, false
	}
	c.used++
	return c.used, true
}

func (c *chainAgentCounter) count() int {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.used
}

func (c *chainAgentCounter) seed(used int) {
	c.mu.Lock()
	defer c.mu.Unlock()
	c.used = used
}

func (s *chainState) complete(stage string) {
	for _, done := range s.CompletedStages {
		if done == stage {
			return
		}
	}
	s.CompletedStages = append(s.CompletedStages, stage)
}

func sameStageSequence(got, want []string) bool {
	if len(got) != len(want) {
		return false
	}
	for i := range got {
		if got[i] != want[i] {
			return false
		}
	}
	return true
}

type chainRuntime struct {
	ctx              context.Context
	opts             runOpts
	logln            func(string)
	lifecycle        string
	tracer           *trace.Tracer
	budget           *runBudget
	guard            *chainGuard
	calls            *chainAgentCounter
	state            chainState
	resumingApproval bool
}

type chainStepResult struct {
	next asset.Workflow
	code int
	done bool
}

func runStageChain(ctx context.Context, first asset.Workflow, o runOpts, logln func(string), lifecycle string, tracer *trace.Tracer, budget *runBudget, resume *chainState) int {
	if code, halted := haltInitialStageByPolicy(first, o, lifecycle, tracer, logln); halted {
		return code
	}
	if !o.chain {
		_, rejected, code, _ := execOneStage(ctx, first, o, logln, lifecycle, tracer, budget, nil)
		if code == 0 {
			if err := consumeRejectionAfterSuccess(first, o.root, rejected, logln); err != nil {
				fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
				return 1
			}
		}
		return code
	}
	state := chainState{
		RunID: tracer.RunID, EntryStage: first.Stage,
		Mode: o.mode, Lifecycle: lifecycle, Materiality: o.materiality,
		WorkflowDigests: make(map[string]string),
		MaxAgentCalls:   o.maxAgentCalls, MaxChainStages: o.maxChainStages,
		BudgetCapMicros: budget.CapUsdMicros(),
	}
	if resume != nil {
		state = *resume
	}
	guard := newChainGuard(o.maxChainStages)
	if err := guard.seed(state.CompletedStages); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: invalid persisted chain traversal: %v\n", err)
		return 1
	}
	calls := &chainAgentCounter{}
	calls.seed(state.AgentCalls)
	r := &chainRuntime{
		ctx: ctx, opts: o, logln: logln, lifecycle: lifecycle,
		tracer: tracer, budget: budget, guard: guard, calls: calls, state: state,
		resumingApproval: resume != nil && resume.Status == "waiting_approval",
	}
	return r.run(first, len(state.CompletedStages))
}

// haltInitialStageByPolicy closes the direct-entry escape hatch: the same mode
// boundary that guards chain transitions must also guard `forge run build`
// when Build is the first requested stage.
func haltInitialStageByPolicy(first asset.Workflow, o runOpts, lifecycle string, tracer *trace.Tracer, logln func(string)) (int, bool) {
	allowed, reason := chainPolicyAllows(mode.Effective(o.mode, lifecycle), first.Stage)
	if allowed {
		return 0, false
	}
	state := chainState{RunID: tracer.RunID, Status: "halted", CurrentStage: first.Stage,
		Materiality: o.materiality, WorkflowDigests: make(map[string]string), Reason: reason}
	bound := first.OutputBindingContract == asset.OutputBindingContractLocalDigestV1
	if err := state.bindWorkflow(first.Stage, checkpointWorkflowDigest(first), bound); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: bind halted workflow: %v\n", err)
		return 1, true
	}
	if err := persistChainState(o.root, state); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: persist policy halt: %v\n", err)
		return 1, true
	}
	logln(fmt.Sprintf("forge run: stage %s halted before execution: %s", first.Stage, reason))
	return 0, true
}

func (r *chainRuntime) run(first asset.Workflow, startIndex int) int {
	wf := first
	for stageIndex := startIndex; ; stageIndex++ {
		next, code, done := r.runOne(wf, stageIndex)
		if done {
			return code
		}
		wf = next
	}
}

func loadBoundChainWorkflow(root, stage string, state chainState) (asset.Workflow, error) {
	wf, err := loadWorkflowNativeOnly(root, stage)
	if err != nil {
		return asset.Workflow{}, fmt.Errorf("load persisted chain stage %q: %w", stage, err)
	}
	if err := validatePersistedWorkflowDigest(state, wf.Stage, checkpointWorkflowDigest(wf)); err != nil {
		return asset.Workflow{}, err
	}
	if wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1 && state.Format != chainStateFormat {
		return asset.Workflow{}, fmt.Errorf(
			"chain state format %q is diagnostic-only for bound workflow %q; resume requires %q",
			state.Format, stage, chainStateFormat,
		)
	}
	if state.Format == chainStateFormat &&
		stageIn(state.BoundStages, stage) != (wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1) {
		return asset.Workflow{}, fmt.Errorf("persisted chain selector binding differs for stage %q", stage)
	}
	return wf, nil
}

func (r *chainRuntime) runOne(wf asset.Workflow, stageIndex int) (asset.Workflow, int, bool) {
	if err := r.guard.Enter(wf.Stage); err != nil {
		return asset.Workflow{}, r.fail(wf.Stage, "chain traversal rejected: "+err.Error(), 1), true
	}
	bound := wf.OutputBindingContract == asset.OutputBindingContractLocalDigestV1
	if err := r.state.bindWorkflow(wf.Stage, checkpointWorkflowDigest(wf), bound); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: chain workflow binding failed: %v\n", err)
		return asset.Workflow{}, 1, true
	}
	stageOpts := r.opts
	if stageIndex > 0 {
		stageOpts.approved = false
	}
	if resumed, handled := r.resumeApprovalDecision(wf, stageOpts); handled {
		return resumed.next, resumed.code, resumed.done
	}
	// Do not overwrite a durable wait before its decision is inspected.
	if err := r.persist("running", wf.Stage, "stage execution in progress"); err != nil {
		return asset.Workflow{}, r.persistenceFailure(err), true
	}
	if wf.Stage == "evolve" && wf.Stop.Type == "external" {
		result := r.runExternalEvolve(wf, stageOpts)
		return result.next, result.code, result.done
	}
	met, rejected, code, validateCompletion := execOneStage(r.ctx, wf, stageOpts, r.logln, r.lifecycle, r.tracer, r.budget, r.calls.charge)
	if code != 0 {
		if rejected {
			r.state.dropStageRecovery(wf.Stage)
			return asset.Workflow{}, r.rejectedReworkFailed(wf, code), true
		}
		return asset.Workflow{}, r.fail(wf.Stage, fmt.Sprintf("stage execution failed with exit code %d", code), code), true
	}
	if err := r.bindStageRecoveryForResult(wf, met); err != nil {
		return asset.Workflow{}, r.fail(wf.Stage, "durable output binding: "+err.Error(), 1), true
	}
	if !met {
		return asset.Workflow{}, r.notConverged(wf, rejected), true
	}
	if err := runStageCompletionValidator(validateCompletion); err != nil {
		return asset.Workflow{}, r.fail(wf.Stage, "chain completion freshness: "+err.Error(), 1), true
	}
	r.state.complete(wf.Stage)
	next, nextCode, done := r.advance(wf)
	if nextCode == 0 {
		if err := consumeRejectionAfterSuccess(wf, r.opts.root, rejected, r.logln); err != nil {
			return asset.Workflow{}, r.persistenceFailure(err), true
		}
	}
	return next, nextCode, done
}

// bindStageRecoveryForResult keeps chain recovery references exact. A held
// human gate is resumable and therefore needs its accepted output binding;
// an unmet conjunction is diagnostic-only and must not look completed.
func (r *chainRuntime) bindStageRecoveryForResult(wf asset.Workflow, met bool) error {
	if err := r.bindStageRecovery(wf); err != nil {
		return err
	}
	if !met && !converge.IsHumanGate(wf.Stop) {
		r.state.dropStageRecovery(wf.Stage)
	}
	return nil
}

func (state *chainState) dropStageRecovery(stage string) {
	state.ensureRecoveryMaps()
	for key := range state.PhaseReceipts {
		if strings.HasPrefix(key, stage+"/") {
			delete(state.PhaseReceipts, key)
		}
	}
	delete(state.StageReceipts, stage)
	delete(state.ApprovalContexts, stage)
}

func (r *chainRuntime) resumeApprovalDecision(wf asset.Workflow, stageOpts runOpts) (chainStepResult, bool) {
	if !r.resumingApproval {
		return chainStepResult{}, false
	}
	r.resumingApproval = false
	rejected, err := rejectionMarkerExists(r.opts.root, wf.Stage)
	if err != nil {
		return chainStepResult{code: r.fail(wf.Stage, err.Error(), 1), done: true}, true
	}
	if rejected {
		r.state.dropStageRecovery(wf.Stage)
		return chainStepResult{}, false
	}
	met := humanApproved(r.opts.root, wf.Stage, stageOpts.approved)
	reportHumanGate(wf, met)
	if !met {
		return chainStepResult{code: r.notConverged(wf), done: true}, true
	}
	r.state.complete(wf.Stage)
	next, code, done := r.advance(wf)
	return chainStepResult{next: next, code: code, done: done}, true
}

func (r *chainRuntime) runExternalEvolve(wf asset.Workflow, stageOpts runOpts) chainStepResult {
	stageOpts.lifecycle = r.lifecycle
	code := execChainEvolve(r.ctx, wf, stageOpts, r.logln, r.tracer, r.budget, r.calls)
	if code != 0 {
		return chainStepResult{
			code: r.fail(wf.Stage, fmt.Sprintf("evolve loop stopped with exit code %d", code), code),
			done: true,
		}
	}
	if err := r.bindStageRecovery(wf); err != nil {
		return chainStepResult{code: r.fail(wf.Stage, "durable output binding: "+err.Error(), 1), done: true}
	}
	r.state.complete(wf.Stage)
	if err := r.persist("completed", "", "evolve loop reached a clean external stop"); err != nil {
		return chainStepResult{code: r.persistenceFailure(err), done: true}
	}
	return chainStepResult{done: true}
}

func (r *chainRuntime) advance(wf asset.Workflow) (asset.Workflow, int, bool) {
	if err := r.persist("running", wf.Stage, "stage converged; resolving next transition"); err != nil {
		return asset.Workflow{}, r.persistenceFailure(err), true
	}
	nextName := chainNextStage(wf.Stop)
	if nextName == "" || !isAutoChainable(wf.Stop) {
		if err := r.persist("completed", "", "no further auto-chainable stage"); err != nil {
			return asset.Workflow{}, r.persistenceFailure(err), true
		}
		return asset.Workflow{}, 0, true
	}
	if allowed, reason := chainPolicyAllows(mode.Effective(r.opts.mode, r.lifecycle), nextName); !allowed {
		r.logln(fmt.Sprintf("forge run: chain stopped at stage=%s before %s (%s)", wf.Stage, nextName, reason))
		if err := r.persist("halted", nextName, reason); err != nil {
			return asset.Workflow{}, r.persistenceFailure(err), true
		}
		return asset.Workflow{}, 0, true
	}
	r.logln(fmt.Sprintf("forge run: chain advancing stage=%s → %s", wf.Stage, nextName))
	next, err := loadWorkflowForExecution(r.opts.root, nextName, r.opts.mode, r.lifecycle, r.opts.materiality)
	if err != nil {
		return asset.Workflow{}, r.fail(nextName, fmt.Sprintf("cannot load next workflow %q: %v", nextName, err), 1), true
	}
	return next, 0, false
}

func (r *chainRuntime) notConverged(wf asset.Workflow, rejected ...bool) int {
	status := "not_converged"
	reason := "stop condition not met; chain did not advance"
	if converge.IsHumanGate(wf.Stop) {
		status = "waiting_approval"
		reason = fmt.Sprintf("awaiting approval for %s; run `forge approve %s --root %s` and rerun the chain", wf.Stage, wf.Stage, r.opts.root)
		r.logln("forge run: chain waiting for human approval at stage=" + wf.Stage)
		r.logln("  " + reason)
	} else {
		r.logln(fmt.Sprintf("forge run: chain stopped at stage=%s (NOT MET — not advancing)", wf.Stage))
	}
	if err := r.persist(status, wf.Stage, reason); err != nil {
		return r.persistenceFailure(err)
	}
	if len(rejected) > 0 {
		if err := consumeRejectionAfterSuccess(wf, r.opts.root, rejected[0], r.logln); err != nil {
			return r.persistenceFailure(err)
		}
	}
	return exitChainIncomplete
}

func (r *chainRuntime) rejectedReworkFailed(wf asset.Workflow, code int) int {
	reason := fmt.Sprintf("rejected %s rework failed with exit code %d; marker retained for retry", wf.Stage, code)
	if err := r.persist("waiting_approval", wf.Stage, reason); err != nil {
		return r.persistenceFailure(err)
	}
	fmt.Fprintf(os.Stderr, "forge run: %s\n", reason)
	return code
}

func (r *chainRuntime) persist(status, current, reason string) error {
	r.state.Status, r.state.CurrentStage, r.state.Reason = status, current, reason
	r.state.Mode, r.state.Lifecycle, r.state.Materiality = r.opts.mode, r.lifecycle, r.opts.materiality
	r.state.AgentCalls = r.calls.count()
	r.state.SpentUsdMicros = r.budget.SpentUsdMicros()
	r.state.BudgetCapMicros = r.budget.CapUsdMicros()
	r.state.MaxAgentCalls = r.opts.maxAgentCalls
	r.state.MaxChainStages = r.opts.maxChainStages
	if err := r.refreshReceiptHead(); err != nil {
		return err
	}
	r.state.Format = chainStateFormat
	if err := validateBoundChainRecovery(r.opts.root, r.state); err != nil {
		return fmt.Errorf("validate chain recovery before commit: %w", err)
	}
	return persistChainState(r.opts.root, r.state)
}

func (r *chainRuntime) fail(stage, reason string, code int) int {
	if err := r.persist("failed", stage, reason); err != nil {
		fmt.Fprintf(os.Stderr, "forge run: %s; additionally could not persist chain failure: %v\n", reason, err)
		return 1
	}
	fmt.Fprintf(os.Stderr, "forge run: chain failed at stage=%s: %s\n", stage, reason)
	return code
}

func (r *chainRuntime) persistenceFailure(err error) int {
	fmt.Fprintf(os.Stderr, "forge run: chain state persistence failed: %v\n", err)
	return 1
}
