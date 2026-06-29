// executor.go — the AgentExecutor abstraction the orchestrator delegates agent
// phases to, split out of orchestrator.go to keep that file under the size budget.
// The Engine (orchestrator.go) owns the phase STATE MACHINE; this file owns WHAT a
// phase's agent action is (the dry-run narrator + the per-phase tier resolution the
// real command executor in command_executor.go reuses). Pure Go standard library.
package orchestrator

import (
	"fmt"

	"forgeos/forge-core/internal/asset"
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
// taking the tier from PhaseTier so a workflow's per-phase model_tier override is
// honored (raise-only, never below the safety floor — see PhaseTier).
func (d DryRunExecutor) Execute(p asset.Phase, mode string) error {
	tier := PhaseTier(p, mode)
	d.logf("phase %s -> agent %s (tier %s)", p.Name, p.Agent, tier)
	return nil
}

// PhaseTier resolves the model tier for a phase under a mode, honoring an
// OPTIONAL per-phase model_tier OVERRIDE authored in the workflow asset. Exported
// because the REAL executor (cmd/forge) maps it onto `claude --model <tier>`, so a
// real run honors the routed tier — not just the dry-run narration.
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
// (reviewer/architect/cto -> Opus) is supreme — overrides are raise-only. Under the
// dry-run executor the tier is narration only; under the command executor it is
// passed to `claude --model`, so the routed tier actually drives the model.
func PhaseTier(p asset.Phase, mode string) string {
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
