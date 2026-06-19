// Package mode distills .agent/policies/modes.yml into a runnable Workflow-depth
// policy — the third subsystem the central knob (mode × lifecycle) drives, after
// the Router tier (internal/routing) and the out-of-band Harness strictness.
//
// This is the SAME play as internal/routing: a compact, deterministic, pure-Go
// distillation of the declarative YAML — enough for forge-core to FILTER a
// workflow's gates and skip the optional reviewer per mode, not the full PDP
// (that is the v2+ policy service that will consume modes.yml directly).
//
// HONESTY — scope of THIS slice. Of modes.yml's workflow_depth block, Policy now
// models three dimensions: the gate-set (harness.gates ∩ this set is what the
// orchestrator actually runs), reviewer (workflow_depth.reviewer — whether the
// fresh-context Reviewer phase is mandatory), and evolve depth (workflow_depth.
// evolve — how hard `forge evolve` loops). The remaining workflow_depth dimensions
// — discover skip, design depth, adr — are NOT yet modeled here (see the TODO
// fields on Policy); they are a later wave. What this distillation encodes is the
// principle that mode sets ENFORCEMENT STRENGTH: explorer deliberately runs FEWER
// gates (speed over rigor) and a SHALLOWER evolve loop, engineering/cto run all
// gates and (for engineering) a deeper loop — but the production LIFECYCLE has a
// one-vote veto that FORCES full enforcement regardless of mode (safety is never
// relaxed by a loose mode).
//
// HONESTY — which lifecycle_modifiers dimensions land HERE. Of modes.yml's
// lifecycle_modifiers block, this slice wires only require_min_gates — the gate
// FLOOR a lifecycle adds to the mode's gate-set (∪, tighten-only), the one
// dimension forge-core's Policy can verify because it IS a gate-set. The sibling
// dimensions are deliberately NOT modeled here, each owned by another subsystem:
//   - coverage_delta (idea 0 · growth +10 · production +20): a coverage THRESHOLD
//     adjustment, not a gate name. It needs a coverage tool to mean anything —
//     that lives in the harness coverage adapters (harness/adapters/<lang>.yml's
//     `coverage:` runner), not in this pure gate-set distillation. A later wave.
//   - enforce_floor (warn vs block): the ENFORCEMENT MODE (advisory vs hard-stop),
//     which is a harness-policy knob (harness/policies.yml `enforce:`), applied by
//     the out-of-band harness when it RUNS a gate — not by which gates this Policy
//     permits. forge-core decides the gate-SET; the harness decides warn/block.
// So production's require_min_gates floor is wired below; its coverage_delta/
// enforce_floor veto is honestly left to those subsystems (documented, not dead).
//
// HONESTY — what "evolve depth" means in v1. EvolveDepth maps to ONE concrete
// behavior: the DEFAULT --max-iter (the loop's safety bound) `forge evolve` uses
// when the operator did not pass one (advisory→1, opportunistic→2, standard→5,
// thorough→10). The richer "scan dimensions" semantics modes.yml hints at —
// thorough = full-dimension scan + auto-derive vs opportunistic = only-obvious-
// opportunities, propose-only — require a real agent reading the codebase and are
// a LATER wave. v1 honestly encodes only the iteration budget, not scan breadth.
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

// Evolve-depth labels — modes.yml workflow_depth.evolve's vocabulary, ascending
// loop intensity. Each maps to a default --max-iter via EvolveMaxIter (advisory
// 1, opportunistic 2, standard 5, thorough 10). standard is also the CONSERVATIVE
// default: an unknown/empty depth (or a zero-value Policy) resolves to it, so an
// unrecognized posture inherits today's `--max-iter 5` rather than 0 (which would
// never loop) or a runaway count.
const (
	EvolveAdvisory      = "advisory"      // gap report + roadmap proposal only → 1 iteration
	EvolveOpportunistic = "opportunistic" // scan only the obvious opportunities → 2 iterations
	EvolveStandard      = "standard"      // the pragmatic middle (and the safe default) → 5 iterations
	EvolveThorough      = "thorough"      // full-dimension scan, auto-derive → 10 iterations
)

// evolveMaxIter is EvolveDepth → default --max-iter. An unknown/empty key is NOT
// present, so the lookup miss falls back to defaultEvolveMaxIter (standard's 5) —
// the same conservative number `forge evolve` defaulted to before mode drove it.
var evolveMaxIter = map[string]int{
	EvolveAdvisory:      1,
	EvolveOpportunistic: 2,
	EvolveStandard:      5,
	EvolveThorough:      10,
}

// defaultEvolveMaxIter is the conservative fallback for an unknown/empty/zero
// EvolveDepth — standard's iteration count, identical to the historical
// `--max-iter 5` default, so a Policy that never set EvolveDepth (or set an
// unrecognized one) behaves exactly as the pre-mode CLI did.
const defaultEvolveMaxIter = 5

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
// EvolveDepth ZERO-VALUE CONTRACT (also load-bearing for back-compat): an empty
// EvolveDepth is NOT a third state — EvolveMaxIter resolves it (and any
// unrecognized value) to defaultEvolveMaxIter (standard's 5), so a zero-value
// Policy reports the SAME max-iter the CLI defaulted to before mode drove it. No
// real (mode, lifecycle) leaves EvolveDepth empty (every baseline below sets it,
// and the production/fail-safe paths force standard-or-deeper).
//
// The Discover/ADR fields are reserved for the later workflow_depth wave; they
// are unset and unread by this slice (documented honesty, not dead config).
type Policy struct {
	Gates       []string // gate-set this mode may run (∩ with each phase's required_gates)
	Reviewer    bool     // is the fresh-context Reviewer phase mandatory?
	EvolveDepth string   // workflow_depth.evolve: advisory|opportunistic|standard|thorough → default --max-iter

	// TODO(workflow-depth wave 3): the rest of modes.yml workflow_depth.
	// DiscoverDepth string // skip | light | full
	// ADR           bool   // is an ADR required for complex design?
}

// baseline is each mode's (gate-set, reviewer, evolve-depth) distilled from
// modes.yml's modes.<mode>.harness.gates, .workflow_depth.reviewer, and
// .workflow_depth.evolve:
//
//	explorer    → [lint, build]                              reviewer=false  opportunistic ("does it run")
//	balanced    → [lint, test, build, complexity]            reviewer=true   standard
//	engineering → [lint, test, build, complexity, arch, security] reviewer=true thorough (all gates)
//	cto         → []  (produces no code → no code gates)     reviewer=true   advisory (report/propose only)
//
// Each entry copies fullGates / a fresh slice so a caller mutating a returned
// Policy.Gates can never corrupt this table.
var baseline = map[string]Policy{
	"explorer":    {Gates: []string{GateLint, GateBuild}, Reviewer: false, EvolveDepth: EvolveOpportunistic},
	"balanced":    {Gates: []string{GateLint, GateTest, GateBuild, GateComplexity}, Reviewer: true, EvolveDepth: EvolveStandard},
	"engineering": {Gates: allGates(), Reviewer: true, EvolveDepth: EvolveThorough},
	"cto":         {Gates: []string{}, Reviewer: true, EvolveDepth: EvolveAdvisory},
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
//     reviewer AND raises evolve depth to at least standard, overriding ANY loose
//     mode (explorer + production MUST pass every gate, per modes.yml's
//     "explorer+production 也必须过全闸门", and must not run a prototype-shallow
//     evolve loop). growth raises the floor to its require_min_gates. An
//     unknown/empty lifecycle is itself fail-safe — treated as the strictest,
//     forcing the full policy (standard evolve depth).
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
// ADD rigor (union the gate floor, OR the reviewer flag up, RAISE the evolve-depth
// floor); it never removes a gate, turns the reviewer off, or SHALLOWS the loop —
// modes.yml: "tightens/loosens it; on conflict the stricter wins (production
// always tightens)", and here loosening is simply never expressed.
func applyLifecycle(p Policy, lifecycle string) Policy {
	floor, ok := lifecycleFloor[lifecycle]
	if !ok {
		// Unknown/empty lifecycle is fail-safe: treat as the strictest (full).
		return fullPolicy()
	}
	p.Gates = union(p.Gates, floor.minGates)
	p.Reviewer = p.Reviewer || floor.reviewer
	p.EvolveDepth = deeperEvolve(p.EvolveDepth, floor.evolveFloor)
	return p
}

// lifecycleMod is the tightening a lifecycle imposes: a minimum gate-set the
// effective policy must include (require_min_gates), whether it forces the
// reviewer on, and a minimum evolve depth it raises the loop to. Only TIGHTENING
// is modeled — modifiers never loosen. An empty evolveFloor imposes no floor (the
// mode baseline's depth passes through unchanged).
type lifecycleMod struct {
	minGates    []string
	reviewer    bool
	evolveFloor string // "" = no floor; else raise EvolveDepth to at least this
}

// lifecycleFloor distills modes.yml lifecycle_modifiers' require_min_gates (and
// production's reviewer + evolve-depth veto):
//
//	idea       → no gate floor, reviewer not forced, no evolve floor (max freedom).
//	mvp        → require_min_gates: [lint, build]; no evolve floor.
//	growth     → require_min_gates: [lint, test, build, complexity]; no evolve floor.
//	production → require_min_gates: full set + reviewer forced on + evolve floor
//	             raised to standard — the safety veto overriding any loose mode
//	             (explorer+production = full gates, AND ≥ standard evolve depth, so
//	             a prototype-shallow opportunistic loop can never run in prod).
//	             thorough is already ≥ standard, so engineering+production stays
//	             thorough (the floor RAISES, never caps).
//
// An unknown/empty lifecycle is intentionally ABSENT so Effective's fail-safe
// branch forces the full policy for it.
var lifecycleFloor = map[string]lifecycleMod{
	"idea":       {minGates: nil, reviewer: false},
	"mvp":        {minGates: []string{GateLint, GateBuild}, reviewer: false},
	"growth":     {minGates: []string{GateLint, GateTest, GateBuild, GateComplexity}, reviewer: false},
	"production": {minGates: allGates(), reviewer: true, evolveFloor: EvolveStandard},
}

// fullPolicy is the maximally strict policy — every gate + reviewer on, with a
// CONSERVATIVE evolve depth of standard. It backs BOTH the fail-safe (unknown
// input) and the production override, so "strict" means one thing. The evolve
// depth is deliberately standard, NOT thorough: an unrecognized posture should
// inherit the historical `--max-iter 5` budget (the safe, well-understood
// default), not a 10-iteration loop it never asked for — over-enforcing GATES is
// safe, but silently DEEPENING an autonomous loop past the legacy default would
// surprise callers. production then re-raises only to this same standard floor.
func fullPolicy() Policy {
	return Policy{Gates: allGates(), Reviewer: true, EvolveDepth: EvolveStandard}
}

// allGates returns a fresh copy of the full gate catalog, so each Policy /
// lifecycle floor owns its slice (no shared backing array to mutate).
func allGates() []string {
	out := make([]string, len(fullGates))
	copy(out, fullGates)
	return out
}

// clone returns a Policy with its own Gates slice, so a caller mutating the
// result can never reach back into the baseline table. EvolveDepth is a string
// (value-copied), so it needs no defensive copy.
func (p Policy) clone() Policy {
	g := make([]string, len(p.Gates))
	copy(g, p.Gates)
	return Policy{Gates: g, Reviewer: p.Reviewer, EvolveDepth: p.EvolveDepth}
}

// EvolveMaxIter maps this Policy's EvolveDepth to the DEFAULT --max-iter
// `forge evolve` uses when the operator did not pass one: advisory→1,
// opportunistic→2, standard→5, thorough→10. An unknown/empty depth (including a
// zero-value Policy) falls back to defaultEvolveMaxIter (standard's 5) — the same
// number the CLI defaulted to before mode drove it, so back-compat holds.
//
// HONESTY (v1): this is the ONLY behavior evolve-depth drives today — the loop's
// iteration budget, not the richer "scan breadth" (thorough = full-dimension scan
// vs opportunistic = obvious-only) modes.yml hints at; that needs a real agent and
// is a later wave.
func (p Policy) EvolveMaxIter() int {
	if n, ok := evolveMaxIter[p.EvolveDepth]; ok {
		return n
	}
	return defaultEvolveMaxIter
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

// evolveRank orders the evolve-depth labels by loop intensity so deeperEvolve can
// pick the stricter (deeper) of two. An unrecognized/empty label ranks below all
// of them (rank 0 via the zero value) — so an empty floor never lowers a real
// baseline, and an unknown baseline is raised by any real floor.
var evolveRank = map[string]int{
	EvolveAdvisory:      1,
	EvolveOpportunistic: 2,
	EvolveStandard:      3,
	EvolveThorough:      4,
}

// deeperEvolve returns the DEEPER (stricter) of a mode baseline's depth and a
// lifecycle floor — the "raise, never lower" rule applied to evolve depth, the
// same direction union/||/Higher use for gates/reviewer/risk. An empty floor (no
// lifecycle veto) leaves the baseline untouched; a floor deeper than the baseline
// (production's standard over explorer's opportunistic) wins, while a baseline
// already deeper (engineering's thorough) is never capped down to the floor.
func deeperEvolve(baseline, floor string) string {
	if evolveRank[floor] > evolveRank[baseline] {
		return floor
	}
	return baseline
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
