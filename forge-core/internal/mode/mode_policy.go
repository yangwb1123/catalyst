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

// Discover-depth labels — modes.yml workflow_depth.discover's vocabulary,
// ascending rigor. DiscoverSkip makes the orchestrator elide the discover stage's
// agent phases (explorer's "go straight to build"); light/full run them (light =
// requirement-discovery only with market-research optional, full = all phases).
// DiscoverFull is also the CONSERVATIVE default: an unknown/empty depth resolves
// to it, so an unrecognized posture runs discovery rather than silently skipping it
// (fail-safe in the enforcement direction — never silently DROP a stage).
const (
	DiscoverSkip  = "skip"  // skip the discover stage entirely → no agent phases run
	DiscoverLight = "light" // minimal: requirement-discovery, market-research optional
	DiscoverFull  = "full"  // all discover phases run (and the safe default)
)

// Design-depth labels — modes.yml workflow_depth.design's vocabulary, ascending
// rigor. Narrated by the runtime (not yet a hard skip — design phases always run;
// the depth tunes how much an agent does, a later wave). DesignFull is the
// CONSERVATIVE default for an unknown/empty value, matching the discover/adr
// fail-safe direction.
const (
	DesignLight    = "light"    // minimal architecture, ADR optional
	DesignStandard = "standard" // the pragmatic middle
	DesignFull     = "full"     // full architecture + proposal (and the safe default)
)

// Review-depth labels — modes.yml workflow_depth.review's vocabulary, ascending
// rigor. ReviewSkip makes the orchestrator elide the WHOLE review stage (explorer's
// "go straight to build", mirroring DiscoverSkip exactly); standard/full run the
// REVIEW stage the Discover→Design→★Review★→Build→Evolve spine inserts between
// design approval and writing code (standard = a lighter cut of the four review
// dimensions, full = all four: security, distributed, performance+reliability, and
// the CTO executive synthesis). ReviewFull is also the CONSERVATIVE default: an
// unknown/empty depth resolves to it, so an unrecognized posture runs the deep
// review rather than silently skipping it (fail-safe in the enforcement direction,
// matching Discover/Design's default direction).
const (
	ReviewSkip     = "skip"     // skip the review stage entirely → no review phases run
	ReviewStandard = "standard" // a lighter cut of the review dimensions
	ReviewFull     = "full"     // all four review phases (and the safe default)
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

// Priorities is a mode's declared trade-off ranking over the three axes in
// modes.yml `priorities:` — 1 = highest priority. It is a WEAK order: ties are
// legal and intentional (cto ranks speed=cost=3, "quality only").
//
// HONESTY — this is an OBSERVABILITY surface, NOT a routing input. A mode's
// priorities state its INTENT; the EFFECT of that intent is already carried by
// the other knobs this package and internal/routing distill — router_default_tier
// (explorer speed=1 → Haiku, cto quality=1 → Opus), the gate-set, and evolve
// depth. There is deliberately NO independent "priorities → budget/route weight"
// semantics here: modes.yml does not declare that priorities DRIVE anything on
// their own, so wiring one would invent un-declared behavior (gold-plating). v1
// surfaces priorities so the trade-off is inspectable; a real weighting semantics
// is a future design decision, not assumed. check.py's check_mode_priorities keeps
// the declaration honest (well-formed ranking); this exposes it.
type Priorities struct {
	Speed   int // rank 1..3, 1 = highest
	Quality int
	Cost    int
}

// modePriorities is each mode's priorities ranking, distilled VERBATIM from
// modes.yml modes.<mode>.priorities — the same hardcoded-distillation play as
// internal/routing's modeDefault (forge-core is zero-dependency, so it cannot
// parse the YAML at runtime; the table is the single Go mirror of that data,
// kept in lockstep with modes.yml, which check.py independently validates).
//
//	mode         speed quality cost   (modes.yml priorities)
//	explorer     1     3       2      speed > cost > quality
//	balanced     2     1       3      quality first, then speed
//	engineering  3     1       2      quality >> speed
//	cto          3     1       3      quality only (speed=cost tied, deprioritized)
var modePriorities = map[string]Priorities{
	"explorer":    {Speed: 1, Quality: 3, Cost: 2},
	"balanced":    {Speed: 2, Quality: 1, Cost: 3},
	"engineering": {Speed: 3, Quality: 1, Cost: 2},
	"cto":         {Speed: 3, Quality: 1, Cost: 3},
}

// PrioritiesFor returns a mode's declared trade-off ranking and whether the mode
// is known. For an unknown/empty mode it returns the balanced ranking and false —
// balanced is the modes.yml selector default, so an unrecognized mode surfaces the
// default posture's priorities (consistent with routing.defaultFor's balanced
// fallback) while ok=false lets a caller say the mode was not found.
//
// HONESTY: this is a read-only accessor for observability (forge route prints it).
// It does not, and must not, feed the tier/gate/evolve decisions — those already
// encode the trade-off; priorities are the human-readable statement of intent.
func PrioritiesFor(mode string) (Priorities, bool) {
	p, ok := modePriorities[mode]
	if !ok {
		return modePriorities["balanced"], false
	}
	return p, true
}

// baseline is each mode's full workflow-depth posture distilled from modes.yml's
// modes.<mode>.harness.gates and .workflow_depth.{reviewer,evolve,discover,design,review,adr}:
//
//	mode         gates                          reviewer evolve         discover design   review   adr
//	explorer     [lint, build]                  false    opportunistic  skip     light    skip     false
//	balanced     [lint, test, build, complexity] true    standard       light    standard standard false
//	engineering  [all six gates]                true     thorough       full     full     full     true
//	cto          [] (no code → no code gates)   true     advisory       full     full     full     true
//
// Each entry copies fullGates / a fresh slice so a caller mutating a returned
// Policy.Gates can never corrupt this table.
var baseline = map[string]Policy{
	"explorer":    {Gates: []string{GateLint, GateBuild}, Reviewer: false, EvolveDepth: EvolveOpportunistic, DiscoverDepth: DiscoverSkip, DesignDepth: DesignLight, ReviewDepth: ReviewSkip, ADR: false},
	"balanced":    {Gates: []string{GateLint, GateTest, GateBuild, GateComplexity}, Reviewer: true, EvolveDepth: EvolveStandard, DiscoverDepth: DiscoverLight, DesignDepth: DesignStandard, ReviewDepth: ReviewStandard, ADR: false},
	"engineering": {Gates: allGates(), Reviewer: true, EvolveDepth: EvolveThorough, DiscoverDepth: DiscoverFull, DesignDepth: DesignFull, ReviewDepth: ReviewFull, ADR: true},
	"cto":         {Gates: []string{}, Reviewer: true, EvolveDepth: EvolveAdvisory, DiscoverDepth: DiscoverFull, DesignDepth: DesignFull, ReviewDepth: ReviewFull, ADR: true},
}
