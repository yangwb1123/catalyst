// Package orchestrator is forge-core's workflow runtime: it turns a declarative
// Workflow into a state machine that "runs itself" by stepping through phases
// in order. Two phase kinds matter — gate phases (required_gates non-empty),
// where it ENFORCES by invoking the real harness gates, and agent phases, where
// it delegates to an AgentExecutor.
//
// A red gate is NOT always fatal. build.yml's gate phases declare a DIRECTED
// loop-back (on_fail: {action: loop_back, target_phase: implementer}): when such
// a phase fails, the runtime jumps the state machine BACK to the named phase and
// re-runs forward to the failing gate, bounded by Engine.MaxLoopBack (fail-closed:
// when the budget is spent the run aborts). A gate phase WITHOUT an on_fail (or a
// zero MaxLoopBack budget) keeps the legacy behavior — the first red gate aborts.
//
// The executor is an interface so the runtime stays honest about what it can
// actually do today: the shipped DryRunExecutor only logs the routing decision
// (no LLM is invoked). Wiring a real Agent executor (Claude Agent SDK) behind
// this same interface is the future extension point. HONESTY on loop-back under a
// dry-run executor: re-running phases re-runs the DRY agent, which produces no new
// code, so the same red gate stays red and the budget is simply spent — the
// directed-jump STATE MACHINE is exercised and verifiable (a fake gate scripted to
// fail-then-pass proves the jump), but the loop-back's repair VALUE only
// materializes once a real agent (--executor=command) edits code between attempts.
package orchestrator

import (
	"errors"
	"fmt"
	"strings"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
	"forgeos/forge-core/internal/routing"
)

// AgentExecutor performs the agent action for one phase under a mode. A real
// implementation would drive an LLM agent; DryRunExecutor only narrates.
type AgentExecutor interface {
	Execute(p asset.Phase, mode string) error
}

// DryRunExecutor is the zero-LLM executor: it logs the resolved routing for a
// phase and returns nil. Log defaults to a no-op when nil, so the executor is
// safe to use without configuration.
type DryRunExecutor struct {
	Log func(string)
}

// Execute narrates the phase as "phase <name> -> agent <agent> (tier <tier>)",
// taking the tier from phaseTier so a workflow's per-phase model_tier override is
// honored (raise-only, never below the safety floor — see phaseTier).
func (d DryRunExecutor) Execute(p asset.Phase, mode string) error {
	tier := phaseTier(p, mode)
	d.logf("phase %s -> agent %s (tier %s)", p.Name, p.Agent, tier)
	return nil
}

// phaseTier resolves the model tier for a phase under a mode, honoring an
// OPTIONAL per-phase model_tier OVERRIDE authored in the workflow asset.
//
// The base is routing.TierFor(agent, mode) — the routed verdict, which already
// applies the non-negotiable Opus SAFETY FLOOR for judgement-only agents
// (architect/cto/reviewer) and the per-agent/mode floors. When the phase declares
// a model_tier, it is combined with the base via routing.Higher: the override can
// only RAISE the tier, never lower it below the floor. So a phase that writes
// model_tier: opus on a plain agent routes to Opus (override lifts), while a phase
// that writes model_tier: haiku on the reviewer STILL routes to Opus (the safety
// floor in TierFor wins — the override cannot sink it). An empty model_tier (the
// fault-tolerant default) yields exactly TierFor's verdict, so a workflow without
// the field is byte-for-byte unchanged.
//
// HONESTY: model_tier is an explicit author override, but the safety floor
// (reviewer/architect/cto -> Opus) is supreme — overrides are raise-only. Under
// the dry-run executor the resolved tier is narrative/prompt-fidelity only; no
// model is actually invoked.
func phaseTier(p asset.Phase, mode string) string {
	base := routing.TierFor(p.Agent, mode)
	if p.ModelTier == "" {
		return base
	}
	// Argument order matters: Higher returns its FIRST argument on a rank tie, so
	// pass base first. An UNRECOGNIZED model_tier ranks as the cheapest (rank 0)
	// and ties with a haiku base — keeping base first means a garbage override can
	// never displace a valid routed tier, only a strictly-higher known tier lifts.
	return routing.Higher(base, p.ModelTier)
}

func (d DryRunExecutor) logf(format string, args ...any) {
	if d.Log != nil {
		d.Log(fmt.Sprintf(format, args...))
	}
}

// Engine runs a workflow. Exec performs agent phases; RunGate runs one named
// gate and reports its Result (injected so tests can supply a fake and the CLI
// can wire the real harness). A nil RunGate treats every gate as failing,
// surfacing misconfiguration loudly rather than silently passing.
//
// MaxRetries is the hard ceiling on RETRIES (re-attempts beyond the first) of a
// single agent phase, and only a retryable failure consumes one. This is the
// consumer of ExecError.Retryable that turns ROADMAP direction-one's "classify
// to drive retry vs human vs halt" into behavior: a transient KindTimeout is
// re-attempted up to MaxRetries times, while a permanent KindConfig or a clean
// KindFailed (and any non-ExecError) aborts on the spot — retrying those only
// burns turns. The default 0 means "no retries": the first error aborts,
// exactly the pre-retry behavior, so existing runs are byte-for-byte unchanged.
//
// ModePolicy is the central knob's Workflow-depth output (mode × lifecycle,
// distilled by internal/mode): it FILTERS what Run actually executes — a gate
// phase runs only the intersection of its required_gates with the policy's
// gate-set, a phase gated on the reviewer (RequiredWhen) is SKIPPED when the
// policy makes the reviewer optional (explorer), the whole DISCOVER stage is
// elided when DiscoverDepth=="skip" (explorer), and a design-stage writes_adr
// phase NARRATES whether an ADR is required (Policy.ADR). BACKWARD-COMPATIBILITY
// CONTRACT: the ZERO-VALUE ModePolicy means "no mode gating" — Run executes the
// workflow UNFILTERED (every required_gate, no phase skipped, no stage elided),
// byte-for-byte as before this field existed. Only an explicitly injected non-zero
// policy filters; cmd injects mode.Effective(mode, lifecycle). SAFETY: because
// mode.Effective forces the full policy under the production lifecycle (and for any
// unknown input), a production run filters to ALL gates, runs FULL discovery, and
// requires an ADR even when mode=explorer — a loose mode can never relax
// enforcement here.
// MaxLoopBack is the hard ceiling on DIRECTED LOOP-BACKS triggered by gate
// phases that declare on_fail:{action:loop_back} — a doom-loop backstop, never
// the goal. Each time such a phase fails and the runtime jumps back to its
// target phase, one unit is consumed; when the budget is spent a still-red gate
// ABORTS (fail-closed). The default 0 means "no loop-back": a red gate aborts on
// the spot exactly as before this field existed, so a workflow whose gate phases
// carry no on_fail is byte-for-byte unchanged regardless of this value, and even
// one that DOES carry on_fail still aborts on the first red until the budget is
// raised. cmd sets a conservative 3.
type Engine struct {
	Exec        AgentExecutor
	RunGate     func(name string) gate.Result
	Log         func(string)
	MaxRetries  int
	MaxLoopBack int
	ModePolicy  mode.Policy
}

// Run executes the workflow phase by phase under mode, applying the central
// knob's Workflow-depth gating (e.ModePolicy) as it goes. It begins at phase 0;
// RunFrom is the variant the loop uses to begin at a directed start phase.
func (e Engine) Run(wf asset.Workflow, mode string) error {
	return e.RunFrom(wf, mode, 0)
}

// RunFrom executes the workflow starting at phase index `start`, applying mode
// gating, agent retries, and DIRECTED GATE LOOP-BACK as it steps.
//
// For a gate phase it runs the required gates FILTERED by the mode policy (the
// intersection of required_gates with the policy's gate-set). A not-OK result is
// handled by gateOutcome:
//   - If the phase declares on_fail:{action:loop_back} AND the loop-back budget is
//     not yet spent, the runtime JUMPS BACK to the target phase (by name) and
//     re-runs forward to this gate — a directed state-machine transition, not an
//     abort and not a whole-workflow replay. One budget unit is consumed.
//   - Otherwise (no on_fail, an unknown action, an unresolvable target, or the
//     budget spent) it ABORTS with an error — enforcement, fail-closed.
//
// An empty gate intersection is a legal no-op (no gate to run this phase). A
// non-gate phase whose RequiredWhen makes it the optional reviewer is SKIPPED when
// the policy turns the reviewer off (explorer); otherwise the executor runs. After
// all phases complete, it logs whether the stop condition is (declared) satisfied.
// It returns the first unrecoverable error, or nil on a clean run.
//
// BACK-COMPAT: with the zero-value ModePolicy gating is INACTIVE (every
// required_gate runs, no phase skipped); with MaxLoopBack 0 or gate phases that
// carry no on_fail, a red gate aborts on the spot exactly as before — directed
// loop-back is opt-in on BOTH the asset (on_fail) and the engine (budget) side.
func (e Engine) RunFrom(wf asset.Workflow, mode string, start int) error {
	if e.discoverStageSkipped(wf) {
		e.logf("discover stage skipped (mode gating: explorer skips discovery)")
		e.reportStop(wf)
		return nil
	}
	loopBacks := 0
	for i := start; i < len(wf.Phases); i++ {
		p := wf.Phases[i]
		if len(p.RequiredGates) > 0 {
			if err := e.runGates(p, e.gatesFor(p)); err != nil {
				target, jumped := e.gateOutcome(wf, p, &loopBacks)
				if !jumped {
					return err
				}
				i = target - 1 // -1 because the for-loop will ++ back to target
				continue
			}
			continue
		}
		if e.skipByMode(p) {
			e.logf("phase %s skipped (mode gating: reviewer off)", p.Name)
			continue
		}
		e.narrateADR(wf, p)
		if err := e.runAgentPhase(p, mode); err != nil {
			return err
		}
	}
	e.reportStop(wf)
	return nil
}

// gateOutcome decides what happens after a gate phase failed: a DIRECTED jump
// back to its on_fail target, or "no jump" (the caller then aborts — fail-closed).
// It returns the target phase index and whether a jump was taken, consuming one
// unit of the loop-back budget (*loopBacks) on a jump. A jump is taken only when
// ALL hold: the phase declares on_fail with action "loop_back", the target phase
// resolves by name, and the budget is not yet spent (loopBacks < MaxLoopBack).
// Any miss is logged with the honest reason and returns jumped=false so the run
// aborts — back-compat (no on_fail) and fail-closed (budget spent) share this path.
func (e Engine) gateOutcome(wf asset.Workflow, p asset.Phase, loopBacks *int) (target int, jumped bool) {
	if p.OnFail == nil || p.OnFail.Action != "loop_back" {
		return 0, false // no directed loop-back declared: legacy abort.
	}
	idx, ok := phaseIndex(wf, p.OnFail.TargetPhase)
	if !ok {
		e.logf("phase %s: on_fail target %q not found — aborting (fail-closed)", p.Name, p.OnFail.TargetPhase)
		return 0, false
	}
	if *loopBacks >= e.MaxLoopBack {
		e.logf("phase %s: gate still red after %d/%d loop-backs to %s — aborting (fail-closed)",
			p.Name, *loopBacks, e.MaxLoopBack, p.OnFail.TargetPhase)
		return 0, false
	}
	*loopBacks++
	e.logf("phase %s: gate FAILED, loop-back %d/%d to %s (re-running %s→%s)",
		p.Name, *loopBacks, e.MaxLoopBack, p.OnFail.TargetPhase, p.OnFail.TargetPhase, p.Name)
	return idx, true
}

// phaseIndex returns the index of the phase named `name`, or ok=false when no
// phase carries that name (an unresolvable on_fail/on_unmet target). The lookup is
// by name — the asset stored the target as a phase name, and the runtime resolves
// it to a position here, the orchestration counterpart to requiredWhenKey.
func phaseIndex(wf asset.Workflow, name string) (int, bool) {
	for i, p := range wf.Phases {
		if p.Name == name {
			return i, true
		}
	}
	return 0, false
}

// gatingActive reports whether a mode policy was actually injected. The
// zero-value Policy ({nil Gates, Reviewer:false}) means "no mode gating" and
// gating stays INACTIVE (full back-compat: nothing is filtered). Any real mode
// is distinguishable from the zero value — it carries gates, or (cto, the only
// empty-gate mode) forces the reviewer on — so this never mistakes a configured
// policy for the zero value. See mode.Policy's zero-value contract.
func (e Engine) gatingActive() bool {
	return len(e.ModePolicy.Gates) > 0 || e.ModePolicy.Reviewer
}

// gatesFor returns the gates a gate phase should actually run: with gating
// inactive (zero policy) it is the phase's full required_gates (back-compat);
// with gating active it is the INTERSECTION of required_gates and the policy's
// gate-set, so explorer drops complexity/arch/security while a production-forced
// full policy keeps them all. Order follows the phase's declaration. An empty
// result is legal — that gate phase simply has no gate to run under this mode.
func (e Engine) gatesFor(p asset.Phase) []string {
	if !e.gatingActive() {
		return p.RequiredGates
	}
	kept := make([]string, 0, len(p.RequiredGates))
	for _, g := range p.RequiredGates {
		if e.ModePolicy.Allows(g) {
			kept = append(kept, g)
		}
	}
	return kept
}

// skipByMode reports whether a non-gate phase should be skipped by the mode
// policy. The only modeled condition (this slice) is the OPTIONAL reviewer: a
// phase whose RequiredWhen resolves to "reviewer" is skipped iff gating is active
// AND the policy makes the reviewer non-mandatory (explorer). With gating
// inactive (zero policy) nothing is ever skipped — full back-compat. A phase with
// no RequiredWhen, or a RequiredWhen naming any other condition, is never skipped
// here (honest: only the reviewer dimension is wired in this slice).
func (e Engine) skipByMode(p asset.Phase) bool {
	if !e.gatingActive() {
		return false
	}
	return requiredWhenKey(p.RequiredWhen) == "reviewer" && !e.ModePolicy.Reviewer
}

// discoverStageSkipped reports whether the WHOLE discover stage should be elided
// for this run — true iff gating is active AND this is the discover stage AND the
// policy's DiscoverDepth is "skip" (explorer's "go straight to build"). It gates
// the entire stage, not a single phase: discover.yml's phases are all agent phases,
// so when skip fires RunFrom runs none of them. With gating inactive (zero policy)
// it is always false — full back-compat, the stage runs unfiltered. light/full
// (or any non-discover stage) also return false, so only the explicit explorer
// skip suppresses the stage.
//
// HONESTY: this is the gating DECISION — the runtime does not run the discovery
// phases and says so; under the dry-run executor it cannot prove the skipped
// discovery was safe to elide (that judgement is a real agent's). It does NOT
// pretend discovery ran or that real work was skipped — only that the explorer
// policy elects to skip the stage.
func (e Engine) discoverStageSkipped(wf asset.Workflow) bool {
	return e.gatingActive() && wf.Stage == "discover" && e.ModePolicy.DiscoverSkipped()
}

// narrateADR reports the ADR gating verdict for a design-stage phase that declares
// writes_adr (design.yml's solution-architect). It NARRATES only — when the mode
// policy requires an ADR (Policy.ADR, e.g. engineering/cto) it logs that an ADR is
// required; otherwise (explorer/balanced) it logs that an ADR is not required and
// will be skipped. It is a no-op unless gating is active, this is the design stage,
// and the phase actually carries writes_adr — so it never fires for build/discover
// or for a phase with no ADR marker (back-compat: zero policy narrates nothing).
//
// HONESTY: under the dry-run executor this is the gating DECISION + narration —
// whether an ADR is required, NOT a real ADR written (design.yml enables the ADR
// target dir from v2; a real agent writes the document). The runtime does not
// pretend an ADR was authored; it reports the required/not-required verdict the
// mode policy dictates.
func (e Engine) narrateADR(wf asset.Workflow, p asset.Phase) {
	if !e.gatingActive() || wf.Stage != "design" || p.WritesADR == nil {
		return
	}
	if e.ModePolicy.ADR {
		e.logf("phase %s: ADR required (mode gating: design writes an ADR) — narrated; real ADR needs a live agent", p.Name)
		return
	}
	e.logf("phase %s: ADR not required (mode gating: explorer/balanced skip ADR) — narrated", p.Name)
}

// requiredWhenKey extracts a RequiredWhen's trailing identifier — the part after
// the last '#' (a fragment) and the last '.' (a dotted path) — so build.yml's
// "../policies/modes.yml#workflow_depth.reviewer" reduces to "reviewer". A bare
// value (no '#'/'.') is returned as-is; an empty value yields "". This is the
// orchestrator interpreting the fragment the asset stored verbatim.
func requiredWhenKey(rw string) string {
	if i := strings.LastIndex(rw, "#"); i >= 0 {
		rw = rw[i+1:]
	}
	if i := strings.LastIndex(rw, "."); i >= 0 {
		rw = rw[i+1:]
	}
	return rw
}

// runAgentPhase executes one agent phase, retrying ONLY on retryable failures up
// to MaxRetries. The first attempt always runs; each subsequent attempt is a
// retry and is taken only when the last error errors.As's to an *ExecError whose
// Retryable() is true AND the retry budget is not yet spent. A non-ExecError or
// any non-retryable ExecError (KindConfig, KindFailed) aborts immediately — the
// pre-retry behavior — and so does exhausting the budget, returning the LAST
// error so the operator sees the final failure, not a stale earlier one.
func (e Engine) runAgentPhase(p asset.Phase, mode string) error {
	if e.Exec == nil {
		return fmt.Errorf("phase %s: no agent executor configured (fail closed)", p.Name)
	}
	for attempt := 0; ; attempt++ {
		err := e.Exec.Execute(p, mode)
		if err == nil {
			return nil
		}
		var execErr *ExecError
		if !errors.As(err, &execErr) || !execErr.Retryable() || attempt >= e.MaxRetries {
			return fmt.Errorf("phase %s: agent execution failed: %w", p.Name, err)
		}
		e.logf("phase %s: retryable %s, retry %d/%d", p.Name, execErr.Kind, attempt+1, e.MaxRetries)
	}
}

// runGates resolves every required gate of a phase with three honest outcomes:
//
//	PASS — the gate was actually CHECKED and passed: log "gate X ok", continue.
//	FAIL — a real check failed: log + ABORT the run (a red gate blocks the
//	       increment — enforcement).
//	NA   — no executable check backs this gate in THIS repo (e.g. lint/build/
//	       security with no tooling): log "gate X N/A (not checked: <detail>)"
//	       and continue. N/A is a known environmental limitation, NOT a pass and
//	       NOT a fail — it never counts as "ok" and never aborts the run.
//
// This is the fix for the FAKE PASS: never-checked gates used to be reported as
// "ok"; now they surface as N/A so the honesty of acceptance.mjs is preserved.
//
// gates is the mode-FILTERED gate list (required_gates ∩ ModePolicy.Gates, or
// the full required_gates when gating is inactive) computed by gatesFor — Run
// passes it in so the filtering lives in one place. An empty gates slice is a
// legal no-op: the phase runs no gate under this mode (logged for visibility).
func (e Engine) runGates(p asset.Phase, gates []string) error {
	if len(gates) < len(p.RequiredGates) {
		e.logf("phase %s: mode gating runs %d/%d gates (%v)", p.Name, len(gates), len(p.RequiredGates), gates)
	}
	for _, name := range gates {
		res := e.callGate(name)
		switch gateStatus(res) {
		case gate.StatusFail:
			e.logf("phase %s: gate %s FAILED", p.Name, name)
			return fmt.Errorf("phase %s: required gate %q not OK: %s", p.Name, name, res.Output)
		case gate.StatusNA:
			e.logf("phase %s: gate %s N/A (not checked: %s)", p.Name, name, naDetail(res))
		default: // StatusPass
			e.logf("phase %s: gate %s ok", p.Name, name)
		}
	}
	return nil
}

// gateStatus reads a Result's tri-state Status, falling back to its OK flag when
// a runner supplies no explicit Status (back-compat: legacy/test fakes set only
// OK). This keeps OK==true -> PASS, OK==false -> FAIL, while honoring an
// explicit NA that a tri-state runner sets.
func gateStatus(res gate.Result) string {
	if res.Status != "" {
		return res.Status
	}
	if res.OK {
		return gate.StatusPass
	}
	return gate.StatusFail
}

// naDetail returns a short reason for an N/A gate, defaulting when the runner
// supplied no detail so the log line is always informative.
func naDetail(res gate.Result) string {
	if res.Output != "" {
		return res.Output
	}
	return "no executable check in this repo"
}

// callGate invokes the injected RunGate, or returns a failing result when none
// is wired so a missing dependency cannot masquerade as a pass.
func (e Engine) callGate(name string) gate.Result {
	if e.RunGate == nil {
		return gate.Result{Name: name, OK: false, Output: "no gate runner configured"}
	}
	return e.RunGate(name)
}

// reportStop logs the workflow's stop condition. ForgeOS forbids round-count
// termination: this notes the declared condition here, and the live
// per-criterion verdict is evaluated against real signals (roadmap completion,
// gate state) by internal/converge — printed by `forge run`'s reportConvergence
// and, per iteration, by LoopEngine.Run. This is metadata, not a convergence
// claim; the actual evaluation is done live elsewhere.
func (e Engine) reportStop(wf asset.Workflow) {
	if wf.Stop.Type == "" {
		e.logf("stop: no condition declared")
		return
	}
	e.logf("stop: condition declared (%s, %d criteria, anti-pattern=%s) — evaluated live by converge",
		wf.Stop.Type, len(wf.Stop.AllOf), wf.Stop.AntiPattern)
}

func (e Engine) logf(format string, args ...any) {
	if e.Log != nil {
		e.Log(fmt.Sprintf(format, args...))
	}
}
