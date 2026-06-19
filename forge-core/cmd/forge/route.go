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

	"forgeos/forge-core/internal/routing"
)

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
	score := routing.Score(dims, dimWeights)
	tier := routing.TierForScore(score, o.taskType, o.risk, o.budget)
	rule := decidingRule(score, o.taskType, o.risk, o.budget)
	fmt.Printf("forge route: score=%.4f tier=%s decided-by=%s\n", score, tier, rule)
	fmt.Printf("  task-type=%q risk=%q budget(spend-ratio)=%.2f\n", o.taskType, o.risk, o.budget)
	return 0
}

// parseRouteFlags binds the dimension/context flags and parses args. The six
// dimensions are 0..1 floats; budget is a spend ratio (0..1+). Returns false on
// a parse error (flag pkg already printed the diagnostic + usage).
func parseRouteFlags(args []string) (routeOpts, bool) {
	fs := flag.NewFlagSet("route", flag.ContinueOnError)
	var o routeOpts
	fs.Float64Var(&o.complexity, "complexity", 0, "complexity dimension 0..1")
	fs.Float64Var(&o.riskScore, "risk-score", 0, "risk (blast-radius) dimension 0..1")
	fs.Float64Var(&o.security, "security", 0, "security-surface dimension 0..1")
	fs.Float64Var(&o.dependency, "dependency", 0, "dependency-change dimension 0..1")
	fs.Float64Var(&o.context, "context", 0, "context-size dimension 0..1")
	fs.Float64Var(&o.business, "business", 0, "business-impact dimension 0..1")
	fs.StringVar(&o.taskType, "task-type", "", "task type (e.g. crud|implementation|security|payment|architecture)")
	fs.StringVar(&o.risk, "risk", "low", "risk level: low|medium|high|critical")
	fs.Float64Var(&o.budget, "budget", 0, "spend ratio (cumulative_spend/budget_cap), 0..1+")
	if err := fs.Parse(args); err != nil {
		return routeOpts{}, false
	}
	return o, true
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
