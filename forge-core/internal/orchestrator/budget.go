package orchestrator

import "fmt"

// checkAgentBudget is the per-run agent-call budget guard — the PAIRED PREREQUISITE
// to the recursion guard (CommandExecutor.MaxDepth). The recursion guard bounds
// DEPTH (nested spawns, a fork-bomb); this bounds the TOTAL number of agent-phase
// EXECUTIONS in a single run. RunFrom calls it immediately BEFORE every
// runAgentPhase, so each prospective spawn first charges the budget: it increments
// the run-local counter (*calls) and, when MaxAgentCalls is a positive ceiling that
// the count now exceeds, refuses with a fail-closed error so the phase is NEVER
// spawned (the operator sees the final failure, not a silent overrun).
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
// EVOLVE SCOPE: *calls lives in RunFrom (reset per call) and LoopEngine invokes
// RunFrom once per iteration, so under `forge evolve` this ceiling is PER-ITERATION —
// total spend is bounded by max-iter × MaxAgentCalls, not by MaxAgentCalls alone.
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
	*calls++
	if e.MaxAgentCalls > 0 && *calls > e.MaxAgentCalls {
		e.logf("agent-call budget exhausted (%d > cap %d) — refusing another agent spawn (fail-closed)",
			*calls, e.MaxAgentCalls)
		return fmt.Errorf("agent-call budget exhausted: %d agent-phase executions exceeds the per-run cap of %d (--max-agent-calls)",
			*calls, e.MaxAgentCalls)
	}
	return nil
}
