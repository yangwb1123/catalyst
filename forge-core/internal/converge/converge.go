// Package converge evaluates a workflow's declarative stop condition against
// live repository signals — turning forge-core's stop handling from "declared"
// into a real convergence check. ForgeOS forbids round-count termination, so a
// workflow converges only when its actual criteria (roadmap completion, gate
// state) are met, never after N iterations.
package converge

import (
	"fmt"
	"strings"

	"forgeos/forge-core/internal/asset"
)

// Signals are the live measurements a stop condition is evaluated against.
type Signals struct {
	RoadmapCompletion float64 // fraction in [0,1] of decided checklist items done
	GatesGreen        bool    // every required harness gate currently passes

	// Criteria carries per-criterion acceptance verdicts (criterion name ->
	// "PASS"|"FAIL"|"NA", exactly as gate.ProbeAll emits them) so a workflow can
	// converge on a single acceptance criterion (e.g. test_pass) instead of only
	// the coarse GatesGreen aggregate. It reuses the SAME ProbeAll result the
	// acceptance gate already ran — no extra spawn. A nil map means no probe data
	// is wired, and every per-criterion check then degrades safely to unmet
	// (honest: absence of a verdict is never satisfaction).
	Criteria map[string]string
}

// acceptanceMetrics is the set of metric names that evalOne resolves from
// Signals.Criteria (the load-bearing acceptance criteria plus the governance
// ones). A metric outside this set keeps its prior dispatch (roadmap/gates) or
// falls through to the honest unknown->unmet default.
var acceptanceMetrics = map[string]bool{
	"test_pass":             true,
	"app_test_pass":         true,
	"architecture":          true,
	"arch_violations":       true,
	"complexity_violations": true,
}

// Result is one evaluated criterion and whether the live signals satisfy it.
type Result struct {
	Expr   string // human-readable rendering of the criterion
	Met    bool
	Detail string
}

// Evaluate checks each typed criterion (a conjunction) against signals and
// reports per-criterion results plus whether ALL are met. An unknown metric is
// reported as NOT met (never silently passed) so convergence cannot be faked.
// Zero criteria is NOT convergence — nothing has been proven.
func Evaluate(allOf []asset.Criterion, sig Signals) (results []Result, allMet bool) {
	allMet = len(allOf) > 0
	for _, c := range allOf {
		r := evalOne(c, sig)
		results = append(results, r)
		if !r.Met {
			allMet = false
		}
	}
	return results, allMet
}

// evalOne dispatches a single criterion on its Metric. Only recognized metrics
// can be Met; an unknown metric is unmet-by-default (honest, never a silent pass).
func evalOne(c asset.Criterion, sig Signals) Result {
	switch {
	case c.Metric == "roadmap_completion":
		return evalRoadmap(c, sig)
	case c.Metric == "gates_status":
		met := c.Value == "green" && sig.GatesGreen
		return Result{render(c), met, greenDetail(sig.GatesGreen)}
	case acceptanceMetrics[c.Metric]:
		return evalCriterion(c, sig)
	default:
		return Result{render(c), false, unknownDetail(c)}
	}
}

// evalCriterion resolves a known acceptance criterion (test_pass, architecture,
// …) from the per-criterion verdicts in Signals.Criteria. Only an explicit PASS
// is Met; FAIL, NA, or an absent verdict are all unmet. This mirrors
// acceptance.decide's honesty law: NA means "not actually checked", so it can
// never count as satisfied, and a criterion the probe didn't report cannot be
// assumed green.
func evalCriterion(c asset.Criterion, sig Signals) Result {
	status, ok := sig.Criteria[c.Metric]
	if !ok {
		return Result{render(c), false, fmt.Sprintf("%s: no verdict (probe absent) — treated as unmet", c.Metric)}
	}
	met := status == "PASS"
	return Result{render(c), met, fmt.Sprintf("%s=%s", c.Metric, status)}
}

// evalRoadmap compares roadmap completion (as a percentage) against the
// criterion threshold using its operator. A missing threshold cannot be met.
func evalRoadmap(c asset.Criterion, sig Signals) Result {
	pct := sig.RoadmapCompletion * 100
	detail := fmt.Sprintf("roadmap_completion=%.0f%%", pct)
	if c.Threshold == nil {
		return Result{render(c), false, detail + " (no threshold given)"}
	}
	met := compare(pct, c.Operator, *c.Threshold)
	return Result{render(c), met, detail}
}

// compare applies a comparison operator. An unknown operator yields false so an
// unparseable criterion is never silently satisfied.
func compare(left float64, op string, right float64) bool {
	switch op {
	case "==":
		return left == right
	case ">=":
		return left >= right
	case "<=":
		return left <= right
	case ">":
		return left > right
	case "<":
		return left < right
	default:
		return false
	}
}

// render produces a stable human-readable string for a criterion: the bare Raw
// string when the criterion was authored as one, else metric/operator/value.
func render(c asset.Criterion) string {
	if c.Raw != "" {
		return c.Raw
	}
	if c.Threshold != nil {
		return fmt.Sprintf("%s %s %g", c.Metric, c.Operator, *c.Threshold)
	}
	return strings.TrimSpace(fmt.Sprintf("%s %s %s", c.Metric, c.Operator, c.Value))
}

func unknownDetail(c asset.Criterion) string {
	if c.Raw != "" {
		return "unrecognized criterion (bare string, no metric) — treated as unmet"
	}
	return fmt.Sprintf("unknown metric %q — treated as unmet", c.Metric)
}

func greenDetail(green bool) string {
	if green {
		return "all required gates green"
	}
	return "a required gate is not green"
}

// RoadmapCompletion returns the fraction of decided checklist items ([x] of
// [x]+[ ]+[~]) in a ROADMAP markdown. Partial items ([~]) count as not done.
// With no checklist items it returns 0 (nothing proven complete).
func RoadmapCompletion(markdown string) float64 {
	done, total := 0, 0
	for _, line := range strings.Split(markdown, "\n") {
		switch t := strings.TrimSpace(line); {
		case strings.HasPrefix(t, "- [x]"), strings.HasPrefix(t, "- [X]"):
			done++
			total++
		case strings.HasPrefix(t, "- [ ]"), strings.HasPrefix(t, "- [~]"):
			total++
		}
	}
	if total == 0 {
		return 0
	}
	return float64(done) / float64(total)
}
