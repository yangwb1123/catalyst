// Package mode distills .agent/policies/modes.yml into a runnable Workflow-depth
// policy — the third subsystem the central knob (mode × lifecycle) drives, after
// the Router tier (internal/routing) and the out-of-band Harness strictness.
//
// This is the SAME play as internal/routing: a compact, deterministic, pure-Go
// distillation of the declarative YAML — enough for forge-core to FILTER a
// workflow's gates and skip the optional reviewer per mode, not the full PDP
// (that is the v2+ policy service that will consume modes.yml directly).
//
// HONESTY — scope of THIS slice. Of modes.yml's workflow_depth block, Policy
// models exactly two dimensions: the gate-set (harness.gates ∩ this set is what
// the orchestrator actually runs) and reviewer (workflow_depth.reviewer — whether
// the fresh-context Reviewer phase is mandatory). The remaining workflow_depth
// dimensions — discover skip, design depth, adr, evolve depth — are NOT yet
// modeled here (see the TODO fields on Policy); they are a later wave. What this
// distillation encodes is the principle that mode sets ENFORCEMENT STRENGTH:
// explorer deliberately runs FEWER gates (speed over rigor), engineering/cto run
// all of them — but the production LIFECYCLE has a one-vote veto that FORCES full
// enforcement regardless of mode (safety is never relaxed by a loose mode).
package mode

// Gate names — the full gate_catalog from modes.yml (ascending rigor). The
// orchestrator intersects a phase's required_gates with Policy.Gates, so these
// are the vocabulary a Policy can permit.
const (
	GateLint       = "lint"
	GateTest       = "test"
	GateBuild      = "build"
	GateComplexity = "complexity"
	GateArch       = "arch"
	GateSecurity   = "security"
)

// fullGates is the complete gate_catalog: every gate enabled. It backs three
// distinct things and must stay the single list — engineering/cto's baseline,
// the production lifecycle override, AND the fail-safe for unknown input — so
// "all gates" means the same set everywhere.
var fullGates = []string{
	GateLint, GateTest, GateBuild, GateComplexity, GateArch, GateSecurity,
}

// Policy is the effective Workflow-depth decision for one (mode, lifecycle): the
// gate-set the orchestrator may run and whether the Reviewer phase is mandatory.
//
// Gate ZERO-VALUE CONTRACT (load-bearing for backward compatibility): a nil/empty
// Gates with Reviewer=false is NOT "no gates / skip reviewer" — the orchestrator
// treats the zero-value Policy as "no mode gating configured" and runs the
// workflow unfiltered (all required_gates, no phase skipped). A Policy that
// genuinely permits gates therefore always carries a non-empty Gates slice; the
// loosest real mode (explorer) still lists [lint, build]. There is no real mode
// with an empty gate-set EXCEPT cto (which produces no code) — and cto's reviewer
// stays on, so even cto is distinguishable from the zero value by Reviewer=true.
//
// The Discover/ADR/Evolve fields are reserved for the later workflow_depth wave;
// they are unset and unread by this slice (documented honesty, not dead config).
type Policy struct {
	Gates    []string // gate-set this mode may run (∩ with each phase's required_gates)
	Reviewer bool     // is the fresh-context Reviewer phase mandatory?

	// TODO(workflow-depth wave 2): the rest of modes.yml workflow_depth.
	// DiscoverDepth string // skip | light | full
	// ADR           bool   // is an ADR required for complex design?
	// EvolveDepth   string // opportunistic | standard | thorough | advisory
}

// baseline is each mode's (gate-set, reviewer) distilled from modes.yml's
// modes.<mode>.harness.gates and modes.<mode>.workflow_depth.reviewer:
//
//	explorer    → [lint, build]                              reviewer=false  ("does it run")
//	balanced    → [lint, test, build, complexity]            reviewer=true
//	engineering → [lint, test, build, complexity, arch, security] reviewer=true (all gates)
//	cto         → []  (produces no code → no code gates)     reviewer=true   (reviews the docs)
//
// Each entry copies fullGates / a fresh slice so a caller mutating a returned
// Policy.Gates can never corrupt this table.
var baseline = map[string]Policy{
	"explorer":    {Gates: []string{GateLint, GateBuild}, Reviewer: false},
	"balanced":    {Gates: []string{GateLint, GateTest, GateBuild, GateComplexity}, Reviewer: true},
	"engineering": {Gates: allGates(), Reviewer: true},
	"cto":         {Gates: []string{}, Reviewer: true},
}

// Effective resolves the Workflow-depth Policy for a (mode, lifecycle) pair,
// applying modes.yml's composition rule: mode is the BASELINE, lifecycle is a
// MODIFIER that can only TIGHTEN, never loosen ("on conflict the stricter wins").
//
// Order of resolution (each step can only raise rigor):
//
//  1. fail-safe: an unknown/empty mode → the FULL policy (all gates + reviewer).
//     fail-CLOSED in the enforcement direction — an unrecognized posture must
//     never silently DROP gates; better to over-enforce than to skip a gate.
//  2. mode baseline: explorer's lean set, engineering's full set, etc.
//  3. lifecycle override: production is the safety veto — it FORCES all gates +
//     reviewer, overriding ANY loose mode (explorer + production MUST pass every
//     gate, per modes.yml's "explorer+production 也必须过全闸门"). growth raises
//     the floor to its require_min_gates. An unknown/empty lifecycle is itself
//     fail-safe — treated as the strictest, forcing the full policy.
//
// Pure and deterministic; the returned Policy owns its slice (safe to mutate).
func Effective(mode, lifecycle string) Policy {
	p, ok := baseline[mode]
	if !ok {
		// fail-safe: unknown/empty mode over-enforces rather than under-enforces.
		p = fullPolicy()
	} else {
		p = p.clone()
	}
	return applyLifecycle(p, lifecycle)
}

// applyLifecycle tightens a mode baseline by the lifecycle modifier. It can only
// ADD rigor (union the gate floor, OR the reviewer flag up); it never removes a
// gate or turns the reviewer off — modes.yml: "tightens/loosens it; on conflict
// the stricter wins (production always tightens)", and here loosening is simply
// never expressed.
func applyLifecycle(p Policy, lifecycle string) Policy {
	floor, ok := lifecycleFloor[lifecycle]
	if !ok {
		// Unknown/empty lifecycle is fail-safe: treat as the strictest (full).
		return fullPolicy()
	}
	p.Gates = union(p.Gates, floor.minGates)
	p.Reviewer = p.Reviewer || floor.reviewer
	return p
}

// lifecycleMod is the tightening a lifecycle imposes: a minimum gate-set the
// effective policy must include (require_min_gates) and whether it forces the
// reviewer on. Only TIGHTENING is modeled — modifiers never loosen.
type lifecycleMod struct {
	minGates []string
	reviewer bool
}

// lifecycleFloor distills modes.yml lifecycle_modifiers' require_min_gates (and
// production's reviewer veto):
//
//	idea       → no gate floor, reviewer not forced (maximum freedom).
//	mvp        → require_min_gates: [lint, build].
//	growth     → require_min_gates: [lint, test, build, complexity].
//	production → require_min_gates: full set + reviewer forced on — the safety
//	             veto that overrides any loose mode (explorer+production = full).
//
// An unknown/empty lifecycle is intentionally ABSENT so Effective's fail-safe
// branch forces the full policy for it.
var lifecycleFloor = map[string]lifecycleMod{
	"idea":       {minGates: nil, reviewer: false},
	"mvp":        {minGates: []string{GateLint, GateBuild}, reviewer: false},
	"growth":     {minGates: []string{GateLint, GateTest, GateBuild, GateComplexity}, reviewer: false},
	"production": {minGates: allGates(), reviewer: true},
}

// fullPolicy is the maximally strict policy — every gate + reviewer on. It backs
// BOTH the fail-safe (unknown input) and the production override, so "strict"
// means one thing.
func fullPolicy() Policy { return Policy{Gates: allGates(), Reviewer: true} }

// allGates returns a fresh copy of the full gate catalog, so each Policy /
// lifecycle floor owns its slice (no shared backing array to mutate).
func allGates() []string {
	out := make([]string, len(fullGates))
	copy(out, fullGates)
	return out
}

// clone returns a Policy with its own Gates slice, so a caller mutating the
// result can never reach back into the baseline table.
func (p Policy) clone() Policy {
	g := make([]string, len(p.Gates))
	copy(g, p.Gates)
	return Policy{Gates: g, Reviewer: p.Reviewer}
}

// Allows reports whether this Policy permits running the named gate. The
// orchestrator uses it to intersect a phase's required_gates with the mode's
// gate-set. A zero-value (nil Gates) Policy allows NOTHING here — callers must
// route the zero value through the "no gating" path BEFORE consulting Allows
// (see orchestrator.Engine.Run), never letting a zero Policy silently drop gates.
func (p Policy) Allows(gate string) bool {
	for _, g := range p.Gates {
		if g == gate {
			return true
		}
	}
	return false
}

// union returns the de-duplicated combination of two gate slices, PRESERVING a's
// order then appending b's extras. Used to raise a mode baseline to a lifecycle's
// gate floor without reordering or dropping the baseline's gates.
func union(a, b []string) []string {
	seen := make(map[string]bool, len(a)+len(b))
	out := make([]string, 0, len(a)+len(b))
	for _, s := range a {
		if !seen[s] {
			seen[s] = true
			out = append(out, s)
		}
	}
	for _, s := range b {
		if !seen[s] {
			seen[s] = true
			out = append(out, s)
		}
	}
	return out
}
