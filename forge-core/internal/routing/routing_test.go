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
