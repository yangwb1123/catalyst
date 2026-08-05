package routing

import (
	"math"
	"testing"
)

func TestTierFor_OpusFloor(t *testing.T) {
	// The judgement-only roles are pinned to Opus under EVERY mode, even the
	// cheapest one — this is the safety floor, not a default.
	floors := []string{"architect", "cto", "reviewer"}
	modes := []string{"explorer", "balanced", "engineering", "cto", "unknown-mode"}
	for _, agent := range floors {
		for _, mode := range modes {
			if got := TierFor(agent, mode); got != Opus {
				t.Errorf("TierFor(%q, %q) = %q, want %q (safety floor)", agent, mode, got, Opus)
			}
		}
	}
}

func TestTierFor_AgentDefaults(t *testing.T) {
	cases := []struct {
		agent, mode, want string
	}{
		{"implementer", "balanced", Sonnet},
		{"planner", "explorer", Sonnet}, // planner=sonnet floor, lifted above explorer's haiku
		{"harness", "explorer", Haiku},  // cheap agent in cheap mode
		{"qa", "balanced", Sonnet},
		{"docs", "balanced", Sonnet}, // docs floor=haiku, lifted by balanced default
	}
	for _, c := range cases {
		if got := TierFor(c.agent, c.mode); got != c.want {
			t.Errorf("TierFor(%q, %q) = %q, want %q", c.agent, c.mode, got, c.want)
		}
	}
}

func TestTierFor_ModeDefaultForUnknownAgent(t *testing.T) {
	// An agent with no specific floor inherits the mode default.
	if got := TierFor("mystery", "explorer"); got != Haiku {
		t.Errorf("unknown agent in explorer = %q, want %q", got, Haiku)
	}
	if got := TierFor("mystery", "engineering"); got != Sonnet {
		t.Errorf("unknown agent in engineering = %q, want %q", got, Sonnet)
	}
	if got := TierFor("mystery", "cto"); got != Opus {
		t.Errorf("unknown agent in cto mode = %q, want %q", got, Opus)
	}
}

func TestTierFor_StricterModeLiftsCheapAgent(t *testing.T) {
	// harness floors at haiku, but cto mode default is opus -> the higher wins.
	if got := TierFor("harness", "cto"); got != Opus {
		t.Errorf("harness in cto = %q, want %q (mode lifts agent)", got, Opus)
	}
}

// Higher is the exported "raise, never lower" tier-max the orchestrator uses to
// apply a phase's model_tier override. It returns the more capable of two tiers;
// a strictly-higher tier wins regardless of position, and a RANK TIE returns the
// FIRST argument (so the orchestrator passes the routed base first — a garbage
// override that ranks at 0 can never displace a valid base, only lift it).
func TestHigher(t *testing.T) {
	cases := []struct{ a, b, want string }{
		{Haiku, Opus, Opus},
		{Opus, Haiku, Opus}, // strictly-higher wins from either position
		{Sonnet, Haiku, Sonnet},
		{Haiku, Sonnet, Sonnet},
		{Opus, Opus, Opus}, // equal -> same
		{Sonnet, Sonnet, Sonnet},
		// Rank tie returns the FIRST arg: an unknown tier ranks 0, the same as
		// haiku, so order decides. This is exactly why phaseTier passes base first.
		{Haiku, "garbage", Haiku},     // base haiku, garbage override -> base stays
		{"garbage", Haiku, "garbage"}, // tie returns first arg (documents the order)
		{Opus, "garbage", Opus},       // a real floor always beats a rank-0 unknown
	}
	for _, c := range cases {
		if got := Higher(c.a, c.b); got != c.want {
			t.Errorf("Higher(%q, %q) = %q, want %q", c.a, c.b, got, c.want)
		}
	}
}

func TestScore_WeightedSumRenormalized(t *testing.T) {
	// Weights here sum to 1.0 exactly (policy dimension weights), so the
	// renormalized result equals the raw weighted sum.
	dims := map[string]float64{
		"complexity":        0.8,
		"risk":              0.6,
		"dependency_change": 0.0,
		"security":          0.0,
		"context_size":      1.0,
		"business_impact":   0.5,
	}
	weights := map[string]float64{
		"complexity":        0.25,
		"risk":              0.25,
		"dependency_change": 0.12,
		"security":          0.18,
		"context_size":      0.10,
		"business_impact":   0.10,
	}
	// 0.8*.25 + 0.6*.25 + 0 + 0 + 1.0*.10 + 0.5*.10 = 0.20+0.15+0.10+0.05 = 0.50
	if got := Score(dims, weights); math.Abs(got-0.50) > 1e-9 {
		t.Errorf("Score = %v, want 0.50", got)
	}

	// Renormalization: doubling every weight must not change the score (Σw drifts
	// from 1.0 to 2.0 but the ratio is invariant).
	doubled := make(map[string]float64, len(weights))
	for k, v := range weights {
		doubled[k] = v * 2
	}
	if got := Score(dims, doubled); math.Abs(got-0.50) > 1e-9 {
		t.Errorf("Score with doubled weights = %v, want 0.50 (renormalized)", got)
	}

	// Non-unit weights: a single dimension with any positive weight renormalizes
	// to exactly that dimension's value.
	if got := Score(map[string]float64{"a": 0.42}, map[string]float64{"a": 7.0}); math.Abs(got-0.42) > 1e-9 {
		t.Errorf("single-dim Score = %v, want 0.42", got)
	}
}

func TestScore_NegativeDimValues(t *testing.T) {
	dims := map[string]float64{"complexity": -0.5, "risk": -0.3}
	weights := map[string]float64{"complexity": 0.5, "risk": 0.5}
	score := Score(dims, weights)
	// Negative inputs produce negative scores but never panic.
	if score >= 0 {
		t.Errorf("negative dims should yield negative score, got %v", score)
	}
}

func TestScore_NaNDimValue(t *testing.T) {
	dims := map[string]float64{"complexity": math.NaN(), "risk": 0.5}
	weights := map[string]float64{"complexity": 0.5, "risk": 0.5}
	score := Score(dims, weights)
	// NaN in any dim propagates through weighted sum. Math.IsNaN check.
	if !math.IsNaN(score) {
		t.Errorf("NaN dim should produce NaN score, got %v", score)
	}
}

func TestScore_ZeroWeightTotal(t *testing.T) {
	if got := Score(map[string]float64{"a": 0.9}, map[string]float64{}); got != 0 {
		t.Errorf("Score with empty weights = %v, want 0", got)
	}
	if got := Score(map[string]float64{"a": 0.9}, map[string]float64{"a": 0}); got != 0 {
		t.Errorf("Score with zero weight = %v, want 0", got)
	}
}

// tierForScoreCase is one TierForScore expectation. The table lives at package
// scope (not inside the test) so the test body stays a thin loop — the routing
// policy bands are data, and a large data table should not blow the function-
// length budget that arch-check enforces.
type tierForScoreCase struct {
	name       string
	score      float64
	taskType   string
	risk       string
	spendRatio float64
	want       string
}

func TestTierForScore_EdgeCases(t *testing.T) {
	// Negative spendRatio -> in-budget (no downgrade).
	if got := TierForScore(0.90, "implementation", "low", -1.0); got != Opus {
		t.Errorf("negative spendRatio should not trigger budget guard, got %q", got)
	}
	// NaN spendRatio -> in-budget (no downgrade).
	if got := TierForScore(0.90, "implementation", "low", math.NaN()); got != Opus {
		t.Errorf("NaN spendRatio should not trigger budget guard, got %q", got)
	}
	// NaN score -> Opus (safest fallback for unknown input).
	if got := TierForScore(math.NaN(), "crud", "low", 0.0); got != Opus {
		t.Errorf("NaN score should fall back to Opus, got %q", got)
	}
	// score out of [0,1] range -> no panic, tier clamped by band.
	if got := TierForScore(2.5, "implementation", "low", 0.0); got != Opus {
		t.Errorf("score=2.5 should map to Opus, got %q", got)
	}
	if got := TierForScore(-0.5, "implementation", "low", 0.0); got != Sonnet {
		t.Errorf("score=-0.5 should map to Sonnet (task floor), got %q", got)
	}
	// Fuzzer-discovered: unknown task type + over budget must NOT return "".
	// Regression: the over-budget branch returned floor ("" for unknown types).
	if got := TierForScore(249962, "0", "0", 2.0); got != Haiku {
		t.Errorf("unknown task type over budget should collapse to Haiku, got %q", got)
	}
}

var tierForScoreCases = []tierForScoreCase{
	// (1) Each threshold band, low risk / in-budget / no task floor.
	{"band haiku lower", 0.00, "crud", "low", 0.0, Haiku},
	{"band haiku boundary", 0.34, "crud", "low", 0.0, Haiku},
	{"band sonnet lower", 0.3401, "implementation", "low", 0.0, Sonnet},
	{"band sonnet boundary", 0.69, "implementation", "low", 0.0, Sonnet},
	{"band opus", 0.70, "implementation", "low", 0.0, Opus},
	{"band opus high", 1.00, "implementation", "low", 0.0, Opus},

	// (2) by_task_type floor lifts a low score.
	{"task floor sonnet over haiku band", 0.10, "bugfix", "low", 0.0, Sonnet},
	{"task floor opus arch over haiku band", 0.05, "architecture", "low", 0.0, Opus},

	// (3) safety_override: payment/security force Opus from a haiku score.
	{"payment opus floor", 0.10, "payment", "low", 0.0, Opus},
	{"security opus floor", 0.10, "security", "low", 0.0, Opus},
	{"authorization opus floor", 0.10, "authorization", "low", 0.0, Opus},

	// (3) safety_override: critical risk forces Opus, ignoring budget...
	{"critical forces opus low score", 0.10, "crud", "critical", 0.0, Opus},
	{"critical pinned near budget", 0.50, "implementation", "critical", 0.85, Opus},
	// ...and at spend >= 1.0 critical escalates instead of downgrading.
	{"critical escalates over budget", 0.90, "implementation", "critical", 1.00, EscalateToHuman},
	{"critical escalates well over budget", 0.10, "crud", "critical", 1.50, EscalateToHuman},

	// (4) budget_guard near-budget (0.80<=spend<1.00) downgrades one tier.
	{"0.85 non-critical sonnet->haiku", 0.50, "implementation", "low", 0.85, Haiku},
	{"0.80 boundary opus->sonnet", 0.90, "implementation", "low", 0.80, Sonnet},
	// Near-budget downgrade is clamped only by the safety floor, NOT the
	// softer task_type prior, so a sonnet-floored bugfix still falls to haiku.
	{"near-budget ignores task prior", 0.50, "bugfix", "medium", 0.90, Haiku},
	// Just under the 0.80 gate: no downgrade, tier stands.
	{"below near-budget gate no change", 0.50, "implementation", "low", 0.79, Sonnet},

	// (4) budget_guard over-budget non-critical downgrades to task floor.
	{"over budget to haiku floor", 0.90, "crud", "low", 1.00, Haiku},
	{"over budget to sonnet floor", 0.95, "implementation", "high", 1.20, Sonnet},
	{"over budget opus task stays opus floor", 0.20, "architecture", "high", 1.00, Opus},
}

func TestTierForScore(t *testing.T) {
	for _, c := range tierForScoreCases {
		t.Run(c.name, func(t *testing.T) {
			got := TierForScore(c.score, c.taskType, c.risk, c.spendRatio)
			if got != c.want {
				t.Errorf("TierForScore(%v, %q, %q, %v) = %q, want %q",
					c.score, c.taskType, c.risk, c.spendRatio, got, c.want)
			}
		})
	}
}

// budgetAdjustCase is one BudgetAdjustTier expectation. Package scope (like
// tierForScoreCases) so the test body stays a thin loop — the budget bands are data.
type budgetAdjustCase struct {
	name       string
	base       string
	agent      string
	spendRatio float64
	want       string
}

// budgetAdjustCases pins the AGENT-pathway budget_guard (the near-budget down-tier that
// realizes routing's budget_guard semantics on an agent role instead of a task score).
// The boundaries mirror the score-path TierForScore cases: 0.80 is the inclusive gate, the
// opusFloor roles are the agent-side analogue of risk==critical (exempt), and an unset cap
// (ratio 0) leaves the routed tier byte-identical.
var budgetAdjustCases = []budgetAdjustCase{
	// In-budget: under the 0.80 gate nothing moves, whatever the agent/tier.
	{"in budget just under gate", Opus, "implementer", 0.79, Opus},       // 0.79 < 0.80 -> no change
	{"in budget mid", Sonnet, "implementer", 0.40, Sonnet},               // well in budget
	{"no cap ratio zero byte-identical", Opus, "implementer", 0.0, Opus}, // unset cap -> base verbatim

	// Near-budget boundary: 0.80 is INCLUSIVE — exactly at the gate it down-tiers.
	{"0.80 boundary downtiers", Opus, "implementer", 0.80, Sonnet},

	// Near-budget, non-floor agents drop exactly one tier.
	{"0.85 opus->sonnet non-floor", Opus, "implementer", 0.85, Sonnet},
	{"0.90 sonnet->haiku non-floor", Sonnet, "qa", 0.90, Haiku},

	// opusFloor roles are EXEMPT near budget — safety beats cost (the agent-side mirror of
	// a critical task staying Opus): reviewer/architect/cto keep their tier untouched.
	{"reviewer exempt near budget", Opus, "reviewer", 0.85, Opus},
	{"architect exempt high ratio", Opus, "architect", 0.95, Opus},
	{"cto exempt high ratio", Opus, "cto", 0.95, Opus},

	// downgradeOne clamps at Haiku: a haiku base near budget stays haiku (cannot go lower).
	{"haiku clamps at floor", Haiku, "harness", 0.90, Haiku},

	// NaN and negative spendRatio -> treated as "no cap/fully in budget" (ratio=0).
	{"NaN spendRatio treated as in budget", Opus, "implementer", math.NaN(), Opus},
	{"negative spendRatio treated as in budget", Opus, "implementer", -1.0, Opus},
	{"negative small spendRatio treated as in budget", Sonnet, "qa", -0.01, Sonnet},
}

// TestBudgetAdjustTier is the decisive boundary table for the near-budget down-tier: the
// 0.80 inclusive gate, the one-tier drop, the opusFloor (judgement-role) exemption, the
// Haiku clamp, and the ratio-0 byte-identical no-op. It proves BudgetAdjustTier reuses the
// SAME downgradeOne/opusFloorAgents as the routed/score paths — no second tier-math.
func TestBudgetAdjustTier(t *testing.T) {
	for _, c := range budgetAdjustCases {
		t.Run(c.name, func(t *testing.T) {
			if got := BudgetAdjustTier(c.base, c.agent, c.spendRatio); got != c.want {
				t.Errorf("BudgetAdjustTier(%q, %q, %v) = %q, want %q",
					c.base, c.agent, c.spendRatio, got, c.want)
			}
		})
	}
}

// FuzzTierForScore ensures TierForScore never panics on ANY input and always
// returns one of the four valid verdicts (Haiku / Sonnet / Opus / EscalateToHuman).
// Run: go test -fuzz=FuzzTierForScore -fuzztime=30s ./internal/routing/
func FuzzTierForScore(f *testing.F) {
	f.Add(float64(0.5), "implementation", "low", float64(0.0))
	f.Add(float64(math.NaN()), "", "", float64(math.NaN()))
	f.Add(float64(1e6), "security", "critical", float64(2.0))
	f.Fuzz(func(t *testing.T, score float64, taskType string, risk string, spendRatio float64) {
		got := TierForScore(score, taskType, risk, spendRatio)
		valid := got == Haiku || got == Sonnet || got == Opus || got == EscalateToHuman
		if !valid {
			t.Errorf("TierForScore(%v, %q, %q, %v) = %q (unexpected)", score, taskType, risk, spendRatio, got)
		}
	})
}

func TestFromChangedPathsDerivesAllDims(t *testing.T) {
	dims := FromChangedPaths([]string{
		"internal/payment/billing.go",
		"internal/payment/webhook.go",
		"internal/domain/models.go",
		"internal/auth/session.go",
		"db/migrations/0001_init.sql",
	})
	if dims["complexity"] != 0.625 {
		t.Fatalf("complexity = %v, want 0.625 (5 files/8)", dims["complexity"])
	}
	if dims["business_impact"] != 1 {
		t.Fatalf("business_impact = %v, want 1 (payment/auth/secrets)", dims["business_impact"])
	}
	if dims["risk"] < 0.5 {
		t.Fatalf("risk dim = %v, want >= 0.5 (high from sensitive surface)", dims["risk"])
	}
	if dims["dependency_change"] == 0 {
		t.Fatal("dependency dim must be non-zero (domain/schema/migration files)")
	}
	if dims["context_size"] == 0 {
		t.Fatal("context dim must be non-zero (multiple top-level domains)")
	}
}

func TestFromChangedPathsEmptyAndOrdinaryInputs(t *testing.T) {
	for _, paths := range [][]string{nil, {}} {
		dims := FromChangedPaths(paths)
		for _, dim := range []string{
			"complexity", "context_size", "dependency_change", "risk", "business_impact", "security",
		} {
			if dims[dim] != 0 {
				t.Fatalf("empty input must yield zero %s, got %v", dim, dims)
			}
		}
	}
	// A single non-sensitive file carries only its change-volume complexity.
	dims := FromChangedPaths([]string{"README.md"})
	if dims["complexity"] != 0.125 {
		t.Fatalf("single-file complexity = %v, want 0.125", dims["complexity"])
	}
	if dims["context_size"] != 0 || dims["dependency_change"] != 0 ||
		dims["risk"] != 0 || dims["business_impact"] != 0 {
		t.Fatalf("README must not trigger spread/dependency/risk dims, got %v", dims)
	}
}

func TestFromChangedPathsScoreReachesBands(t *testing.T) {
	weights := map[string]float64{
		"complexity": 1, "risk": 1, "business_impact": 1,
		"dependency_change": 1, "context_size": 1,
	}
	paths := []string{
		"internal/payment/billing.go", "internal/payment/webhook.go",
		"internal/domain/models.go", "internal/auth/session.go",
		"db/migrations/0001_init.sql",
	}
	score := Score(FromChangedPaths(paths), weights)
	if score < SonnetMax {
		t.Fatalf("sensitive multi-file diff score %v must reach at least the Sonnet band", score)
	}
}
