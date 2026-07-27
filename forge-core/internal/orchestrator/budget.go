package orchestrator

import "fmt"

// checkAgentBudget is the per-run agent-call budget guard — the PAIRED PREREQUISITE
// to the recursion guard (CommandExecutor.MaxDepth). The recursion guard bounds
// DEPTH (nested spawns, a fork-bomb); this bounds the TOTAL number of agent-phase
// EXECUTIONS in a single run. RunFrom calls it immediately BEFORE every
// runAgentPhase, so each prospective spawn first checks the budget. An allowed
// execution increments the run-local counter (*calls); when MaxAgentCalls is a
// positive ceiling already reached by completed calls, the next execution is
// refused without incrementing the counter, so the phase is NEVER spawned.
//
// Because RunFrom routes EVERY agent-phase execution through here — including the
// re-runs a directed gate loop-back triggers (a loop-back jumps back to an agent
// phase, which RunFrom reaches and re-executes) — a workflow that bounces N agent
// phases through K loop-backs charges all N×(K+1) executions, not just the N
// distinct phases. That is the point: MaxLoopBack bounds the NUMBER of loop-backs,
// but only this budget bounds the resulting TOTAL agent spend (loop-back × phase
// count is exactly the blow-up MaxLoopBack/MaxIter do not cover).
//
// BACK-COMPAT: MaxAgentCalls == 0 means UNBOUNDED — the count is still tallied but
// never trips, so an existing run is byte-for-byte unchanged. Only a positive
// ceiling enforces.
//
// EVOLVE SCOPE: without ChargeAgentCall, *calls is RunFrom-local, preserving
// standalone evolve's per-iteration ceiling. A chain injects ChargeAgentCall so
// all stages and loop iterations consume one invocation-wide ceiling instead.
//
// HONESTY (scope, mirroring the recursion guard's SCOPE note): the budget counts
// agent-phase EXECUTIONS (each a real spawn under --executor=command), INCLUDING
// loop-back re-runs. It deliberately does NOT separately count the RETRIES within a
// single phase — those re-attempts are bounded independently by MaxRetries (a
// retryable failure consuming one), so charging them here too would double-govern
// the same dimension; one phase execution (however many internal retries it took)
// charges the budget exactly once, at entry. This is the predictable cost bound for
// --executor=command's REAL firings; under a dry-run executor the counting logic is
// fully exercised and verifiable but no real budget is spent.
func (e Engine) checkAgentBudget(calls *int) error {
	if e.ChargeAgentCall != nil {
		count, allowed := e.ChargeAgentCall(e.MaxAgentCalls)
		*calls = count
		if !allowed {
			return e.agentBudgetError(count + 1)
		}
	} else {
		if e.MaxAgentCalls > 0 && *calls >= e.MaxAgentCalls {
			return e.agentBudgetError(*calls + 1)
		}
		*calls++
	}
	return nil
}

func (e Engine) agentBudgetError(attempted int) error {
	e.logf("agent-call budget exhausted (attempted execution %d, completed %d, cap %d) — refusing another agent spawn (fail-closed)",
		attempted, attempted-1, e.MaxAgentCalls)
	return fmt.Errorf("agent-call budget exhausted: refusing agent-phase execution %d after %d completed at the configured cap of %d (--max-agent-calls)",
		attempted, attempted-1, e.MaxAgentCalls)
}

// checkRunBudget is the RUN-LEVEL budget guard — the cumulative-resource sibling of
// checkAgentBudget. Where checkAgentBudget bounds the COUNT of agent-phase executions
// (a number the engine itself tallies), this bounds a cumulative resource the engine
// does NOT meter: it asks the OPTIONAL Engine.BudgetExhausted puller — owned and metered
// entirely by the caller — the single opaque question "is the run-level budget used up?"
// RunFrom calls it immediately BEFORE every runAgentPhase (right after checkAgentBudget),
// so a prospective spawn is refused the instant the budget is gone. A true verdict is a
// fail-CLOSED RUN-LEVEL STOP: this phase and every later one are NEVER spawned, and the
// run ends with the structured error below. This is deliberately a STOP, not a per-phase
// retry — over-budget is exactly like an over-count, not a transient failure.
//
// HONESTY — a budget stop is NOT a run failure. Reaching the cap means the run did its
// work right up to the spend limit and stopped to PREVENT overspend; the message says so
// plainly ("not a failure — the budget is used up") so an operator does not read it as a
// crash. completed is the number of agent phases that already ran, reported for an honest
// "stopped after N" account. (checkRunBudget is a HARD STOP: no near-budget down-tier-and-
// continue HERE. The budget-aware down-tier DOES exist — wired in cmd/forge's
// phaseTierResolver via routing.BudgetAdjustTier (PR6); this hard stop is the floor beneath
// it, reached only when spend fully exhausts the cap.)
//
// BACK-COMPAT: a nil BudgetExhausted is "no run-level budget" — never consulted, zero
// overhead, so an existing run/evolve is byte-for-byte unchanged. Only a wired closure
// (cmd/forge injects one only when --run-budget-usd is set) ever stops a run here.
//
// EVOLVE SCOPE: the engine does not reset anything per iteration; LoopEngine reuses the
// SAME Engine (hence the SAME puller closure) across all iterations, so when the caller's
// accumulator is run-scoped (not iteration-scoped) this guard meters the WHOLE evolve run
// — total spend across every iteration, which is the correct semantics for a run budget.
func (e Engine) checkRunBudget(completed int) error {
	if e.BudgetExhausted == nil || !e.BudgetExhausted() {
		return nil
	}
	e.logf("run budget exhausted after %d agent phase(s) — stopping to prevent overspend (not a failure: the budget is used up, fail-closed)",
		completed)
	return fmt.Errorf("run budget exhausted: stopped after %d completed agent phase(s) to stay within the cumulative run budget (--run-budget-usd); this is a budget stop, not a run failure",
		completed)
}
