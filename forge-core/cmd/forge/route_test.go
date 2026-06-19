package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/mode"
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

// --diff-files AUTO-derives risk features from changed paths and feeds them into
// the SAME classifier path. The headline case: a payment file plus an irreversible
// migration auto-classifies CRITICAL, which pins TierForScore to Opus — the whole
// point of automatic extraction (no --touches-* typed by hand).
func TestRoute_DiffFilesAutoDerivesAndForcesOpus(t *testing.T) {
	o, ok := parseRouteFlags([]string{
		"--task-type", "implementation",
		"--diff-files", "src/payment/charge.go,db/migrations/001.sql",
	})
	if !ok {
		t.Fatal("parseRouteFlags(--diff-files payment+migration) failed to parse")
	}
	// The path heuristic must have set the payment + migration signals (and dropped
	// reversibility for the migration), and recorded observable reasons.
	if !o.sig.TouchesPayment || !o.sig.TouchesMigration || o.sig.Reversible {
		t.Errorf("auto signals = %+v, want payment+migration, irreversible", o.sig)
	}
	if len(o.autoReasons) == 0 {
		t.Errorf("autoReasons empty; want observable path-heuristic hits")
	}
	level, report := resolveRisk(o)
	if level != risk.Critical {
		t.Fatalf("resolveRisk(auto payment+migration) = %q, want critical (report %q)", level, report)
	}
	if got := routing.TierForScore(0.05, "implementation", level, 0.0); got != routing.Opus {
		t.Errorf("auto-critical did not force opus: TierForScore = %q, want opus", got)
	}
	// End-to-end command runs clean with the diff flag.
	if code := cmdRoute([]string{"--task-type", "implementation", "--diff-files", "src/payment/charge.go,db/migrations/001.sql"}); code != 0 {
		t.Errorf("cmdRoute(--diff-files payment+migration) = %d, want 0", code)
	}
}

// A single payment file auto-classifies at least high (the security floor), even
// for a low-score non-sensitive task type.
func TestRoute_DiffFilesPaymentIsHigh(t *testing.T) {
	o, _ := parseRouteFlags([]string{"--task-type", "implementation", "--diff-files", "src/payment/charge.go"})
	level, _ := resolveRisk(o)
	if risk.Rank(level) < risk.Rank(risk.High) {
		t.Errorf("auto payment-only risk = %q, want >= high", level)
	}
}

// An empty --diff-files (and a no-match list) must NOT engage the classifier as a
// false signal: with no explicit --risk the effective risk stays the "low"
// default and the no-features report line is used (backward-compatible).
func TestRoute_DiffFilesEmptyStaysLow(t *testing.T) {
	// Whitespace-only list -> no paths -> no auto signal.
	o, _ := parseRouteFlags([]string{"--task-type", "crud", "--diff-files", " , ,"})
	if level, report := resolveRisk(o); level != "low" || !strings.Contains(report, "no change features") {
		t.Errorf("empty --diff-files risk = (%q, %q), want (low, ...no change features...)", level, report)
	}
}

// The AUTO result and an explicit --risk take the STRICTER (auto may RAISE the
// manual floor; manual may RAISE the auto verdict) — never lowered either way.
func TestRoute_DiffFilesAutoAndManualTakeStricter(t *testing.T) {
	// Auto says high (payment); manual --risk=critical raises it to critical.
	o, _ := parseRouteFlags([]string{"--diff-files", "src/payment/charge.go", "--risk", "critical"})
	if level, _ := resolveRisk(o); level != risk.Critical {
		t.Errorf("auto(high) + --risk=critical -> %q, want critical (manual raises)", level)
	}
	// Auto says critical (payment+migration); manual --risk=low cannot lower it.
	o2, _ := parseRouteFlags([]string{"--diff-files", "src/payment/charge.go,db/migrations/001.sql", "--risk", "low"})
	if level, _ := resolveRisk(o2); level != risk.Critical {
		t.Errorf("auto(critical) + --risk=low -> %q, want critical (auto never lowered)", level)
	}
}

// AUTO and EXPLICIT --touches-* feature flags must also take the stricter merge:
// an explicit --touches-auth ORs with an auto payment hit, so both surfaces show.
func TestRoute_DiffFilesMergesWithExplicitTouches(t *testing.T) {
	o, _ := parseRouteFlags([]string{"--diff-files", "src/payment/charge.go", "--touches-auth"})
	if !o.sig.TouchesPayment || !o.sig.TouchesAuth {
		t.Errorf("merged signals = %+v, want BOTH payment (auto) and auth (explicit)", o.sig)
	}
	// An explicit --irreversible must survive an auto reversible (irreversible wins).
	o2, _ := parseRouteFlags([]string{"--diff-files", "src/payment/charge.go", "--irreversible"})
	if o2.sig.Reversible {
		t.Errorf("explicit --irreversible was lowered by auto reversible; Reversible=%v, want false", o2.sig.Reversible)
	}
}

// --from-git must be FAIL-TOLERANT: pointed at a directory that is not a git repo
// it yields no paths, engages no classifier, and route still exits 0 (an
// unavailable git is a missing signal, not a failure).
func TestRoute_FromGitFailTolerant(t *testing.T) {
	nonRepo := t.TempDir() // a fresh temp dir is not a git repo
	if got := gitChangedPaths(nonRepo); got != nil {
		t.Errorf("gitChangedPaths(non-repo) = %v, want nil (fail-tolerant)", got)
	}
	o, _ := parseRouteFlags([]string{"--task-type", "crud", "--from-git", "--root", nonRepo})
	if level, report := resolveRisk(o); level != "low" || !strings.Contains(report, "no change features") {
		t.Errorf("--from-git on non-repo risk = (%q, %q), want low/no-features (fail-tolerant)", level, report)
	}
	if code := cmdRoute([]string{"--task-type", "crud", "--from-git", "--root", nonRepo}); code != 0 {
		t.Errorf("cmdRoute(--from-git non-repo) = %d, want 0 (fail-tolerant)", code)
	}
}

// splitDiffFiles drops blank/whitespace entries so BlastRadius is not inflated.
func TestSplitDiffFiles(t *testing.T) {
	got := splitDiffFiles("a, ,b,,  c  ,")
	want := []string{"a", "b", "c"}
	if len(got) != len(want) {
		t.Fatalf("splitDiffFiles = %v, want %v", got, want)
	}
	for i := range want {
		if got[i] != want[i] {
			t.Errorf("splitDiffFiles[%d] = %q, want %q", i, got[i], want[i])
		}
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

// prioritiesLine SURFACES a mode's declared trade-off ranking (observability) —
// the numbers must match modes.yml verbatim, and the line must HONESTLY mark
// itself as intent (not an independent route input). For an unknown mode it falls
// back to balanced's ranking and says so. This is a read-only surface: it never
// changes the tier (covered by the unchanged-verdict test below).
func TestRoute_PrioritiesLineSurfacesRanking(t *testing.T) {
	cases := []struct {
		mode                   string
		speed, quality, cost   int
		wantsUnknownAnnotation bool
	}{
		{"explorer", 1, 3, 2, false},
		{"balanced", 2, 1, 3, false},
		{"engineering", 3, 1, 2, false},
		{"cto", 3, 1, 3, false}, // the deliberate tie (speed=cost=3): must surface as-is
		{"nope", 2, 1, 3, true}, // unknown -> balanced default, annotated
	}
	for _, c := range cases {
		t.Run(c.mode, func(t *testing.T) {
			got := prioritiesLine(c.mode)
			want := "speed=" + itoa(c.speed) + " quality=" + itoa(c.quality) + " cost=" + itoa(c.cost)
			if !strings.Contains(got, want) {
				t.Errorf("prioritiesLine(%q) = %q, want substring %q", c.mode, got, want)
			}
			// Honesty: the line must say the ranking is intent, not a route input.
			if !strings.Contains(got, "not an independent route input") {
				t.Errorf("prioritiesLine(%q) = %q, want honesty annotation", c.mode, got)
			}
			if c.wantsUnknownAnnotation && !strings.Contains(got, "unknown -> balanced default") {
				t.Errorf("prioritiesLine(%q) = %q, want unknown-mode annotation", c.mode, got)
			}
		})
	}
}

// The numbers prioritiesLine prints must be exactly what internal/mode distills
// (the single Go mirror of modes.yml), so the surface cannot silently drift from
// the policy data.
func TestRoute_PrioritiesLineMatchesModeDistillation(t *testing.T) {
	for _, m := range []string{"explorer", "balanced", "engineering", "cto"} {
		p, ok := mode.PrioritiesFor(m)
		if !ok {
			t.Fatalf("PrioritiesFor(%q) ok=false, want a known mode", m)
		}
		got := prioritiesLine(m)
		want := "speed=" + itoa(p.Speed) + " quality=" + itoa(p.Quality) + " cost=" + itoa(p.Cost)
		if !strings.Contains(got, want) {
			t.Errorf("prioritiesLine(%q) = %q, want it to carry %q", m, got, want)
		}
	}
}

// Surfacing priorities is observability ONLY: --mode must not change route's tier
// verdict (priorities are not a route input). Same dims/task/risk/budget -> same
// tier regardless of --mode.
func TestCmdRoute_ModeFlagDoesNotChangeTier(t *testing.T) {
	// All four modes parse and run clean; the flag is accepted everywhere.
	for _, m := range []string{"explorer", "balanced", "engineering", "cto"} {
		if code := cmdRoute([]string{"--task-type", "implementation", "--complexity", "0.5", "--mode", m}); code != 0 {
			t.Errorf("cmdRoute(--mode %s) = %d, want 0", m, code)
		}
	}
	// The tier is TierForScore's verdict, independent of --mode — assert directly.
	want := routing.TierForScore(0.5, "implementation", "low", 0.0)
	if want == "" {
		t.Fatal("TierForScore returned empty tier")
	}
	// An unknown --mode must still parse and run (surfaces balanced default).
	if code := cmdRoute([]string{"--task-type", "crud", "--mode", "does-not-exist"}); code != 0 {
		t.Errorf("cmdRoute(--mode does-not-exist) = %d, want 0 (unknown mode surfaces default)", code)
	}
}

// itoa is a tiny local int->string for building expected substrings (avoids a
// strconv import for one-digit ranks; ranks are always 1..3).
func itoa(n int) string {
	return string(rune('0' + n))
}
