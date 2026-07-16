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

import "math"

// Tier names (v1 is Claude-only; ascending capability and cost). Provider names
// extend these for the cross-vendor pool: a fully-qualified tier is "provider/tier"
// (e.g. "openai/gpt-4o", "anthropic/claude-sonnet-4"). The routing package assigns
// tiers without knowing providers; the CLI's executor maps them to --model flags.
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

// Rank orders tiers by capability/cost so they can be compared.
var Rank = map[string]int{Haiku: 0, Sonnet: 1, Opus: 2}

// higher returns the more capable of two tiers.
func higher(a, b string) string {
	if Rank[b] > Rank[a] {
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

// IsOpusFloorAgent reports whether agent is a judgement-only role (architect/cto/reviewer)
// whose Opus safety floor cannot be lowered by budget pressure or learning-loop history.
// These agents always route to Opus regardless of scorecard evidence.
func IsOpusFloorAgent(agent string) bool { return opusFloorAgents[agent] }

// CandidatesForTier returns the HistoryTiebreak candidate set for a given tier: the tier
// itself at index 0 (safe cold-start fallback) followed by cheaper alternatives in
// descending tier order. HistoryTiebreak picks the highest-quality qualifying candidate;
// a cheaper model wins only when it has sufficient samples AND better quality than the
// tier default, making cost-reduction evidence-gated rather than static.
//
// Used by phaseTierResolver and historyDecision for non-floor agents (v1.5 upgrade):
// the safety floor agents (IsOpusFloorAgent) still use a single-element [adj] set so
// their Opus assignment can never be overridden by scorecard data.
func CandidatesForTier(tier string) []string {
	switch tier {
	case Opus:
		return []string{Opus, Sonnet, Haiku}
	case Sonnet:
		return []string{Sonnet, Haiku}
	default:
		return []string{tier}
	}
}

// ── Multi-dimensional task scorer (policy.yml scoring/tiers/override/budget) ──
//
// Score + TierForScore are the task-scoring pathway, distinct from TierFor's
// agent-role mapping above. They mirror the EXACT numbers in policy.yml:
// thresholds, by_task_type floors, safety_override, and budget_guard.

// Threshold bands from scoring.thresholds: total <= 0.34 -> Haiku,
// 0.34 < total <= 0.69 -> Sonnet, total > 0.69 -> Opus.
const (
	HaikuMax  = 0.34
	SonnetMax = 0.69
)

// TaskTypeFloor mirrors tiers.by_task_type: each task_type's minimum tier
// (a strong prior that can lift a low score straight to Opus).
var TaskTypeFloor = map[string]string{
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

// SafetyForceOpus mirrors safety_override.rules: these task_types hard-force
// Opus (alongside risk == critical, handled separately in TierForScore).
var SafetyForceOpus = map[string]bool{
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

// BandForScore maps a renormalized score to its base tier via the thresholds.
func BandForScore(score float64) string {
	switch {
	case score <= HaikuMax:
		return Haiku
	case score <= SonnetMax:
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
	floor := TaskTypeFloor[taskType] // "" (Haiku rank 0) for unknown task types
	tier := higher(BandForScore(score), floor)

	critical := risk == "critical"

	// budget_guard: spend exhausted on a critical task escalates to a human
	// rather than silently down-tiering it (returns the sentinel; safety_override
	// otherwise pins critical to Opus below).
	if spendRatio >= 1.00 && critical {
		return EscalateToHuman
	}

	if critical || SafetyForceOpus[taskType] {
		return Opus // safety_override beats budget; override can only raise.
	}

	switch {
	case spendRatio >= 1.00:
		// Non-critical & over budget: collapse to the by_task_type floor.
		if floor == "" {
			return Haiku // unknown task type -> cheapest tier (no prior to collapse to)
		}
		return floor
	case spendRatio >= 0.80:
		// Non-critical & near budget: drop exactly one tier. Clamped only by the
		// safety_override floor (the hard Opus pins, already returned above) — not
		// by the softer by_task_type prior — so a score-derived Sonnet can fall to
		// Haiku here, unlike the over-budget collapse which honors the task floor.
		return DowngradeOne(tier)
	}
	return tier
}

// DowngradeOne returns the next cheaper tier (opus->sonnet->haiku), clamped at
// Haiku. Used by the budget_guard near-budget downgrade.
func DowngradeOne(tier string) string {
	switch tier {
	case Opus:
		return Sonnet
	case Sonnet:
		return Haiku
	default:
		return Haiku
	}
}

// BudgetAdjustTier applies the budget_guard's NEAR-BUDGET down-tiering to an
// already-routed agent-phase tier, given the run's cumulative spend ratio
// (spent/cap). It is the AGENT-PATHWAY counterpart of TierForScore's score-path
// budget_guard: the same "near budget, drop one tier, but never the safety-floor
// roles" policy, expressed against an agent role instead of a task score+risk.
//
// SEMANTIC MIGRATION of budget_guard from the score path to the agent path:
//   - the "don't down-tier high-stakes work" exemption that TierForScore keys on
//     risk == "critical" becomes, here, opusFloorAgents[agent] — the judgement-only
//     roles (architect / cto / reviewer). Those agents keep their Opus safety floor
//     near budget exactly as a critical task keeps Opus: safety beats cost.
//   - the one-tier drop reuses downgradeOne (the SAME helper the score path uses), so
//     there is one tier-math definition, not a second copy that could drift.
//
// QUALITY TRADE-OFF (honest): near budget, a non-floor phase routes to a CHEAPER,
// LOWER-QUALITY model to extend the run's runway rather than burn the remaining
// budget at full tier and hard-stop sooner. The judgement-only roles are exempt — we
// would rather stop the run (PR4 hard-stop, below) than silently down-tier a reviewer
// or architect and ship a worse decision.
//
// BANDS — why only [0.80, 1.00):
//   - spendRatio < 0.80 (in budget): return base UNCHANGED — byte-for-byte the
//     pre-PR routed tier, so a run that never approaches its cap (or has no cap at
//     all → ratio 0) is completely unaffected.
//   - 0.80 <= spendRatio < 1.00 (near budget): down-tier one step (floor agents exempt).
//   - spendRatio >= 1.00 is DELIBERATELY ABSENT: PR4's run-level hard-stop
//     (checkRunBudget, consulted BEFORE the spawn that would call this) already
//     aborts the run at ratio >= 1.00, so control never reaches BudgetAdjustTier at or
//     past the cap. There is nothing to do here for that band — a >= 1.00 branch would
//     be dead code, so it is omitted (the three bands are disjoint: [0,0.80) untouched
//     | [0.80,1.00) down-tier | [1.00,∞) PR4 already stopped).
//
// PURE and deterministic (no clock, no state) — it only reads its arguments and the
// package's opusFloorAgents/DowngradeOne, mirroring TierForScore's purity.
//
// HONESTY: a NaN or negative spendRatio is a signal of no valid cap (ratio=0) and
// is treated as "in budget" — we refuse to silently downgrade on corrupted input.
func BudgetAdjustTier(base, agent string, spendRatio float64) string {
	if math.IsNaN(spendRatio) || spendRatio < 0 {
		spendRatio = 0 // treat corrupted input as "no cap / fully in budget"
	}
	if spendRatio < 0.80 {
		return base // in budget: unchanged (byte-identical to the routed tier).
	}
	if opusFloorAgents[agent] {
		return base // judgement-only roles keep their Opus safety floor near budget.
	}
	return DowngradeOne(base) // near budget, non-floor: drop one tier to extend runway.
}

// ── Cross-vendor pool (v3 direction) ──────────────────────────────────────

// ModelMap maps a (provider, tier) pair to the model name the executor passes
// to --model. Built-in providers cover the three Claude tiers; external providers
// extend this at the caller level (cmd/forge).
var ModelMap = map[string]map[string]string{
	"anthropic": {
		Haiku:  "claude-sonnet-4-haiku",
		Sonnet: "claude-sonnet-4",
		Opus:   "claude-opus-4",
	},
}

// ResolveModel resolves a (provider, tier) to a model name string.
// When provider is empty, defaults to "anthropic" (the v1 default).
// When the provider+tier combo is unknown, returns tier as-is — the CLI
// executor may still know what to do with the bare tier name.
func ResolveModel(provider, tier string) string {
	if provider == "" {
		provider = "anthropic"
	}
	if models, ok := ModelMap[provider]; ok {
		if m, ok := models[tier]; ok {
			return m
		}
	}
	return tier // fallback: pass tier through as model name
}

// Providers returns the list of known provider names.
func Providers() []string {
	ps := make([]string, 0, len(ModelMap))
	for p := range ModelMap {
		ps = append(ps, p)
	}
	return ps
}
