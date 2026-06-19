// Package routing maps an (agent, mode) pair to a model tier. It is a compact,
// runnable distillation of .agent/routing/policy.yml and .agent/policies/
// modes.yml — enough for forge-core to log/choose a tier per phase, not the
// full multi-dimensional scorer (that is the v2+ Router service).
//
// One rule is non-negotiable and implemented as a hard floor: agents whose
// work is pure high-stakes judgement — architect, cto, reviewer — always route
// to Opus, regardless of mode or any cheaper default. This mirrors the
// safety_override / reviewer_min_tier floors in the policy data: risk beats
// cost, and a fresh-context reviewer is never down-tiered.
package routing

// Tier names (v1 is Claude-only; ascending capability and cost).
const (
	Haiku  = "haiku"
	Sonnet = "sonnet"
	Opus   = "opus"
)

// opusFloorAgents always route to Opus, overriding mode and any per-agent
// default. These are the judgement-only roles from the routing/modes policies.
var opusFloorAgents = map[string]bool{
	"architect": true,
	"cto":       true,
	"reviewer":  true,
}

// agentTier is the per-agent default floor distilled from policy.yml's
// by_task_type floors and the three coding agents' card defaults:
//   - planner / implementer / qa → Sonnet, the model_tier on those agents' role
//     cards (and the build.yml phase model_tier where present).
//   - "harness" is the non-LLM pseudo-agent that only shells out to the harness
//     gates; "docs" is a by_task_type label (documentation work), not an agent
//     card. Both floor to Haiku as the cheapest work that needs no judgement.
//
// Card-only agents without an entry here — explorer, product-manager,
// researcher (and architect/cto/reviewer, which are handled by the Opus floor
// above) — inherit the mode default via TierFor. Absent agents fall back to the
// mode default.
var agentTier = map[string]string{
	"planner":     Sonnet,
	"implementer": Sonnet,
	"qa":          Sonnet,
	"harness":     Haiku,
	"docs":        Haiku,
}

// modeDefault is each mode's router_default_tier baseline from modes.yml, used
// for agents without a specific entry in agentTier.
var modeDefault = map[string]string{
	"explorer":    Haiku,
	"balanced":    Sonnet,
	"engineering": Sonnet,
	"cto":         Opus,
}

// TierFor returns the model tier for an agent under a mode.
//
// Precedence: the Opus safety floor wins outright; otherwise the higher of the
// agent's policy floor and the mode's default tier is chosen, so a stricter
// mode can lift a cheap agent but never sink a floored one.
func TierFor(agent, mode string) string {
	if opusFloorAgents[agent] {
		return Opus
	}
	base, ok := agentTier[agent]
	if !ok {
		base = defaultFor(mode)
	}
	return higher(base, defaultFor(mode))
}

// defaultFor resolves a mode's baseline tier, defaulting to Sonnet (the
// balanced baseline) for unknown modes.
func defaultFor(mode string) string {
	if t, ok := modeDefault[mode]; ok {
		return t
	}
	return Sonnet
}

// rank orders tiers by capability/cost so they can be compared.
var rank = map[string]int{Haiku: 0, Sonnet: 1, Opus: 2}

// higher returns the more capable of two tiers.
func higher(a, b string) string {
	if rank[b] > rank[a] {
		return b
	}
	return a
}

// Higher is the exported tier-max: the more capable of two tiers, an unknown tier
// ranking as the cheapest (rank 0). It is the "raise, never lower" combiner the
// orchestrator uses to apply a phase's model_tier OVERRIDE on top of TierFor's
// routed verdict — an override can lift the tier but never sink it below the
// safety floor. Mirrors risk.Higher's exported role for risk levels.
func Higher(a, b string) string { return higher(a, b) }

// ── Multi-dimensional task scorer (policy.yml scoring/tiers/override/budget) ──
//
// Score + TierForScore are the task-scoring pathway, distinct from TierFor's
// agent-role mapping above. They mirror the EXACT numbers in policy.yml:
// thresholds, by_task_type floors, safety_override, and budget_guard.

// Threshold bands from scoring.thresholds: total <= 0.34 -> Haiku,
// 0.34 < total <= 0.69 -> Sonnet, total > 0.69 -> Opus.
const (
	haikuMax  = 0.34
	sonnetMax = 0.69
)

// taskTypeFloor mirrors tiers.by_task_type: each task_type's minimum tier
// (a strong prior that can lift a low score straight to Opus).
var taskTypeFloor = map[string]string{
	"docs":            Haiku,
	"crud":            Haiku,
	"test":            Haiku,
	"implementation":  Sonnet,
	"refactor_medium": Sonnet,
	"bugfix":          Sonnet,
	"architecture":    Opus,
	"security":        Opus,
	"payment":         Opus,
	"authorization":   Opus,
	"requirements":    Opus,
	"reviewer":        Opus,
}

// safetyForceOpus mirrors safety_override.rules: these task_types hard-force
// Opus (alongside risk == critical, handled separately in TierForScore).
var safetyForceOpus = map[string]bool{
	"security":      true,
	"payment":       true,
	"authorization": true,
}

// EscalateToHuman is the sentinel returned when budget is exhausted on a
// critical task: per budget_guard, ForgeOS blocks for a human rather than
// silently down-tiering critical work. The caller is expected to escalate.
const EscalateToHuman = "escalate_to_human"

// Score computes the weighted-sum task score from per-dimension 0..1 values and
// their weights, renormalized by the weight total (scoring.normalize: true), so
// drifting weights never shift the output_range. Pure and deterministic; a zero
// or empty weight total yields 0. Dimensions without a weight contribute 0.
func Score(dims map[string]float64, weights map[string]float64) float64 {
	var weighted, totalWeight float64
	for dim, w := range weights {
		totalWeight += w
		weighted += dims[dim] * w
	}
	if totalWeight == 0 {
		return 0
	}
	return weighted / totalWeight
}

// bandForScore maps a renormalized score to its base tier via the thresholds.
func bandForScore(score float64) string {
	switch {
	case score <= haikuMax:
		return Haiku
	case score <= sonnetMax:
		return Sonnet
	default:
		return Opus
	}
}

// TierForScore resolves the final tier for a scored task. The decision chain
// mirrors policy.yml: score -> tier -> task_type floor -> safety_override ->
// budget_guard.
//
//  1. Band the score by thresholds.
//  2. Raise to the by_task_type floor (a strong prior).
//  3. safety_override (HARD): risk == "critical" OR task_type in
//     {security, payment, authorization} forces Opus, ignoring budget.
//  4. budget_guard: 0.80 <= spend < 1.00 and risk < critical downgrades one
//     tier (clamped by the task_type floor); spend >= 1.00 and risk critical
//     returns EscalateToHuman; spend >= 1.00 and risk < critical downgrades to
//     the task_type floor.
func TierForScore(score float64, taskType string, risk string, spendRatio float64) string {
	floor := taskTypeFloor[taskType] // "" (Haiku rank 0) for unknown task types
	tier := higher(bandForScore(score), floor)

	critical := risk == "critical"

	// budget_guard: spend exhausted on a critical task escalates to a human
	// rather than silently down-tiering it (returns the sentinel; safety_override
	// otherwise pins critical to Opus below).
	if spendRatio >= 1.00 && critical {
		return EscalateToHuman
	}

	if critical || safetyForceOpus[taskType] {
		return Opus // safety_override beats budget; override can only raise.
	}

	switch {
	case spendRatio >= 1.00:
		// Non-critical & over budget: collapse to the by_task_type floor.
		return floor
	case spendRatio >= 0.80:
		// Non-critical & near budget: drop exactly one tier. Clamped only by the
		// safety_override floor (the hard Opus pins, already returned above) — not
		// by the softer by_task_type prior — so a score-derived Sonnet can fall to
		// Haiku here, unlike the over-budget collapse which honors the task floor.
		return downgradeOne(tier)
	}
	return tier
}

// downgradeOne returns the next cheaper tier (opus->sonnet->haiku), clamped at
// Haiku. Used by the budget_guard near-budget downgrade.
func downgradeOne(tier string) string {
	switch tier {
	case Opus:
		return Sonnet
	case Sonnet:
		return Haiku
	default:
		return Haiku
	}
}
