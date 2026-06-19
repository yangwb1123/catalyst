// Package orchestrator is forge-core's workflow runtime: it turns a declarative
// Workflow into a state machine that "runs itself" by stepping through phases
// in order. Two phase kinds matter — gate phases (required_gates non-empty),
// where it ENFORCES by invoking the real harness gates and aborting on the
// first red, and agent phases, where it delegates to an AgentExecutor.
//
// The executor is an interface so the runtime stays honest about what it can
// actually do today: the shipped DryRunExecutor only logs the routing decision
// (no LLM is invoked). Wiring a real Agent executor (Claude Agent SDK) behind
// this same interface is the future extension point.
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
// taking the tier from the routing policy for the phase's agent under mode.
func (d DryRunExecutor) Execute(p asset.Phase, mode string) error {
	tier := routing.TierFor(p.Agent, mode)
	d.logf("phase %s -> agent %s (tier %s)", p.Name, p.Agent, tier)
	return nil
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
// gate-set, and a phase gated on the reviewer (RequiredWhen) is SKIPPED when the
// policy makes the reviewer optional (explorer). BACKWARD-COMPATIBILITY CONTRACT:
// the ZERO-VALUE ModePolicy means "no mode gating" — Run executes the workflow
// UNFILTERED (every required_gate, no phase skipped), byte-for-byte as before
// this field existed. Only an explicitly injected non-zero policy filters; cmd
// injects mode.Effective(mode, lifecycle). SAFETY: because mode.Effective forces
// the full policy under the production lifecycle (and for any unknown input), a
// production run filters to ALL gates even when mode=explorer — a loose mode can
// never relax enforcement here.
type Engine struct {
	Exec       AgentExecutor
	RunGate    func(name string) gate.Result
	Log        func(string)
	MaxRetries int
	ModePolicy mode.Policy
}

// Run executes the workflow phase by phase under mode, applying the central
// knob's Workflow-depth gating (e.ModePolicy) as it goes.
//
// For a gate phase it runs the required gates FILTERED by the mode policy (the
// intersection of required_gates with the policy's gate-set); the first not-OK
// result aborts the run with an error (enforcement — a red gate blocks the
// increment). An empty intersection is a legal no-op (no gate to run this phase).
// A non-gate phase whose RequiredWhen makes it the optional reviewer is SKIPPED
// when the policy turns the reviewer off (explorer); otherwise the executor runs.
// After all phases complete, it logs whether the stop condition is (declared)
// satisfied. It returns the first error encountered, or nil on a clean run.
//
// BACK-COMPAT: with the zero-value ModePolicy, gating is INACTIVE — every
// required_gate runs and no phase is skipped, exactly as before this filtering
// existed (see gatingActive / gatesFor).
func (e Engine) Run(wf asset.Workflow, mode string) error {
	for _, p := range wf.Phases {
		if len(p.RequiredGates) > 0 {
			if err := e.runGates(p, e.gatesFor(p)); err != nil {
				return err
			}
			continue
		}
		if e.skipByMode(p) {
			e.logf("phase %s skipped (mode gating: reviewer off)", p.Name)
			continue
		}
		if err := e.runAgentPhase(p, mode); err != nil {
			return err
		}
	}
	e.reportStop(wf)
	return nil
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
