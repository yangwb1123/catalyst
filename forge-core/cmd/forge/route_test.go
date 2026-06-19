package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/risk"
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

// --scorecard must parse on cmdRoute and a MISSING scorecard file must NOT be an
// error (cold start): route still exits 0 and its tier verdict is unchanged.
func TestCmdRoute_ScorecardFlagParsesAndColdStartIsOK(t *testing.T) {
	// Default path (no file in a temp cwd is unlikely to exist) still exits 0.
	if code := cmdRoute([]string{"--task-type", "implementation", "--complexity", "0.5"}); code != 0 {
		t.Errorf("route with default scorecard (cold start) = %d, want 0", code)
	}
	// Explicit, deliberately-absent path is a cold start, not a failure.
	missing := filepath.Join(t.TempDir(), "no-such-scorecards.json")
	if code := cmdRoute([]string{"--task-type", "security", "--complexity", "0.2", "--scorecard", missing}); code != 0 {
		t.Errorf("route with absent --scorecard (cold start) = %d, want 0", code)
	}
	// A non-existent flag is still a parse error (exit 2) — the new flag did not
	// loosen parsing.
	if code := cmdRoute([]string{"--scorecard", missing, "--bogus"}); code != 2 {
		t.Errorf("unknown flag must still be a parse error; got %d, want 2", code)
	}
}

// historyDecision applies policy.yml's history-tiebreak step ON TOP of the
// resolved tier. v1 is single-candidate (claude-only): the candidate set is just
// [tier]. These exercise every honest outcome — cold start (missing file),
// qualifying winner (enough samples), thin history (below min_samples), and an
// unreadable scorecard surfaced honestly — all keying off the SAME [tier] set.
func TestHistoryDecision_Outcomes(t *testing.T) {
	dir := t.TempDir()
	// A qualifying scorecard (>= historyMinSamples) for (sonnet, implementation)
	// plus a thin one for (opus, security).
	good := writeScorecard(t, dir, "good.json", `[
	  {"model":"sonnet","task_type":"implementation","quality_score":0.87,"samples":42,"updated_at":"2026-06-18T10:00:00Z"},
	  {"model":"opus","task_type":"security","quality_score":0.99,"samples":3,"updated_at":"2026-06-18T11:00:00Z"}
	]`)

	t.Run("cold start: missing file -> tier_default, tier echoed", func(t *testing.T) {
		picked, reason := historyDecision(routing.Sonnet, "implementation", filepath.Join(dir, "absent.json"))
		if picked != routing.Sonnet || !strings.Contains(reason, "no scorecard -> tier_default") {
			t.Errorf("cold start = (%q, %q), want (sonnet, no scorecard -> tier_default)", picked, reason)
		}
	})
	t.Run("qualifying winner: history observable", func(t *testing.T) {
		picked, reason := historyDecision(routing.Sonnet, "implementation", good)
		if picked != routing.Sonnet || !strings.Contains(reason, "picked sonnet by quality 0.87") || !strings.Contains(reason, "42 samples") {
			t.Errorf("qualifying = (%q, %q), want picked sonnet by quality 0.87 (42 samples)", picked, reason)
		}
	})
	t.Run("thin history: below min_samples -> tier_default", func(t *testing.T) {
		picked, reason := historyDecision(routing.Opus, "security", good)
		if picked != routing.Opus || !strings.Contains(reason, "insufficient samples") {
			t.Errorf("thin = (%q, %q), want (opus, insufficient samples ... tier_default)", picked, reason)
		}
	})
	t.Run("malformed scorecard: surfaced honestly, tier echoed", func(t *testing.T) {
		bad := writeScorecard(t, dir, "bad.json", "{not valid json")
		picked, reason := historyDecision(routing.Opus, "security", bad)
		if picked != routing.Opus || !strings.Contains(reason, "unreadable") {
			t.Errorf("malformed = (%q, %q), want (opus, scorecard unreadable ...)", picked, reason)
		}
	})
}

// writeScorecard writes a scorecards.json fixture under dir and returns its path.
func writeScorecard(t *testing.T, dir, name, body string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatalf("write scorecard %s: %v", path, err)
	}
	return path
}

// resolveRisk wires the risk CLASSIFIER into route: declared change features ->
// risk level -> the risk arg TierForScore consumes. The headline case is the one
// the whole feature exists for — an irreversible payment change classifies
// CRITICAL, and that level drives TierForScore to Opus (the safety_override hard
// floor that previously "waited for an input with no producer").
func TestResolveRisk_FeaturesDriveLevelAndForceOpus(t *testing.T) {
	// MEASURED end-to-end: parse the feature flags exactly as the CLI would,
	// resolve the risk, and feed it through the real TierForScore.
	o, ok := parseRouteFlags([]string{
		"--task-type", "implementation", "--touches-payment", "--irreversible",
	})
	if !ok {
		t.Fatal("parseRouteFlags(payment+irreversible) failed to parse")
	}
	level, report := resolveRisk(o)
	if level != risk.Critical {
		t.Fatalf("resolveRisk level = %q, want %q (report %q)", level, risk.Critical, report)
	}
	if !strings.Contains(report, "forces Opus") {
		t.Errorf("report = %q, want it to announce the Opus floor", report)
	}
	// The classified critical must actually pin TierForScore to Opus — a low score
	// on a non-sensitive task_type would otherwise be Sonnet/Haiku.
	if got := routing.TierForScore(0.05, "implementation", level, 0.0); got != routing.Opus {
		t.Errorf("TierForScore(low score, %q risk) = %q, want opus (safety_override)", level, got)
	}
	// And the command runs clean end-to-end with these flags.
	if code := cmdRoute([]string{"--task-type", "implementation", "--touches-payment", "--irreversible"}); code != 0 {
		t.Errorf("cmdRoute(payment+irreversible) = %d, want 0", code)
	}
}

// Backward compatibility: with NO feature flag, route's effective risk is just
// --risk (manual value or its "low" default) — the classifier never engages and
// behavior is unchanged from before this feature existed.
func TestResolveRisk_BackwardCompatNoFeatures(t *testing.T) {
	// No --risk, no features -> default "low", classifier untouched.
	o, _ := parseRouteFlags([]string{"--task-type", "crud"})
	if level, report := resolveRisk(o); level != "low" || !strings.Contains(report, "no change features") {
		t.Errorf("no-flags risk = (%q, %q), want (low, ...no change features...)", level, report)
	}
	// Manual --risk still honored verbatim when no features are supplied.
	o2, _ := parseRouteFlags([]string{"--task-type", "crud", "--risk", "high"})
	if level, _ := resolveRisk(o2); level != "high" {
		t.Errorf("manual --risk=high with no features -> %q, want high", level)
	}
}

// The manual --risk override may only RAISE the classifier's verdict, never lower
// it: features that classify high + an explicit --risk=critical -> critical;
// features that classify critical + an explicit --risk=low -> still critical.
func TestResolveRisk_OverrideRaisesNeverLowers(t *testing.T) {
	// Classifier says high (auth, reversible); manual --risk=critical raises it.
	o, _ := parseRouteFlags([]string{"--touches-auth", "--risk", "critical"})
	if level, _ := resolveRisk(o); level != risk.Critical {
		t.Errorf("auth(high) + --risk=critical -> %q, want critical (override raises)", level)
	}
	// Classifier says critical (payment+irreversible); manual --risk=low cannot lower it.
	o2, _ := parseRouteFlags([]string{"--touches-payment", "--irreversible", "--risk", "low"})
	if level, _ := resolveRisk(o2); level != risk.Critical {
		t.Errorf("payment+irreversible(critical) + --risk=low -> %q, want critical (never lowered)", level)
	}
}

// The risk-feature flags must parse cleanly and a bad value (non-int blast-radius)
// is still a parse error (exit 2) — the new flags did not loosen parsing.
func TestCmdRoute_RiskFeatureFlagsParse(t *testing.T) {
	if code := cmdRoute([]string{"--touches-migration", "--prod-traffic", "--blast-radius", "7"}); code != 0 {
		t.Errorf("cmdRoute(migration+prod+blast) = %d, want 0", code)
	}
	if code := cmdRoute([]string{"--blast-radius", "not-an-int"}); code != 2 {
		t.Errorf("cmdRoute(bad blast-radius) = %d, want 2", code)
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
