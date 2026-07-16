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
	"os/exec"
	"strings"

	"forgeos/forge-core/internal/gate"
	"forgeos/forge-core/internal/mode"
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
	mode                            string // --mode: engineering posture, for surfacing its priorities (observability)
	budget                          float64
	scorecard                       string
	sig                             risk.Signals // declared change features (risk classifier input)
	riskSetByUser                   bool         // --risk explicitly passed (vs. the "low" default)
	sigSetByUser                    bool         // any --touches-*/--prod-traffic/--irreversible/--blast-radius passed
	diffFiles                       string       // --diff-files: comma-separated explicit changed-path list
	fromGit                         bool         // --from-git: derive changed paths from `git diff --name-only HEAD`
	root                            string       // --root: repo root for --from-git (gate.RepoRoot resolution)
	autoReasons                     []string     // human-readable hit reasons from FromChangedPaths (observability)
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
	if len(o.autoReasons) > 0 {
		fmt.Printf("  auto-features (path heuristic): %s\n", strings.Join(o.autoReasons, "; "))
	}
	fmt.Printf("  %s\n", riskLine)
	fmt.Printf("  %s\n", prioritiesLine(o.mode))
	picked, reason := historyDecision(tier, o.taskType, o.scorecard)
	fmt.Printf("  history: model=%s — %s\n", picked, reason)
	return 0
}

// prioritiesLine renders the --mode's declared trade-off ranking as an observable
// report line. HONESTY: priorities are surfaced here ONLY to make a mode's intent
// inspectable — they did NOT drive this route's tier. Their effect is already
// carried upstream by router_default_tier + gates + evolve depth (see
// internal/mode.Priorities); an independent "priorities → route weighting"
// semantics is undeclared in modes.yml and deliberately not wired. An unknown mode
// surfaces balanced's ranking (the selector default) and says so.
func prioritiesLine(modeName string) string {
	p, ok := mode.PrioritiesFor(modeName)
	suffix := " (trade-off intent; already reflected in tier/gates/evolve — not an independent route input)"
	if !ok {
		return fmt.Sprintf("priorities[mode=%q unknown -> balanced default]: "+
			"speed=%d quality=%d cost=%d%s", modeName, p.Speed, p.Quality, p.Cost, suffix)
	}
	return fmt.Sprintf("priorities[mode=%s]: speed=%d quality=%d cost=%d%s",
		modeName, p.Speed, p.Quality, p.Cost, suffix)
}

// resolveRisk turns the parsed options into the EFFECTIVE risk level fed to
// TierForScore, plus a one-line honest report of how it was decided.
//
// Backward compatibility is the contract: when NO feature flag is given (neither
// an explicit --touches-* nor a changed-path set via --diff-files/--from-git),
// route behaves exactly as before — the effective risk is simply --risk (the
// manual value or its "low" default). When features ARE supplied — whether typed
// explicitly OR auto-derived from changed paths by applyDiffSignals — the
// classifier computes a level from o.sig and route takes the HIGHER of
// (classifier, manual --risk): an explicit --risk can only RAISE the floor, never
// silently lower a risk the declared/derived signals say is greater.
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
// v1.5 multi-candidate scope: for non-safety-floor task types, the candidate set is
// routing.CandidatesForTier(tier) = [tier, ...cheaper]. HistoryTiebreak picks the
// highest-quality qualifying model; a cheaper model wins only when it has sufficient
// samples AND better quality than tier. Cold start → falls back to tier (candidates[0]).
// This makes `forge route` reflect the actual learning-loop routing decision.
//
// LoadScorecards is the only fallible step; a load error is surfaced honestly as
// the reason (the tier model still passes through) rather than silently dropped.
func historyDecision(tier, taskType, path string) (picked, reason string) {
	cards, err := routing.LoadScorecards(path)
	if err != nil {
		return tier, fmt.Sprintf("scorecard unreadable (%v) -> tier_default", err)
	}
	return routing.HistoryTiebreak(routing.CandidatesForTier(tier), taskType, cards, historyMinSamples)
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
	fs.StringVar(&o.mode, "mode", "balanced", "engineering posture (explorer|balanced|engineering|cto); surfaces its priorities — observability only")
	fs.StringVar(&o.risk, "risk", "low", "manual risk override: low|medium|high|critical (raised to classifier verdict if features given)")
	fs.Float64Var(&o.budget, "budget", 0, "spend ratio (cumulative_spend/budget_cap), 0..1+")
	fs.StringVar(&o.scorecard, "scorecard", defaultScorecardPath, "Eval Engine scorecards.json for history-tiebreak (missing file = cold start)")
	fs.StringVar(&o.diffFiles, "diff-files", "", "comma-separated changed-path list; auto-derives risk features (path heuristic)")
	fs.BoolVar(&o.fromGit, "from-git", false, "derive changed paths from `git -C <root> diff --name-only HEAD` (fail-tolerant)")
	fs.StringVar(&o.root, "root", "", "repo root for --from-git (else $FORGE_REPO_ROOT, else cwd)")
	bindRiskFeatureFlags(fs, &o.sig, &irreversible)
	if err := fs.Parse(args); err != nil {
		return routeOpts{}, false
	}
	o.sig.Reversible = !irreversible // policy signal is reversibility; flag is its negation
	recordRiskFlagOrigins(fs, &o)
	applyDiffSignals(&o)
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

// applyDiffSignals AUTOMATICALLY derives risk features from the changed-path set
// (--diff-files and/or --from-git) and folds them into o.sig. It is the bridge
// from risk.FromChangedPaths into route's existing classifier path: when paths
// are present it merges the auto Signals with any explicit --touches-* flags by
// taking the STRICTER of the two (raise-only — see mergeAutoSignals), flips
// sigSetByUser so resolveRisk engages the classifier, and records the hit reasons
// for the report. With neither flag given it is a no-op, so route stays byte-for-
// byte backward compatible. Auto can only RAISE risk, never lower a declared one.
func applyDiffSignals(o *routeOpts) {
	if o.diffFiles == "" && !o.fromGit {
		return
	}
	paths := changedPaths(o)
	if len(paths) == 0 {
		return // no changed paths resolved (empty list / no git / no diff) -> no auto signal
	}
	auto, reasons := risk.FromChangedPaths(paths)
	o.sig = mergeAutoSignals(o.sig, auto)
	o.autoReasons = reasons
	o.sigSetByUser = true // a changed-path set was supplied -> classifier mode
}

// changedPaths assembles the changed-path set from --diff-files (a comma list,
// blanks trimmed) and --from-git (the working tree's diff vs HEAD). Both sources
// are unioned; either may be empty without error.
func changedPaths(o *routeOpts) []string {
	var paths []string
	paths = append(paths, splitDiffFiles(o.diffFiles)...)
	if o.fromGit {
		paths = append(paths, gitChangedPaths(o.root)...)
	}
	return paths
}

// splitDiffFiles parses a comma-separated path list, dropping empty/whitespace
// entries so "a, ,b," yields [a b] rather than blanks that would inflate
// BlastRadius.
func splitDiffFiles(csv string) []string {
	var out []string
	for _, p := range strings.Split(csv, ",") {
		if p = strings.TrimSpace(p); p != "" {
			out = append(out, p)
		}
	}
	return out
}

// gitChangedPaths returns the files changed in the working tree relative to HEAD
// via `git -C <root> diff --name-only HEAD`. FAIL-TOLERANT BY DESIGN: outside a
// git repo, with git absent, or with no changes, it returns an empty slice and
// never errors — an unavailable git is a missing signal (auto under-states, the
// human overrides), not a route failure.
func gitChangedPaths(root string) []string {
	out, err := exec.Command("git", "-C", gate.RepoRoot(root), "diff", "--name-only", "HEAD").Output()
	if err != nil {
		return nil
	}
	return splitLines(string(out))
}

// splitLines returns the non-blank lines of s, trimmed — git emits one path per
// line with a trailing newline.
func splitLines(s string) []string {
	var out []string
	for _, l := range strings.Split(s, "\n") {
		if l = strings.TrimSpace(l); l != "" {
			out = append(out, l)
		}
	}
	return out
}

// mergeAutoSignals folds AUTO-derived Signals into the EXPLICIT ones, taking the
// STRICTER value field-by-field so the merge can only RAISE risk: booleans OR
// (any "true" wins), BlastRadius takes the max, and Reversible takes the AND
// (irreversible from EITHER source wins — the risk-raising direction). ProdTraffic
// thus stays true only if the human declared it; auto never sets it (a path can't
// prove prod exposure). This is the same "raise, never lower" rule resolveRisk
// applies between the classifier and manual --risk, here applied between the
// auto and manual FEATURE sources.
func mergeAutoSignals(explicit, auto risk.Signals) risk.Signals {
	return risk.Signals{
		TouchesPayment:   explicit.TouchesPayment || auto.TouchesPayment,
		TouchesAuth:      explicit.TouchesAuth || auto.TouchesAuth,
		TouchesSecrets:   explicit.TouchesSecrets || auto.TouchesSecrets,
		TouchesMigration: explicit.TouchesMigration || auto.TouchesMigration,
		ProdTraffic:      explicit.ProdTraffic || auto.ProdTraffic,
		Reversible:       explicit.Reversible && auto.Reversible,
		BlastRadius:      maxInt(explicit.BlastRadius, auto.BlastRadius),
	}
}

// maxInt returns the larger of two ints (BlastRadius merge).
func maxInt(a, b int) int {
	if a > b {
		return a
	}
	return b
}

// taskTypeFloor and safetyForceOpus are imported from routing. route.go no
// longer maintains its own copies — the routing package is the single source
// of truth for policy thresholds, task-type floors, safety pins, and tier rank.

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
	if routing.SafetyForceOpus[taskType] {
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
// won — exactly the higher() choice TierForScore makes. All policy data is
// imported from routing (the single source of truth).
func bandOrFloor(score float64, taskType string) string {
	band := routing.BandForScore(score)
	floor := routing.TaskTypeFloor[taskType]
	if routing.Rank[floor] > routing.Rank[band] {
		return fmt.Sprintf("task-type floor (%s for %q beats score band %s)", floor, taskType, band)
	}
	return fmt.Sprintf("score band (%s for score=%.4f)", band, score)
}
