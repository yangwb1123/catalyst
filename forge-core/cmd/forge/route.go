package main

// forge route — expose internal/routing's multi-dimensional scorer as a
// runnable, inspectable command. It parses the six policy.yml dimensions plus
// the task_type / risk / budget context, computes routing.Score, resolves the
// tier with routing.TierForScore (the single source of truth for the tier),
// and prints an HONEST explanation of WHICH policy rule decided it.
//
// The explanation re-derives the deciding rule from the SAME policy numbers as
// TierForScore (bands, task-type floor, safety_override, budget_guard); the
// tier itself is never recomputed here — it is always TierForScore's verdict.

import (
	"flag"
	"fmt"

	"forgeos/forge-core/internal/risk"
	"forgeos/forge-core/internal/routing"
)

// defaultScorecardPath is where the Eval Engine writes the (model, task_type)
// performance history. A MISSING file here is a cold start, not an error
// (policy.history.on_missing == tier_default), so the default is always safe to
// read even before any loop history exists.
const defaultScorecardPath = ".agent/routing/scorecards.json"

// historyMinSamples mirrors policy.history.min_samples: a scorecard must have at
// least this many observations before its quality_score is trusted to break a
// tie. Below it, history is too thin and the tier default passes through.
const historyMinSamples = 20

// dimWeights mirrors policy.yml dimensions[*].weight (Σ = 1.0). These are the
// keys Score reads from the dims map, so the flag-built map must use them.
var dimWeights = map[string]float64{
	"complexity":        0.25,
	"risk":              0.25,
	"dependency_change": 0.12,
	"security":          0.18,
	"context_size":      0.10,
	"business_impact":   0.10,
}

// routeOpts holds the parsed `forge route` flags.
type routeOpts struct {
	complexity, riskScore, security float64
	dependency, context, business   float64
	taskType, risk                  string
	budget                          float64
	scorecard                       string
	sig                             risk.Signals // declared change features (risk classifier input)
	riskSetByUser                   bool         // --risk explicitly passed (vs. the "low" default)
	sigSetByUser                    bool         // any --touches-*/--prod-traffic/--irreversible/--blast-radius passed
}

// cmdRoute parses the routing flags, scores the task, resolves the tier, and
// prints the score + tier + deciding rule. Returns 0 on success, 2 on a flag
// error, so it composes with the run() dispatch like the other subcommands.
func cmdRoute(args []string) int {
	o, ok := parseRouteFlags(args)
	if !ok {
		return 2
	}
	dims := map[string]float64{
		"complexity":        o.complexity,
		"risk":              o.riskScore,
		"security":          o.security,
		"dependency_change": o.dependency,
		"context_size":      o.context,
		"business_impact":   o.business,
	}
	effRisk, riskLine := resolveRisk(o)
	score := routing.Score(dims, dimWeights)
	tier := routing.TierForScore(score, o.taskType, effRisk, o.budget)
	rule := decidingRule(score, o.taskType, effRisk, o.budget)
	fmt.Printf("forge route: score=%.4f tier=%s decided-by=%s\n", score, tier, rule)
	fmt.Printf("  task-type=%q risk=%q budget(spend-ratio)=%.2f\n", o.taskType, effRisk, o.budget)
	fmt.Printf("  %s\n", riskLine)
	picked, reason := historyDecision(tier, o.taskType, o.scorecard)
	fmt.Printf("  history: model=%s — %s\n", picked, reason)
	return 0
}

// resolveRisk turns the parsed options into the EFFECTIVE risk level fed to
// TierForScore, plus a one-line honest report of how it was decided.
//
// Backward compatibility is the contract: when NO feature flag is given, route
// behaves exactly as before — the effective risk is simply --risk (the manual
// value or its "low" default). When feature flags ARE given, the classifier
// computes a level from them and route takes the HIGHER of (classifier, manual
// --risk): an explicit --risk can only RAISE the floor, never silently lower a
// risk the declared signals say is greater.
func resolveRisk(o routeOpts) (level, report string) {
	if !o.sigSetByUser {
		return o.risk, fmt.Sprintf("risk: %s (manual --risk; no change features supplied)", o.risk)
	}
	classified, reason := risk.Classify(o.sig)
	eff := classified
	note := reason
	if o.riskSetByUser {
		eff = risk.Higher(classified, o.risk)
		note = fmt.Sprintf("%s; manual --risk=%s -> using higher: %s", reason, o.risk, eff)
	}
	return eff, fmt.Sprintf("risk: %s%s", note, forcesOpusSuffix(eff))
}

// forcesOpusSuffix appends the visible safety-floor consequence when the
// resolved risk is critical — the whole point of the classifier is to feed
// TierForScore's hard "risk == critical -> Opus" rule, so we say so out loud.
func forcesOpusSuffix(level string) string {
	if level == risk.Critical {
		return " -> forces Opus (safety_override)"
	}
	return ""
}

// historyDecision adds policy.yml's final decision step (history-tiebreak) ON
// TOP of the resolved tier, and prints it as an observable, honest report line.
//
// HONEST v1 scope (claude-only, policy.yml D4): each tier band holds a SINGLE
// candidate model, so the candidate set built here is [tier] — one element. With
// one candidate there is no real shoot-out: HistoryTiebreak either echoes that
// model (its scorecard merely made observable) or falls back to the same tier
// default. What this buys today is decision-chain completeness (the chain no
// longer silently drops its history tail) plus full observability of the scored
// history — the genuine multi-candidate选优 is v3's cross-vendor pool. A missing
// scorecard file is a cold start, NOT an error, so route's verdict is unchanged
// from today and we only add one "no scorecard -> tier_default" line.
//
// LoadScorecards is the only fallible step; a load error is surfaced honestly as
// the reason (the tier model still passes through) rather than silently dropped.
func historyDecision(tier, taskType, path string) (picked, reason string) {
	cards, err := routing.LoadScorecards(path)
	if err != nil {
		return tier, fmt.Sprintf("scorecard unreadable (%v) -> tier_default", err)
	}
	// v1 single-candidate set: the tier name IS the claude-only model for the band.
	return routing.HistoryTiebreak([]string{tier}, taskType, cards, historyMinSamples)
}

// parseRouteFlags binds the dimension/context flags and parses args. The six
// dimensions are 0..1 floats; budget is a spend ratio (0..1+). The risk-feature
// flags (--touches-*, --prod-traffic, --irreversible, --blast-radius) feed the
// risk classifier; whether ANY was set is recorded so the no-feature path stays
// byte-for-byte backward compatible (manual --risk only). Returns false on a
// parse error (flag pkg already printed the diagnostic + usage).
func parseRouteFlags(args []string) (routeOpts, bool) {
	fs := flag.NewFlagSet("route", flag.ContinueOnError)
	var o routeOpts
	var irreversible bool
	fs.Float64Var(&o.complexity, "complexity", 0, "complexity dimension 0..1")
	fs.Float64Var(&o.riskScore, "risk-score", 0, "risk (blast-radius) dimension 0..1")
	fs.Float64Var(&o.security, "security", 0, "security-surface dimension 0..1")
	fs.Float64Var(&o.dependency, "dependency", 0, "dependency-change dimension 0..1")
	fs.Float64Var(&o.context, "context", 0, "context-size dimension 0..1")
	fs.Float64Var(&o.business, "business", 0, "business-impact dimension 0..1")
	fs.StringVar(&o.taskType, "task-type", "", "task type (e.g. crud|implementation|security|payment|architecture)")
	fs.StringVar(&o.risk, "risk", "low", "manual risk override: low|medium|high|critical (raised to classifier verdict if features given)")
	fs.Float64Var(&o.budget, "budget", 0, "spend ratio (cumulative_spend/budget_cap), 0..1+")
	fs.StringVar(&o.scorecard, "scorecard", defaultScorecardPath, "Eval Engine scorecards.json for history-tiebreak (missing file = cold start)")
	bindRiskFeatureFlags(fs, &o.sig, &irreversible)
	if err := fs.Parse(args); err != nil {
		return routeOpts{}, false
	}
	o.sig.Reversible = !irreversible // policy signal is reversibility; flag is its negation
	recordRiskFlagOrigins(fs, &o)
	return o, true
}

// bindRiskFeatureFlags binds the change-feature flags that feed the risk
// classifier. --irreversible is the negation of the Reversible signal (so the
// zero/default change is reversible = the safe assumption to LOWER risk).
func bindRiskFeatureFlags(fs *flag.FlagSet, sig *risk.Signals, irreversible *bool) {
	fs.BoolVar(&sig.TouchesPayment, "touches-payment", false, "change reaches payment/billing code")
	fs.BoolVar(&sig.TouchesAuth, "touches-auth", false, "change reaches authn/authz code")
	fs.BoolVar(&sig.TouchesSecrets, "touches-secrets", false, "change reaches secrets/credentials")
	fs.BoolVar(&sig.TouchesMigration, "touches-migration", false, "change includes a schema/data migration")
	fs.BoolVar(&sig.ProdTraffic, "prod-traffic", false, "change is exercised by production traffic")
	fs.BoolVar(irreversible, "irreversible", false, "change cannot be cleanly rolled back")
	fs.IntVar(&sig.BlastRadius, "blast-radius", 0, "count of affected modules/files")
}

// riskFeatureFlags are the flag names that, if any is set, switch route into
// classifier mode. Kept beside bindRiskFeatureFlags so the two cannot drift.
var riskFeatureFlags = map[string]bool{
	"touches-payment": true, "touches-auth": true, "touches-secrets": true,
	"touches-migration": true, "prod-traffic": true, "irreversible": true, "blast-radius": true,
}

// recordRiskFlagOrigins inspects which flags the user actually passed (flag.Visit
// reports only set flags), so route can tell a deliberate --risk / feature flag
// from a default. This is what keeps the no-feature path backward compatible.
func recordRiskFlagOrigins(fs *flag.FlagSet, o *routeOpts) {
	fs.Visit(func(f *flag.Flag) {
		if f.Name == "risk" {
			o.riskSetByUser = true
		}
		if riskFeatureFlags[f.Name] {
			o.sigSetByUser = true
		}
	})
}

// taskTypeFloor mirrors policy.yml tiers.by_task_type — the EXACT same floors
// internal/routing applies — used only to NAME the deciding rule, never to pick
// the tier (that stays TierForScore's job).
var taskTypeFloor = map[string]string{
	"docs": routing.Haiku, "crud": routing.Haiku, "test": routing.Haiku,
	"implementation": routing.Sonnet, "refactor_medium": routing.Sonnet, "bugfix": routing.Sonnet,
	"architecture": routing.Opus, "security": routing.Opus, "payment": routing.Opus,
	"authorization": routing.Opus, "requirements": routing.Opus, "reviewer": routing.Opus,
}

// safetyForceOpus mirrors policy.yml safety_override.rules task_types.
var safetyForceOpus = map[string]bool{"security": true, "payment": true, "authorization": true}

// rank orders tiers so the explanation can tell whether the floor or the band
// is the higher (deciding) one — same ordering as routing.higher.
var rank = map[string]int{routing.Haiku: 0, routing.Sonnet: 1, routing.Opus: 2}

// decidingRule names WHICH policy rule produced TierForScore's verdict, applying
// the same precedence as TierForScore: safety_override -> budget_guard ->
// task-type floor -> score band. It is explanatory only.
func decidingRule(score float64, taskType, risk string, spend float64) string {
	critical := risk == "critical"
	if spend >= 1.00 && critical {
		return "budget_guard (escalate_to_human: critical & over budget)"
	}
	if critical {
		return "safety_override (risk=critical forces opus)"
	}
	if safetyForceOpus[taskType] {
		return fmt.Sprintf("safety_override (task-type %q forces opus)", taskType)
	}
	if spend >= 1.00 {
		return "budget_guard (over budget: downgrade to task-type floor)"
	}
	if spend >= 0.80 {
		return "budget_guard (near budget: downgrade one tier)"
	}
	return bandOrFloor(score, taskType)
}

// bandOrFloor decides, for the no-override / in-budget path, whether the score
// band or the task-type floor is the higher (deciding) rule, naming whichever
// won — exactly the higher() choice TierForScore makes.
func bandOrFloor(score float64, taskType string) string {
	band := bandForScore(score)
	floor := taskTypeFloor[taskType]
	if rank[floor] > rank[band] {
		return fmt.Sprintf("task-type floor (%s for %q beats score band %s)", floor, taskType, band)
	}
	return fmt.Sprintf("score band (%s for score=%.4f)", band, score)
}

// bandForScore mirrors policy.yml scoring.thresholds (haiku_max=0.34,
// sonnet_max=0.69) — the same banding internal/routing uses.
func bandForScore(score float64) string {
	switch {
	case score <= 0.34:
		return routing.Haiku
	case score <= 0.69:
		return routing.Sonnet
	default:
		return routing.Opus
	}
}
