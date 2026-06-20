package orchestrator

import (
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/routing"
)

// PhaseTier resolves the per-phase model tier, honoring a workflow's model_tier
// OVERRIDE that can only RAISE the routed tier — never lower it below the safety
// floor. Each row is (agent, model_tier, mode) -> expected tier.
func TestPhaseTier(t *testing.T) {
	cases := []struct {
		name      string
		agent     string
		modelTier string
		mode      string
		want      string
	}{
		// Override RAISES a plain agent: docs routes to haiku under explorer, an
		// opus hint lifts it to opus.
		{"override opus lifts plain agent", "docs", routing.Opus, "explorer", routing.Opus},
		// Override sonnet lifts a haiku base (docs in explorer routes to haiku).
		{"override sonnet lifts haiku base", "docs", routing.Sonnet, "explorer", routing.Sonnet},
		// No override -> exactly the routed agent tier (unchanged): docs in explorer
		// is haiku; implementer in balanced is sonnet.
		{"no override uses agent tier (haiku)", "docs", "", "explorer", routing.Haiku},
		{"no override uses agent tier (sonnet)", "implementer", "", "balanced", routing.Sonnet},
		// ★ SAFETY FLOOR NOT BROKEN ★: a phase that authors model_tier: haiku on the
		// reviewer STILL routes to Opus — the override is raise-only and can never
		// sink a floored judgement agent below its safety floor.
		{"override haiku CANNOT sink reviewer below opus floor", "reviewer", routing.Haiku, "explorer", routing.Opus},
		{"override sonnet CANNOT sink architect below opus floor", "architect", routing.Sonnet, "engineering", routing.Opus},
		// Override equal to the floor is a no-op (reviewer opus stays opus).
		{"override opus on reviewer is a no-op", "reviewer", routing.Opus, "balanced", routing.Opus},
		// Override BELOW a non-floor agent's routed tier cannot lower it either:
		// implementer floors at sonnet, a haiku hint stays sonnet (raise-only).
		{"override haiku cannot lower implementer below its sonnet floor", "implementer", routing.Haiku, "balanced", routing.Sonnet},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			p := asset.Phase{Name: c.name, Agent: c.agent, ModelTier: c.modelTier}
			if got := PhaseTier(p, c.mode); got != c.want {
				t.Errorf("PhaseTier(agent=%q, model_tier=%q, mode=%q) = %q, want %q",
					c.agent, c.modelTier, c.mode, got, c.want)
			}
		})
	}
}

// DryRunExecutor.Execute must NARRATE the PhaseTier verdict, so a workflow's
// model_tier override is visible in the dry-run log: a plain agent (implementer)
// with model_tier: opus narrates tier opus, not its sonnet default.
func TestDryRunExecutor_HonorsModelTierOverride(t *testing.T) {
	rec := &recorder{}
	exec := DryRunExecutor{Log: rec.log}
	p := asset.Phase{Name: "implementer", Agent: "implementer", ModelTier: routing.Opus}
	if err := exec.Execute(p, "balanced"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if !containsLine(rec.logs, "phase implementer -> agent implementer (tier opus)") {
		t.Errorf("override must lift the narrated tier to opus; logs=%v", rec.logs)
	}
}

// HONESTY / safety: even a dry-run narration of a reviewer phase that authors
// model_tier: haiku must STILL report opus — the safety floor is supreme over the
// override in the executor's OUTPUT, not just inside PhaseTier.
func TestDryRunExecutor_OverrideCannotBreakFloorInNarration(t *testing.T) {
	rec := &recorder{}
	exec := DryRunExecutor{Log: rec.log}
	p := asset.Phase{Name: "reviewer", Agent: "reviewer", ModelTier: routing.Haiku}
	if err := exec.Execute(p, "explorer"); err != nil {
		t.Fatalf("Execute: %v", err)
	}
	if !containsLine(rec.logs, "phase reviewer -> agent reviewer (tier opus)") {
		t.Errorf("safety floor must beat a haiku override in narration; logs=%v", rec.logs)
	}
	if containsLine(rec.logs, "tier haiku") {
		t.Errorf("a floored reviewer must never narrate tier haiku; logs=%v", rec.logs)
	}
}
