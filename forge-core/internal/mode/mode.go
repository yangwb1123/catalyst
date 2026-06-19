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
// models the FULL set of depth dimensions: the gate-set (harness.gates ∩ this set
// is what the orchestrator actually runs), reviewer (workflow_depth.reviewer —
// whether the fresh-context Reviewer phase is mandatory), evolve depth
// (workflow_depth.evolve — how hard `forge evolve` loops), and now the remaining
// three — discover depth (workflow_depth.discover: skip|light|full — whether the
// Discover STAGE runs at all and how deep), design depth (workflow_depth.design:
// light|standard|full), and adr (workflow_depth.adr — whether an ADR is required
// for a complex design). What this distillation encodes is the principle that mode
// sets ENFORCEMENT STRENGTH: explorer deliberately runs FEWER gates (speed over
// rigor), a SHALLOWER evolve loop, SKIPS discovery, and writes NO ADR, while
// engineering/cto run all gates, a deeper loop, full discovery+design, and require
// an ADR — but the production LIFECYCLE has a one-vote veto that FORCES full
// enforcement regardless of mode (safety is never relaxed by a loose mode).
//
// HONESTY — what the discover/design/adr dimensions DO under a dry-run executor.
// DiscoverDepth=="skip" makes the orchestrator SKIP the discover stage's agent
// phases entirely (explorer), reporting it; light/full run them. DesignDepth and
// ADR are GATING DECISIONS the orchestrator NARRATES (design.yml's solution-architect
// declares writes_adr; the runtime reports whether ADR is required under this mode).
// These are decision-ready + narrated: under the shipped DryRunExecutor no agent
// runs, so the runtime honestly reports the skip/depth/ADR verdict but does not
// pretend it actually performed (or truly elided) the discovery work or wrote a
// real ADR — that value materializes only once a real agent workflow runs behind
// the same interface. The DECISION (skip vs run, ADR required vs not) is real and
// verifiable today; the executed work behind it is the later wave.
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
//
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
// DiscoverDepth / DesignDepth ZERO-VALUE CONTRACT: an empty depth is NOT a skip —
// the orchestrator only ACTS on DiscoverDepth=="skip" (an explicit, non-zero
// value), so a zero-value Policy never skips the discover stage (full back-compat:
// gating inactive runs every phase). DiscoverSkipped() encodes this. Every real
// (mode, lifecycle) sets both depths; the production/fail-safe paths force full.
// ADR is a bool: zero-value false; no real mode leaves it ambiguous (cto/engineering
// set true, explorer/balanced false, production/fail-safe force true).
type Policy struct {
	Gates       []string // gate-set this mode may run (∩ with each phase's required_gates)
	Reviewer    bool     // is the fresh-context Reviewer phase mandatory?
	EvolveDepth string   // workflow_depth.evolve: advisory|opportunistic|standard|thorough → default --max-iter

	// The remaining workflow_depth dimensions (now fully modeled — wave 3).
	DiscoverDepth string // workflow_depth.discover: skip|light|full — skip elides the discover stage
	DesignDepth   string // workflow_depth.design: light|standard|full — narrated design rigor
	ADR           bool   // workflow_depth.adr: is an ADR required for a complex design?
}

// baseline is each mode's full workflow-depth posture distilled from modes.yml's
// modes.<mode>.harness.gates and .workflow_depth.{reviewer,evolve,discover,design,adr}:
//
//	mode         gates                          reviewer evolve         discover design   adr
//	explorer     [lint, build]                  false    opportunistic  skip     light    false
//	balanced     [lint, test, build, complexity] true    standard       light    standard false
//	engineering  [all six gates]                true     thorough       full     full     true
//	cto          [] (no code → no code gates)   true     advisory       full     full     true
//
// Each entry copies fullGates / a fresh slice so a caller mutating a returned
// Policy.Gates can never corrupt this table.
var baseline = map[string]Policy{
	"explorer":    {Gates: []string{GateLint, GateBuild}, Reviewer: false, EvolveDepth: EvolveOpportunistic, DiscoverDepth: DiscoverSkip, DesignDepth: DesignLight, ADR: false},
	"balanced":    {Gates: []string{GateLint, GateTest, GateBuild, GateComplexity}, Reviewer: true, EvolveDepth: EvolveStandard, DiscoverDepth: DiscoverLight, DesignDepth: DesignStandard, ADR: false},
	"engineering": {Gates: allGates(), Reviewer: true, EvolveDepth: EvolveThorough, DiscoverDepth: DiscoverFull, DesignDepth: DesignFull, ADR: true},
	"cto":         {Gates: []string{}, Reviewer: true, EvolveDepth: EvolveAdvisory, DiscoverDepth: DiscoverFull, DesignDepth: DesignFull, ADR: true},
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
	p.DiscoverDepth = deeperDiscover(p.DiscoverDepth, floor.discoverFloor)
	p.DesignDepth = deeperDesign(p.DesignDepth, floor.designFloor)
	p.ADR = p.ADR || floor.adr
	return p
}

// lifecycleMod is the tightening a lifecycle imposes: a minimum gate-set the
// effective policy must include (require_min_gates), whether it forces the
// reviewer on, and the minimum evolve/discover/design depths it raises to plus
// whether it forces an ADR. Only TIGHTENING is modeled — modifiers never loosen.
// An empty *Floor imposes no floor (the mode baseline's depth passes through
// unchanged); adr=false forces nothing (the baseline's ADR flag passes through).
type lifecycleMod struct {
	minGates      []string
	reviewer      bool
	evolveFloor   string // "" = no floor; else raise EvolveDepth to at least this
	discoverFloor string // "" = no floor; else raise DiscoverDepth to at least this
	designFloor   string // "" = no floor; else raise DesignDepth to at least this
	adr           bool   // true = force ADR on (production); false = leave baseline's flag
}

// lifecycleFloor distills modes.yml lifecycle_modifiers' require_min_gates (and
// production's reviewer + evolve/discover/design/adr veto):
//
//	idea       → no gate floor, reviewer not forced, no depth floors (max freedom).
//	mvp        → require_min_gates: [lint, build]; no depth floors.
//	growth     → require_min_gates: [lint, test, build, complexity]; no depth floors.
//	production → require_min_gates: full set + reviewer forced on + evolve floor
//	             raised to standard + discover/design floors raised to FULL + ADR
//	             forced on — the safety veto overriding any loose mode. explorer+
//	             production therefore runs FULL gates, ≥ standard evolve, FULL
//	             discovery (no skipped stage), FULL design, AND requires an ADR — a
//	             prototype's "skip discover / no ADR" can never apply in prod. A
//	             baseline already at/above a floor is never capped down (engineering
//	             stays full/full/true/thorough; the floor RAISES only).
//
// An unknown/empty lifecycle is intentionally ABSENT so Effective's fail-safe
// branch forces the full policy for it.
var lifecycleFloor = map[string]lifecycleMod{
	"idea":       {minGates: nil, reviewer: false},
	"mvp":        {minGates: []string{GateLint, GateBuild}, reviewer: false},
	"growth":     {minGates: []string{GateLint, GateTest, GateBuild, GateComplexity}, reviewer: false},
	"production": {minGates: allGates(), reviewer: true, evolveFloor: EvolveStandard, discoverFloor: DiscoverFull, designFloor: DesignFull, adr: true},
}

// fullPolicy is the maximally strict policy — every gate + reviewer on, FULL
// discover + design depth, ADR required, with a CONSERVATIVE evolve depth of
// standard. It backs BOTH the fail-safe (unknown input) and the production
// override, so "strict" means one thing. discover/design are FULL (an unrecognized
// posture runs discovery + full design + an ADR — never silently skips a stage),
// and ADR is forced on, matching the gate over-enforcement direction. The evolve
// depth is deliberately standard, NOT thorough: an unrecognized posture should
// inherit the historical `--max-iter 5` budget (the safe, well-understood
// default), not a 10-iteration loop it never asked for — over-enforcing GATES (and
// running a stage) is safe, but silently DEEPENING an autonomous loop past the
// legacy default would surprise callers. production then re-raises only to this
// same standard evolve floor (its discover/design/adr floors ARE full/full/true).
func fullPolicy() Policy {
	return Policy{Gates: allGates(), Reviewer: true, EvolveDepth: EvolveStandard,
		DiscoverDepth: DiscoverFull, DesignDepth: DesignFull, ADR: true}
}

// allGates returns a fresh copy of the full gate catalog, so each Policy /
// lifecycle floor owns its slice (no shared backing array to mutate).
func allGates() []string {
	out := make([]string, len(fullGates))
	copy(out, fullGates)
	return out
}

// clone returns a Policy with its own Gates slice, so a caller mutating the
// result can never reach back into the baseline table. The depth strings, ADR
// bool, and Reviewer bool are value-copied, so they need no defensive copy.
func (p Policy) clone() Policy {
	g := make([]string, len(p.Gates))
	copy(g, p.Gates)
	return Policy{Gates: g, Reviewer: p.Reviewer, EvolveDepth: p.EvolveDepth,
		DiscoverDepth: p.DiscoverDepth, DesignDepth: p.DesignDepth, ADR: p.ADR}
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

// DiscoverSkipped reports whether the discover STAGE should be elided under this
// Policy — true iff DiscoverDepth is the explicit "skip" (explorer). This is the
// ONLY discover value that suppresses behavior; light/full (and the zero-value
// empty string) return false, so a zero-value Policy never skips the stage (full
// back-compat). The orchestrator gates the discover stage's agent phases on this,
// and ONLY when gating is active — see orchestrator.Engine.RunFrom.
//
// HONESTY (v1): "skip" makes the runtime NOT run the discover phases and report
// the skip; it does not (cannot, under the dry-run executor) prove the discovery
// work was safely elided — that judgement is a real agent's. The DECISION to skip
// is real; the elided work behind it is narrated, not performed.
func (p Policy) DiscoverSkipped() bool {
	return p.DiscoverDepth == DiscoverSkip
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

// discoverRank / designRank order the discover/design depth labels by rigor so
// deeperDiscover/deeperDesign can pick the stricter of two — the same "raise,
// never lower" composition as evolveRank. An unrecognized/empty label ranks 0
// (below all real labels via the zero value), so an empty floor never lowers a
// real baseline and an unknown baseline is raised by any real floor.
var discoverRank = map[string]int{
	DiscoverSkip:  1,
	DiscoverLight: 2,
	DiscoverFull:  3,
}

var designRank = map[string]int{
	DesignLight:    1,
	DesignStandard: 2,
	DesignFull:     3,
}

// deeperDiscover returns the DEEPER (stricter) of a discover baseline and a
// lifecycle floor — production's full RAISES explorer's skip to full (restoring
// the skipped stage in prod), while a baseline already at full is never capped.
// An empty floor leaves the baseline untouched.
func deeperDiscover(baseline, floor string) string {
	if discoverRank[floor] > discoverRank[baseline] {
		return floor
	}
	return baseline
}

// deeperDesign returns the DEEPER (stricter) of a design baseline and a lifecycle
// floor — production's full RAISES explorer's light to full, while engineering's
// already-full baseline is never capped. An empty floor leaves the baseline
// untouched.
func deeperDesign(baseline, floor string) string {
	if designRank[floor] > designRank[baseline] {
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
