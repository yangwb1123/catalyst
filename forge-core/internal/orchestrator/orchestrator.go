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
	"context"
	"fmt"
	"strings"
	"time"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
)

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
// elided when DiscoverDepth=="skip" (explorer), the whole REVIEW stage is elided
// when ReviewDepth=="skip" (explorer, mirroring discover exactly), proposal-only
// EvolveAuthority stops at the explicit effect=mutate capability boundary,
// BuildHalt enforces that same cutoff, and a design-stage writes_adr phase
// receives the ADR policy verdict.
// BACKWARD-COMPATIBILITY CONTRACT: the ZERO-VALUE ModePolicy means "no mode
// gating" — Run executes the workflow UNFILTERED (every required_gate, no phase
// skipped, no stage elided), byte-for-byte as before this field existed. Only an
// explicitly injected non-zero policy filters; cmd injects mode.Effective(mode,
// lifecycle). SAFETY: because mode.Effective forces the full policy under the
// production lifecycle (and for any unknown input), a production run filters to
// ALL gates, runs FULL discovery, runs the FULL deep review, and requires an ADR
// even when mode=explorer — a loose mode can never relax enforcement here.
// MaxLoopBack is the hard ceiling on DIRECTED LOOP-BACKS triggered by gate
// phases that declare on_fail:{action:loop_back} — a doom-loop backstop, never
// the goal. Each time such a phase fails and the runtime jumps back to its
// target phase, one unit is consumed; when the budget is spent a still-red gate
// ABORTS (fail-closed). The default 0 means "no loop-back": a red gate aborts on
// the spot exactly as before this field existed, so a workflow whose gate phases
// carry no on_fail is byte-for-byte unchanged regardless of this value, and even
// one that DOES carry on_fail still aborts on the first red until the budget is
// raised. cmd sets a conservative 3.
//
// MaxAgentCalls is the per-run ceiling on AGENT-PHASE EXECUTIONS — the paired
// prerequisite to the recursion guard. The recursion guard (CommandExecutor.MaxDepth)
// bounds nesting DEPTH (a fork-bomb); this bounds the TOTAL count of agent spawns in
// one run, INCLUDING the re-runs a directed loop-back triggers (each is a real spawn,
// real spend). Charged immediately before each runAgentPhase via checkAgentBudget; a
// positive ceiling that the running count exceeds refuses the spawn fail-closed. The
// default 0 means "unbounded": the count is tallied but never trips, so an existing
// run is byte-for-byte unchanged — only a positive ceiling enforces. SCOPE: this
// counts phase EXECUTIONS (loop-back re-runs included); the retries WITHIN a single
// phase are bounded separately by MaxRetries and are NOT charged here (one execution
// charges once). EVOLVE defaults to a RunFrom-local, per-iteration counter. An
// optional ChargeAgentCall closure replaces that local charge source, allowing a CLI
// chain to share one concurrency-safe ceiling across stages and loop iterations.
// It is the predictable cost bound for --executor=command's real firings;
// under a dry-run executor the counting is verifiable but no budget is spent.
type Engine struct {
	Exec    AgentExecutor
	RunGate func(name string) gate.Result
	Log     func(string)
	// OnGateResult is an OPTIONAL callback runGates fires once per gate with its name
	// and OBJECTIVE verdict ("ok" | "N/A" | "FAILED"), so cmd/forge (the ONLY layer
	// that knows prompts) can feed a prior gate's real results into a LATER agent
	// phase's prompt — so the reviewer is told "test: ok" and need not re-run it. The
	// engine just REPORTS, oblivious to where they go (mirror of injected Log/RunGate); nil reports nothing (back-compat, byte-exact).
	OnGateResult func(name, status string)
	// AgentVerdict is an OPTIONAL puller — the REVERSE twin of OnGateResult. Where
	// OnGateResult lets the engine PUSH an objective gate verdict out to cmd/forge,
	// AgentVerdict lets the engine PULL an agent phase's objective verdict back IN
	// after that phase ran: cmd/forge parses the reviewer's machine-readable last line
	// (the `VERDICT: …` contract — see parseReviewerVerdict) and exposes the normalized
	// token here, keyed by phase NAME. The engine stays vendor-free: it never sees the
	// claude/reviewer output, only an opaque ("APPROVE"|"REQUEST_CHANGES", ok) it
	// compares against the loop-back literal. A nil puller (or ok=false) keeps an
	// advisory reviewer fail-open, but a phase declaring qa_v1 fails closed below.
	AgentVerdict func(phase string) (verdict string, ok bool)
	// RequireAgentVerdict identifies the small set of agent phases whose verdict is
	// an enforcement boundary rather than advisory review. For those phases only,
	// AgentVerdict must return exactly APPROVE or REQUEST_CHANGES. A missing,
	// malformed, or unsupported verdict aborts; REQUEST_CHANGES must successfully
	// loop back and aborts once its directed-loop budget is unavailable. Nil keeps
	// every legacy/advisory reviewer fail-open; qa_v1 is intrinsically required.
	RequireAgentVerdict func(phase asset.Phase) bool
	// OnRequiredVerdictApproved commits caller-owned evidence only after a
	// required verdict has been parsed and accepted by this state machine.
	// Returning an error aborts before the phase checkpoint or next phase.
	OnRequiredVerdictApproved func(phase asset.Phase) error
	// BudgetExhausted is an OPTIONAL run-level stop puller — the third cost-bound
	// dimension beside MaxAgentCalls (phase COUNT) and MaxLoopBack (loop-back count).
	// Where those two count discrete events the engine itself tallies, this asks an
	// EXTERNAL accumulator (owned by the caller) a single opaque question before each
	// agent spawn: "is the run-level budget used up?" The engine stays vendor-free and
	// unit-free exactly as it does for AgentVerdict — it never sees dollars, a model,
	// or any vendor envelope, only this bool; whatever resource the caller meters
	// (cmd/forge meters cumulative billed spend) and the threshold it compares against
	// live ENTIRELY in the caller's closure. checkRunBudget consults it immediately
	// before each runAgentPhase: a true verdict is a fail-CLOSED run-level stop (like an
	// over-count in checkAgentBudget — the prospective phase is NEVER spawned), NOT a
	// per-phase retry. A nil puller (the default) means "no run-level budget" — never
	// consulted, so an existing run is byte-for-byte unchanged; only a wired closure
	// enforces. Because RunFrom owns the agent loop and LoopEngine reuses the SAME
	// Engine across every iteration, a closure reading a caller accumulator that is NOT
	// reset per iteration meters the WHOLE evolve run (all iterations), which is the
	// point: a run-level budget bounds total spend, not per-iteration spend.
	BudgetExhausted func() bool
	MaxRetries      int
	MaxLoopBack     int
	MaxAgentCalls   int
	ChargeAgentCall func(max int) (count int, allowed bool)
	ModePolicy      mode.Policy
	// Sleep is the OPTIONAL injection point for the inter-retry backoff (the 529/overload
	// resilience pause), the deterministic-test twin of trace.Now: nil = time.Sleep (the
	// production default, a real wall-clock pause so the overloaded backend can recover); a test
	// supplies a fake that RECORDS durations without sleeping, so the schedule is asserted in
	// microseconds. Only the KindOverloaded retry consults it (a KindTimeout retry already burned
	// its deadline and never sleeps), so a nil Sleep leaves every pre-existing path byte-for-byte
	// unchanged. See runAgentPhase, overloadBackoff, and Engine.sleep (backoff.go).
	Sleep func(time.Duration)
	// OnPhase is an OPTIONAL post-phase checkpoint hook: RunFrom fires it with the
	// index of each AGENT phase that just COMPLETED cleanly (not a gate phase, not a
	// loop-back jump, not a mode-skipped phase). cmd/forge uses it to persist a
	// PHASE-granular checkpoint so a crash mid-iteration resumes at the next unstarted
	// phase instead of replaying every completed (expensively-billed) agent phase. The
	// engine stays iteration-oblivious — it reports only the phase index; the caller
	// (LoopEngine) supplies the iteration context. nil = no per-phase checkpoint
	// (byte-for-byte the pre-existing per-iteration-only behavior).
	OnPhase func(phaseIdx int)
	// Ctx is the parent context for cancellation propagation (e.g. SIGINT/SIGTERM).
	// Every agent phase checks ctx before spawning; when cancelled, the phase is
	// skipped and ctx.Err() is returned. A nil Ctx is equivalent to context.Background()
	// (the backward-compatible default — an existing caller that does not set Ctx is
	// byte-for-byte unchanged).
	Ctx context.Context
}

// ctx returns the effective parent context: Ctx if non-nil, else context.Background().
func (e Engine) ctx() context.Context {
	if e.Ctx != nil {
		return e.Ctx
	}
	return context.Background()
}

// reviewerRequestChanges is the one verdict token that triggers an agent-phase
// directed loop-back. It mirrors cmd/forge's VerdictRequestChanges, duplicated here
// (a bare literal, not an import) BECAUSE the orchestrator must stay free of any
// cmd/forge or claude knowledge — the layering bright-line. The two are pinned
// together by verdict_loopback_test.go, which drives this exact string end to end.
const (
	reviewerApprove        = "APPROVE"
	reviewerRequestChanges = "REQUEST_CHANGES"
)

// Run executes the workflow phase by phase under mode, applying the central
// knob's Workflow-depth gating (e.ModePolicy) as it goes. It begins at phase 0;
// RunFrom is the variant the loop uses to begin at a directed start phase.
func (e Engine) Run(wf asset.Workflow, mode string) error {
	return e.RunFrom(wf, mode, 0)
}

// RunFrom executes the workflow starting at phase index `start`, applying mode
// gating, agent retries, and DIRECTED GATE LOOP-BACK as it steps.
//
// For a phase with required_gates it first runs those gates FILTERED by the mode
// policy (the intersection of required_gates with the policy's gate-set). The
// reserved agent "harness" denotes a pure gate phase and stops there; every other
// agent continues to the executor after its gates pass. A not-OK result is handled
// by gateOutcome:
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
	wf, skipped, err := e.prepareSerialWorkflow(wf, start)
	if err != nil {
		return err
	}
	if skipped {
		return nil
	}
	loopBacks := 0
	agentCalls := 0
	phasesRan := 0
	for i := start; i < len(wf.Phases); i++ {
		// Check for cancellation before each phase (e.g. SIGINT).
		if err := e.ctx().Err(); err != nil {
			return fmt.Errorf("cancelled at phase %d: %w", i, err)
		}
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
			if gateOnlyPhase(p) {
				phasesRan++
				continue
			}
		}
		if e.skipByMode(p, wf.Stage) {
			e.logf("phase %s skipped (mode gating: reviewer off)", p.Name)
			continue
		}
		phasesRan++
		target, jumped, err := e.runAgentTransition(wf, p, mode, i, &agentCalls, &loopBacks)
		if err != nil {
			return err
		}
		if jumped {
			i = target - 1 // -1 because the for-loop will ++ back to target
			continue
		}
	}
	e.warnIfVacuous(wf, phasesRan, start)
	e.reportStop(wf)
	return nil
}

// gateOnlyPhase distinguishes the one non-LLM workflow role from an agent phase
// that also declares gate preconditions. In the shipped assets `agent: harness`
// means "run the toolchain only"; required_gates on any other agent are front
// gates and must not suppress that agent's execution.
func gateOnlyPhase(p asset.Phase) bool {
	return p.Agent == "harness"
}

func (e Engine) runAgentTransition(
	wf asset.Workflow,
	p asset.Phase,
	mode string,
	index int,
	agentCalls, loopBacks *int,
) (target int, jumped bool, err error) {
	e.narrateADR(wf, p)
	if err := e.runAgentPhaseBudgeted(e.ctx(), p, mode, agentCalls); err != nil {
		return 0, false, err
	}
	target, jumped, err = e.agentOutcome(wf, p, loopBacks)
	if err != nil || jumped {
		return target, jumped, err
	}
	// Persist the completed agent index so a crash resumes at the next phase.
	// Gate phases intentionally re-run idempotently after resume.
	if e.OnPhase != nil {
		e.OnPhase(index)
	}
	return target, false, nil
}

// runAgentPhaseBudgeted is the pre-spawn cost pre-flight + the spawn itself, split out of
// RunFrom's loop (so that loop stays within the function-length budget). It charges BOTH
// run-level cost guards BEFORE spawning, fail-closed, then runs the phase:
//
//  1. checkAgentBudget — the per-run agent-phase COUNT cap (--max-agent-calls). It
//     increments *calls; a loop-back re-run re-reaches this and is charged again (loop-back
//     × phase is the blow-up MaxLoopBack alone does not bound). On overrun the phase is
//     never spawned.
//  2. checkRunBudget — the run-level CUMULATIVE-resource cap (the opaque BudgetExhausted
//     puller). *calls-1 is the count of agent phases already COMPLETED this run, passed for
//     an honest "stopped after N" report. On exhaustion the phase is never spawned.
//
// Only when both guards pass does runAgentPhase fire (with its own retry/backoff). Either
// guard's error propagates up to abort the run; neither charges anything when unset
// (MaxAgentCalls 0 / nil puller), so an existing run is byte-for-byte unchanged.
// ctx propagates cancellation so a cancelled runAgentPhase is aborted promptly.
func (e Engine) runAgentPhaseBudgeted(ctx context.Context, p asset.Phase, mode string, calls *int) error {
	if err := e.checkAgentBudget(calls); err != nil {
		return err
	}
	if err := e.checkRunBudget(*calls - 1); err != nil {
		return err
	}
	return e.runAgentPhase(ctx, p, mode)
}

// gateOutcome decides what happens after a gate phase failed: a DIRECTED jump
// back to its on_fail target, or "no jump" (the caller then aborts — fail-closed).
// It is a thin shim over loopBackTo, the shared directed-loop-back core: a red gate
// is fail-CLOSED, so jumped=false means the caller aborts. The "gate FAILED" reason
// is what makes loopBackTo emit the legacy "still red after …" budget-spent line that
// the gate loop-back tests assert.
func (e Engine) gateOutcome(wf asset.Workflow, p asset.Phase, loopBacks *int) (target int, jumped bool) {
	return e.loopBackTo(wf, p, loopBacks, "gate FAILED")
}

// agentOutcome applies two verdict postures after a clean AGENT phase. Legacy/advisory
// reviewers remain fail-open: absent or malformed output proceeds, and an exhausted
// REQUEST_CHANGES loop also proceeds. A phase declaring qa_v1, or selected by the
// caller's RequireAgentVerdict hook, is fail-closed: its verdict must exist and be a
// supported token; REQUEST_CHANGES must take the declared directed loop-back and aborts
// when that jump is unavailable. Both postures share loopBackTo's bounded jump core.
func (e Engine) agentOutcome(wf asset.Workflow, p asset.Phase, loopBacks *int) (target int, jumped bool, err error) {
	strictQA := p.VerdictContract == asset.VerdictContractQAV1
	externallyRequired := e.RequireAgentVerdict != nil && e.RequireAgentVerdict(p)
	required := strictQA || externallyRequired
	if e.AgentVerdict == nil {
		if required {
			return 0, false, fmt.Errorf("phase %s: required agent verdict is unavailable", p.Name)
		}
		return 0, false, nil // no puller wired (dry/echo, or no verdict source): proceed.
	}
	v, ok := e.AgentVerdict(p.Name)
	if !ok {
		if required {
			return 0, false, fmt.Errorf("phase %s: required agent verdict is missing or malformed", p.Name)
		}
		return 0, false, nil // no/garbled advisory verdict: proceed forward.
	}
	switch v {
	case reviewerApprove:
		// Required approval evidence is a caller-owned release contract. Strict QA
		// acceptance authorizes progress but must never mint a release receipt.
		if externallyRequired && !strictQA && e.OnRequiredVerdictApproved != nil {
			if err := e.OnRequiredVerdictApproved(p); err != nil {
				return 0, false, fmt.Errorf("phase %s: commit required approval evidence: %w", p.Name, err)
			}
		}
		return 0, false, nil
	case reviewerRequestChanges:
		reason := "reviewer verdict REQUEST_CHANGES"
		if required {
			reason = "required agent verdict REQUEST_CHANGES"
		}
		target, jumped = e.loopBackTo(wf, p, loopBacks, reason)
		if required && !jumped {
			return 0, false, fmt.Errorf("phase %s: REQUEST_CHANGES could not take its required directed loop-back", p.Name)
		}
		return target, jumped, nil
	default:
		if required {
			return 0, false, fmt.Errorf("phase %s: unsupported required agent verdict %q", p.Name, v)
		}
		return 0, false, nil
	}
}

// loopBackTo is the shared DIRECTED LOOP-BACK core for BOTH a failed gate and a
// reviewer's REQUEST_CHANGES. It returns the target phase index and whether a jump was
// taken, consuming one unit of the loop-back budget (*loopBacks) on a jump. A jump is
// taken only when ALL hold: the phase declares on_fail with action "loop_back", the
// target resolves by name, and the budget is not yet spent (loopBacks < MaxLoopBack).
// Any miss is logged with the honest reason and returns jumped=false. The CALLER owns
// the meaning of jumped=false: gates and required agent contracts abort, while an
// advisory reviewer proceeds. reason flows into the logs so the gate path keeps its legacy
// "gate still red after …" budget-spent wording the loop-back tests assert, while the
// JUMP-success line stays byte-identical ("loop-back %d/%d to %s") for both callers.
func (e Engine) loopBackTo(wf asset.Workflow, p asset.Phase, loopBacks *int, reason string) (target int, jumped bool) {
	if p.OnFail == nil || p.OnFail.Action != "loop_back" {
		return 0, false // no directed loop-back declared: legacy abort / proceed.
	}
	idx, ok := phaseIndex(wf, p.OnFail.TargetPhase)
	if !ok {
		e.logf("phase %s: on_fail target %q not found — %s", p.Name, p.OnFail.TargetPhase, outcomeSuffix(reason))
		return 0, false
	}
	if *loopBacks >= e.MaxLoopBack {
		e.logf("phase %s: %s — %s", p.Name, budgetSpentReason(reason, *loopBacks, e.MaxLoopBack, p.OnFail.TargetPhase), outcomeSuffix(reason))
		return 0, false
	}
	*loopBacks++
	e.logf("phase %s: %s, loop-back %d/%d to %s (re-running %s→%s)",
		p.Name, reason, *loopBacks, e.MaxLoopBack, p.OnFail.TargetPhase, p.OnFail.TargetPhase, p.Name)
	return idx, true
}

// budgetSpentReason renders the budget-exhausted log message, preserving the legacy
// "gate still red after N/M loop-backs to T" wording for the gate path (whose tests
// assert the "still red after" substring) while giving an honest, reason-appropriate
// message for any agent-verdict caller.
func budgetSpentReason(reason string, loopBacks, max int, target string) string {
	if reason == "gate FAILED" {
		return fmt.Sprintf("gate still red after %d/%d loop-backs to %s", loopBacks, max, target)
	}
	return fmt.Sprintf("%s but loop-back budget spent after %d/%d to %s", reason, loopBacks, max, target)
}

// outcomeSuffix names the honest consequence of a non-jump: gates and required verdicts
// abort fail-closed, while an advisory reviewer proceeds fail-open.
func outcomeSuffix(reason string) string {
	if reason == "gate FAILED" || strings.HasPrefix(reason, "required agent verdict ") {
		return "aborting (fail-closed)"
	}
	return "proceeding (fail-open)"
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
