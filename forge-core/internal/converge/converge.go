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

	// RequirementConfidence is the confidence score (0-100) from the discover
	// workflow's requirement-discovery phase — the product-manager agent's own
	// self-assessed confidence that the requirements are complete enough to
	// proceed. This is an agent-reported signal (like RoadmapCompletion), so it
	// is honest-but-trusting. A zero value means "no discover phase ran or
	// no confidence score reported" — treated as unmet by evalOne.
	// Wired from the requirement-discovery phase agent output (confidence_metric
	// field in discover.yml). Default 0 = no data (never silently converged).
	RequirementConfidence float64

	// ReviewStatus is the convergence verdict from the review workflow's CTO
	// executive review phase. Maps the agent's VERDICT to a convergence signal:
	// "approved" = APPROVE or APPROVE_WITH_SIMPLIFICATION → Met;
	// anything else or empty = unmet. Wired from parseReviewerVerdict on the
	// cto-review phase output. Default "" = no data (never silently converged).
	ReviewStatus string

	// FileDelta is the fraction in [0,1] of roadmap items that have corresponding
	// file-system changes in the git diff (analysis §scan-current-gaps direction 5).
	// It provides independent cross-validation of RoadmapCompletion: if the agent
	// claims 100% completion but FileDelta is near 0 (no code changes), the
	// convergence report flags a gap. Computed from `git diff --name-only` matched
	// against roadmap item keywords. Not used for convergence (that remains
	// RoadmapCompletion AND GatesGreen); surfaced as an honesty warning in the
	// converge report (analysis §回路D — self-report risk).
	FileDelta float64

	// HumanApproved is the approval signal for a human_gate stop condition (the
	// design->build gate). It is the ONLY key that converges a human_gate: false
	// means the stage honestly waits for a human (NOT MET, never a gate failure),
	// true means an explicit approval was given. It is irrelevant to conjunction /
	// external stops, so the default false leaves those paths byte-for-byte
	// unchanged. The CLI sources it from --approved or a <root>/.forge/<stage>.approved
	// marker (see cmd/forge). HONESTY: v1 is an approval-SIGNAL check, not a durable
	// cross-process wait — see Converge's note on durable_wait.
	HumanApproved bool

	// Criteria carries per-criterion acceptance verdicts (criterion name ->
	// "PASS"|"FAIL"|"NA", exactly as gate.ProbeAll emits them) so a workflow can
	// converge on a single acceptance criterion (e.g. test_pass) instead of only
	// the coarse GatesGreen aggregate. It reuses the SAME ProbeAll result the
	// acceptance gate already ran — no extra spawn. A nil map means no probe data
	// is wired, and every per-criterion check then degrades safely to unmet
	// (honest: absence of a verdict is never satisfaction).
	Criteria map[string]string

	// GateProof is the per-gate breakdown behind GatesGreen — which required gates
	// were PROVEN (a real PASS) and which were EXEMPTED (an N/A the lifecycle-aware
	// matrix waived) with the reason. It does NOT change the meaning of GatesGreen
	// (that bool is still the sole convergence signal for gates_status); it only
	// lets greenDetail render an HONEST line that names the exempted gates and why,
	// instead of claiming everything was verified. Empty (the zero value) makes
	// greenDetail fall back to its terse legacy text, so callers that do not wire it
	// (and every existing test) are byte-for-byte unchanged.
	GateProof GateProof

	// CodeTestRatio is the fraction of changed lines that are test code
	// (0 = no test changes, 0.5 = equal test and prod changes, 1 = all tests).
	// Computed from `git diff --stat` in gatherSignals. A zero value with
	// non-zero prod changes indicates a test gap. Not used for convergence;
	// surfaced as a warning in the converge report (analysis §5.2).
	CodeTestRatio float64
}

// GateProof records, for honesty rendering only, which required gates were proven
// versus exempted under the lifecycle-aware N/A matrix. Proven is the gates that
// resolved to a real PASS; Exemptions is the gates that were N/A but waived, each
// annotated with its category+reason (e.g. "no_tool:not installed"). The CLI fills
// it from gatesGreen; converge only renders it.
type GateProof struct {
	Proven     []string
	Exemptions []GateExemption
}

// GateExemption is one waived gate: its Name, the N/A Category that justified the
// waiver ("inapplicable"|"no_tool"), and a short human Reason carried from the
// acceptance detail. Rendered by greenDetail so the convergence report is explicit
// about what was NOT verified and why it was nonetheless allowed.
type GateExemption struct {
	Name     string
	Category string
	Reason   string
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

// HumanGateType is the stop_condition.type that marks the human-approval gate.
const HumanGateType = "human_gate"

// awaitingApprovalDetail is the honest "stopped, waiting for a human" message a
// not-yet-approved human_gate reports. It is deliberately NOT a gate-failure
// string: the gate has not failed, it is correctly holding for a non-bypassable
// human decision.
const awaitingApprovalDetail = "awaiting human approval (non-bypassable)"

// IsHumanGate reports whether a stop condition is the human-approval gate —
// either by an explicit type, or by carrying a human_approval requirement (so a
// workflow that only sets `human_approval: required` is still gated). This is the
// single predicate the runtime and CLI share, so the two never disagree on what
// "needs a human" means.
func IsHumanGate(stop asset.StopCondition) bool {
	return stop.Type == HumanGateType || stop.HumanApproval != ""
}

// Converge is the single convergence entry point that honors EVERY stop-condition
// shape. It dispatches on the condition, so a caller never has to know which kind
// it holds:
//
//   - human_gate (design->build): converges IF AND ONLY IF sig.HumanApproved is
//     true. This is its OWN branch — it does NOT run the conjunction path, so an
//     unapproved human_gate can never be mistaken for "converged" by the
//     zero-criteria rule or any other conjunction edge. Not approved => NOT MET
//     with the honest "awaiting human approval (non-bypassable)" detail (a stop
//     to wait for a human, not a gate failure). Fail-closed + non-bypassable:
//     there is no path from HumanApproved==false to met.
//   - everything else (conjunction, …): the existing Evaluate(all_of) semantics,
//     unchanged.
//
// HONESTY on durable_wait: design.yml declares `durable_wait: true` (survive
// restarts for hours/days — Temporal, from v2). v1 does NOT implement a durable
// cross-process wait; Converge only checks the approval SIGNAL present at
// evaluation time (a --approved flag or an on-disk marker). It does not block,
// poll, or persist a pending wait across process boundaries. The non-bypassable
// SEMANTICS are real (no approval => never converges); the durability is not yet.
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
	if IsHumanGate(stop) {
		return humanGate(sig)
	}
	return Evaluate(stop.AllOf, sig)
}

// humanGate evaluates the human-approval gate. The ONLY way it converges is an
// explicit HumanApproved==true; anything else is the honest "awaiting human
// approval (non-bypassable)" — a deliberate stop-to-wait, never a failure and
// never an auto-pass. It returns exactly one Result so the report shows the gate.
func humanGate(sig Signals) (results []Result, met bool) {
	if sig.HumanApproved {
		return []Result{{Expr: "human_approval == granted", Met: true, Detail: "human approval granted"}}, true
	}
	return []Result{{Expr: "human_approval == granted", Met: false, Detail: awaitingApprovalDetail}}, false
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
		return Result{render(c), met, greenDetail(sig)}
	case c.Metric == "requirement_confidence":
		return evalRequirementConfidence(c, sig)
	case c.Metric == "review_status":
		return evalReviewStatus(c, sig)
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

// evalRequirementConfidence evaluates the requirement_confidence metric from
// the discover workflow. The signal is the agent-reported confidence (0-100).
// A threshold of 80 means ">= 80% confidence required". Missing threshold:
// falls back to the value from the criterion's Threshold (built-in from YAML).
// Zero confidence (no data) is always unmet.
func evalRequirementConfidence(c asset.Criterion, sig Signals) Result {
	detail := fmt.Sprintf("requirement_confidence=%.0f", sig.RequirementConfidence)
	if sig.RequirementConfidence == 0 {
		return Result{render(c), false, detail + " (no discover phase data)"}
	}
	if c.Threshold == nil {
		return Result{render(c), false, detail + " (no threshold given)"}
	}
	met := compare(sig.RequirementConfidence, c.Operator, *c.Threshold)
	return Result{render(c), met, detail}
}

// evalReviewStatus evaluates the review_status metric from the review workflow.
// The signal is the CTO agent's VERDICT: "approved" matches APPROVE or
// APPROVE_WITH_SIMPLIFICATION; anything else is unmet. An empty review_status
// (no review phase ran) is always unmet.
func evalReviewStatus(c asset.Criterion, sig Signals) Result {
	met := sig.ReviewStatus == "approved"
	detail := fmt.Sprintf("review_status=%s", sig.ReviewStatus)
	if sig.ReviewStatus == "" {
		detail += " (no review phase data)"
	}
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

// greenDetail renders the honest convergence detail for gates_status. When the
// gates are green it does NOT claim blanket verification: if any required gate was
// EXEMPTED (an N/A waived by the lifecycle-aware matrix) it names the proven gates
// AND lists each exemption with its category+reason, so the report never pretends a
// never-run check passed. A red verdict reports it. With no GateProof wired (the
// zero value — e.g. a unit test that sets only GatesGreen) it falls back to the
// terse legacy strings, keeping those call sites unchanged.
func greenDetail(sig Signals) string {
	if !sig.GatesGreen {
		return "a required gate is not green"
	}
	proof := sig.GateProof
	if len(proof.Proven) == 0 && len(proof.Exemptions) == 0 {
		return "all required gates green"
	}
	parts := []string{fmt.Sprintf("%d gate(s) green", len(proof.Proven))}
	if len(proof.Proven) > 0 {
		parts[0] += " (" + strings.Join(proof.Proven, ",") + ")"
	}
	if len(proof.Exemptions) > 0 {
		parts = append(parts, "exempt "+renderExemptions(proof.Exemptions))
	}
	return strings.Join(parts, "; ")
}

// renderExemptions formats the waived gates as "name(category:reason)·…" so the
// detail line is explicit about exactly which gates were not verified and why.
func renderExemptions(ex []GateExemption) string {
	parts := make([]string, len(ex))
	for i, e := range ex {
		parts[i] = fmt.Sprintf("%s(%s:%s)", e.Name, e.Category, e.Reason)
	}
	return strings.Join(parts, "·")
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
