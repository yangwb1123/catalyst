package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/routing"
)

// cmdRoute must exit 0 on valid input (the demo case: a security task with a
// low complexity still routes — and routes to opus via the safety floor).
func TestCmdRoute_ValidInputReturnsZero(t *testing.T) {
	if code := cmdRoute([]string{"--task-type", "security", "--complexity", "0.2"}); code != 0 {
		t.Errorf("cmdRoute(security, complexity=0.2) = %d, want 0", code)
	}
}

// A bad flag must surface as exit 2 (flag parse error), not a silent 0.
func TestCmdRoute_BadFlagReturnsTwo(t *testing.T) {
	if code := cmdRoute([]string{"--complexity", "not-a-float"}); code != 2 {
		t.Errorf("cmdRoute(bad float) = %d, want 2", code)
	}
}

// The decision logic the CLI prints must agree with TierForScore — and with the
// policy rules the prompt calls out: a security/payment task is opus regardless
// of score (safety floor), a high score is opus by band, a low score low-risk
// task is haiku. These exercise the SAME tier verdict the command prints.
func TestRoute_TierDecisions(t *testing.T) {
	cases := []struct {
		name     string
		score    float64
		taskType string
		risk     string
		budget   float64
		want     string
	}{
		// safety floor: security/payment force opus even from a near-zero score.
		{"security forces opus regardless of score", 0.02, "security", "low", 0.0, routing.Opus},
		{"payment forces opus regardless of score", 0.05, "payment", "low", 0.0, routing.Opus},
		// high score bands straight to opus (in-budget, no override).
		{"high score opus band", 0.90, "implementation", "low", 0.0, routing.Opus},
		// low score, low risk, cheap task -> haiku.
		{"low score low risk haiku", 0.10, "crud", "low", 0.0, routing.Haiku},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			if got := routing.TierForScore(c.score, c.taskType, c.risk, c.budget); got != c.want {
				t.Errorf("TierForScore(%v, %q, %q, %v) = %q, want %q",
					c.score, c.taskType, c.risk, c.budget, got, c.want)
			}
		})
	}
}

// decidingRule must NAME the rule that produced the tier, honestly, following
// TierForScore's precedence (safety_override -> budget_guard -> floor -> band).
func TestRoute_DecidingRuleNames(t *testing.T) {
	cases := []struct {
		name             string
		score            float64
		taskType, risk   string
		budget           float64
		wantRuleContains string
	}{
		{"security -> safety_override", 0.20, "security", "low", 0.0, "safety_override"},
		{"critical -> safety_override", 0.10, "crud", "critical", 0.0, "safety_override"},
		{"critical over budget -> escalate", 0.90, "implementation", "critical", 1.0, "escalate_to_human"},
		{"near budget -> budget_guard", 0.50, "implementation", "low", 0.85, "budget_guard"},
		{"over budget -> budget_guard floor", 0.90, "crud", "low", 1.0, "budget_guard"},
		{"high score -> score band", 0.90, "implementation", "low", 0.0, "score band"},
		{"low score cheap task -> score band", 0.10, "crud", "low", 0.0, "score band"},
		{"low score implementation -> task-type floor", 0.10, "implementation", "low", 0.0, "task-type floor"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			got := decidingRule(c.score, c.taskType, c.risk, c.budget)
			if !strings.Contains(got, c.wantRuleContains) {
				t.Errorf("decidingRule(%v, %q, %q, %v) = %q, want substring %q",
					c.score, c.taskType, c.risk, c.budget, got, c.wantRuleContains)
			}
		})
	}
}
