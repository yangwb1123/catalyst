// chain.go — spine workflow chaining for `forge run --chain`.
//
// After a workflow converges MET and declares on_met.next_stage (conjunction)
// or on_approved.next_stage (human_gate), forge run --chain auto-advances to
// the next spine stage. This connects discover→design→review→build→deploy→evolve
// into a single autonomous spine run.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/statefs"
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

const chainStateFormat = "forgeos.chain-state.v2"

// chainState is the versioned durable chain cursor and diagnostic snapshot.
type chainState struct {
	Format          string   `json:"_format"`
	RunID           string   `json:"run_id,omitempty"`
	Status          string   `json:"status"`
	EntryStage      string   `json:"entry_stage,omitempty"`
	CurrentStage    string   `json:"current_stage,omitempty"`
	CompletedStages []string `json:"completed_stages,omitempty"`
	Mode            string   `json:"mode,omitempty"`
	Lifecycle       string   `json:"lifecycle,omitempty"`
	Reason          string   `json:"reason,omitempty"`
	AgentCalls      int      `json:"agent_calls,omitempty"`
	MaxAgentCalls   int      `json:"max_agent_calls,omitempty"`
	MaxChainStages  int      `json:"max_chain_stages"`
	SpentUsdMicros  int64    `json:"spent_usd_micros,omitempty"`
	BudgetCapMicros int64    `json:"budget_cap_micros,omitempty"`
	UpdatedAtUnix   int64    `json:"updated_at_unix"`
}

func chainStatePath(root string) string {
	return filepath.Join(forgeDir(root), "chain-state.json")
}

func saveChainState(root string, state chainState) error {
	state.Format, state.UpdatedAtUnix = chainStateFormat, time.Now().Unix()
	data, err := json.MarshalIndent(state, "", "  ")
	if err != nil {
		return fmt.Errorf("encode chain state: %w", err)
	}
	path := chainStatePath(root)
	if err := statefs.EnsurePrivateDir(filepath.Dir(path)); err != nil {
		return fmt.Errorf("secure chain state directory: %w", err)
	}
	if err := statefs.RemoveRegular(path + ".tmp"); err != nil {
		return fmt.Errorf("reject legacy chain state temp: %w", err)
	}
	if err := statefs.AtomicWrite(path, append(data, '\n'), 0o600); err != nil {
		return fmt.Errorf("commit chain state: %w", err)
	}
	return nil
}

// persistChainState is the runtime persistence seam. Tests replace it to model
// an interrupted atomic commit while direct state fixtures keep using
// saveChainState.
var persistChainState = saveChainState

func loadChainState(root string) (chainState, bool, error) {
	data, found, err := statefs.ReadRegular(chainStatePath(root), 1<<20)
	if err != nil {
		return chainState{}, false, fmt.Errorf("read chain state: %w", err)
	}
	if !found {
		return chainState{}, false, nil
	}
	var state chainState
	if err := json.Unmarshal(data, &state); err != nil {
		return chainState{}, true, fmt.Errorf("decode chain state: %w", err)
	}
	if state.Format != chainStateFormat {
		return state, true, fmt.Errorf("unsupported chain state format %q (want %q)", state.Format, chainStateFormat)
	}
	return state, true, nil
}

func (s *chainState) complete(stage string) {
	for _, done := range s.CompletedStages {
		if done == stage {
			return
		}
	}
	s.CompletedStages = append(s.CompletedStages, stage)
}

func validateResumableChainState(state chainState) error {
	switch {
	case state.RunID == "":
		return fmt.Errorf("persisted waiting chain has no run_id")
	case state.EntryStage == "" || state.CurrentStage == "":
		return fmt.Errorf("persisted waiting chain lacks entry/current stage")
	case state.Mode == "" || state.Lifecycle == "":
		return fmt.Errorf("persisted waiting chain lacks mode/lifecycle policy")
	case state.AgentCalls < 0 || state.MaxAgentCalls < 0:
		return fmt.Errorf("persisted waiting chain has negative agent-call state")
	case state.MaxChainStages < 1:
		return fmt.Errorf("persisted waiting chain has invalid max_chain_stages")
	case state.SpentUsdMicros < 0 || state.BudgetCapMicros < 0:
		return fmt.Errorf("persisted waiting chain has negative budget state")
	}
	return nil
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
		_, rejected, code := execOneStage(ctx, first, o, logln, lifecycle, tracer, budget, nil)
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
		Mode: o.mode, Lifecycle: lifecycle,
		MaxAgentCalls: o.maxAgentCalls, MaxChainStages: o.maxChainStages,
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
		resumingApproval: resume != nil,
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
	state := chainState{RunID: tracer.RunID, Status: "halted", CurrentStage: first.Stage, Reason: reason}
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

func (r *chainRuntime) runOne(wf asset.Workflow, stageIndex int) (asset.Workflow, int, bool) {
	if err := r.guard.Enter(wf.Stage); err != nil {
		return asset.Workflow{}, r.fail(wf.Stage, "chain traversal rejected: "+err.Error(), 1), true
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
	met, rejected, code := execOneStage(r.ctx, wf, stageOpts, r.logln, r.lifecycle, r.tracer, r.budget, r.calls.charge)
	if code != 0 {
		if rejected {
			return asset.Workflow{}, r.rejectedReworkFailed(wf, code), true
		}
		return asset.Workflow{}, r.fail(wf.Stage, fmt.Sprintf("stage execution failed with exit code %d", code), code), true
	}
	if !met {
		return asset.Workflow{}, r.notConverged(wf, rejected), true
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
	next, err := loadWorkflowForExecution(r.opts.root, nextName, r.opts.mode, r.lifecycle)
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
	r.state.Mode, r.state.Lifecycle = r.opts.mode, r.lifecycle
	r.state.AgentCalls = r.calls.count()
	r.state.SpentUsdMicros = r.budget.SpentUsdMicros()
	r.state.BudgetCapMicros = r.budget.CapUsdMicros()
	r.state.MaxAgentCalls = r.opts.maxAgentCalls
	r.state.MaxChainStages = r.opts.maxChainStages
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
