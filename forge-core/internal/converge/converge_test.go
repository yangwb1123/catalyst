package converge

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func ptr(f float64) *float64 { return &f }

// roadmap builds a roadmap_completion criterion with a numeric threshold.
func roadmap(op string, threshold float64) asset.Criterion {
	return asset.Criterion{Metric: "roadmap_completion", Operator: op, Threshold: ptr(threshold)}
}

// gates builds a gates_status criterion expecting the given value.
func gates(value string) asset.Criterion {
	return asset.Criterion{Metric: "gates_status", Operator: "==", Value: value}
}

func TestRoadmapCompletion(t *testing.T) {
	cases := []struct {
		name string
		md   string
		want float64
	}{
		{"all done", "- [x] a\n- [x] b", 1.0},
		{"half", "- [x] a\n- [ ] b", 0.5},
		{"partial counts as not done", "- [x] a\n- [~] b\n- [ ] c", 1.0 / 3.0},
		{"no items", "# heading\nprose only", 0},
		{"empty", "", 0},
	}
	for _, c := range cases {
		if got := RoadmapCompletion(c.md); got != c.want {
			t.Errorf("%s: RoadmapCompletion = %v, want %v", c.name, got, c.want)
		}
	}
}

// The real build.yml conjunction: roadmap==100 AND gates_status==green.
func TestEvaluate_AllMet(t *testing.T) {
	results, all := Evaluate(
		[]asset.Criterion{roadmap("==", 100), gates("green")},
		Signals{RoadmapCompletion: 1.0, GatesGreen: true})
	if !all {
		t.Errorf("expected converged; results=%+v", results)
	}
}

// Operator coverage: == / >= / < against roadmap_completion (as a percentage).
func TestEvaluate_Operators(t *testing.T) {
	cases := []struct {
		name string
		crit asset.Criterion
		comp float64 // RoadmapCompletion fraction
		want bool
	}{
		{"== exact", roadmap("==", 100), 1.0, true},
		{"== not exact", roadmap("==", 100), 0.99, false},
		{">= met at boundary", roadmap(">=", 80), 0.8, true},
		{">= met above", roadmap(">=", 80), 0.9, true},
		{">= unmet below", roadmap(">=", 80), 0.79, false},
		{"< met below", roadmap("<", 50), 0.4, true},
		{"< unmet at boundary", roadmap("<", 50), 0.5, false},
		{"unknown operator never satisfies", roadmap("~=", 100), 1.0, false},
		{"missing threshold never satisfies",
			asset.Criterion{Metric: "roadmap_completion", Operator: "=="}, 1.0, false},
	}
	for _, c := range cases {
		results, _ := Evaluate([]asset.Criterion{c.crit}, Signals{RoadmapCompletion: c.comp})
		if results[0].Met != c.want {
			t.Errorf("%s: Met = %v, want %v (detail=%q)", c.name, results[0].Met, c.want, results[0].Detail)
		}
	}
}

func TestEvaluate_RoadmapNotMet(t *testing.T) {
	if _, all := Evaluate([]asset.Criterion{roadmap("==", 100)}, Signals{RoadmapCompletion: 0.5}); all {
		t.Error("0.5 completion must not satisfy the ==100 criterion")
	}
}

func TestEvaluate_GatesGreen(t *testing.T) {
	if _, all := Evaluate([]asset.Criterion{gates("green")}, Signals{GatesGreen: true}); !all {
		t.Error("gates_status==green with green gates must be met")
	}
}

func TestEvaluate_GateNotGreen(t *testing.T) {
	if _, all := Evaluate([]asset.Criterion{gates("green")}, Signals{GatesGreen: false}); all {
		t.Error("a red gate must block convergence")
	}
}

// gates_status with a non-green expected value is not met even if gates pass.
func TestEvaluate_GatesWrongValueUnmet(t *testing.T) {
	results, _ := Evaluate([]asset.Criterion{gates("amber")}, Signals{GatesGreen: true})
	if results[0].Met {
		t.Error("gates_status only converges on the literal 'green'")
	}
}

// The honesty invariant: an unknown metric is never silently satisfied.
func TestEvaluate_UnknownMetricUnmet(t *testing.T) {
	frob := asset.Criterion{Metric: "frobnicate", Operator: "==", Value: "sparkly"}
	results, all := Evaluate([]asset.Criterion{frob}, Signals{RoadmapCompletion: 1, GatesGreen: true})
	if all || results[0].Met {
		t.Error("unknown metric must be treated as unmet, never silently passed")
	}
}

// A bare-string criterion (no metric) is also unmet-by-default, with Raw shown.
func TestEvaluate_BareStringUnmet(t *testing.T) {
	results, all := Evaluate([]asset.Criterion{{Raw: "roadmap_completion == 100%"}},
		Signals{RoadmapCompletion: 1, GatesGreen: true})
	if all || results[0].Met {
		t.Error("a bare-string criterion has no metric and must be unmet")
	}
	if results[0].Expr != "roadmap_completion == 100%" {
		t.Errorf("bare string should render as its Raw text; got %q", results[0].Expr)
	}
}

func TestEvaluate_EmptyIsNotConverged(t *testing.T) {
	if _, all := Evaluate(nil, Signals{RoadmapCompletion: 1, GatesGreen: true}); all {
		t.Error("zero criteria must not count as converged")
	}
}

// crit builds an acceptance per-criterion criterion (e.g. test_pass) whose
// verdict is resolved from Signals.Criteria, not a threshold.
func crit(metric string) asset.Criterion {
	return asset.Criterion{Metric: metric, Operator: "==", Value: "true"}
}

// Per-criterion dispatch: PASS->met, every other verdict (FAIL/NA/missing)
// ->unmet. The verdict spelling matches gate.ProbeAll's output exactly.
func TestEvaluate_CriterionVerdicts(t *testing.T) {
	cases := []struct {
		name   string
		metric string
		verdct map[string]string // Signals.Criteria
		want   bool
	}{
		{"test_pass PASS is met", "test_pass", map[string]string{"test_pass": "PASS"}, true},
		{"test_pass FAIL is unmet", "test_pass", map[string]string{"test_pass": "FAIL"}, false},
		{"test_pass NA is unmet (honesty)", "test_pass", map[string]string{"test_pass": "NA"}, false},
		{"app_test_pass PASS is met", "app_test_pass", map[string]string{"app_test_pass": "PASS"}, true},
		{"architecture PASS is met", "architecture", map[string]string{"architecture": "PASS"}, true},
		{"arch_violations FAIL is unmet", "arch_violations", map[string]string{"arch_violations": "FAIL"}, false},
		{"complexity_violations PASS is met", "complexity_violations", map[string]string{"complexity_violations": "PASS"}, true},
		{"missing verdict is unmet", "test_pass", map[string]string{"architecture": "PASS"}, false},
		{"nil Criteria map is unmet", "test_pass", nil, false},
		{"unexpected verdict value is unmet", "test_pass", map[string]string{"test_pass": "MAYBE"}, false},
	}
	for _, c := range cases {
		results, all := Evaluate([]asset.Criterion{crit(c.metric)}, Signals{Criteria: c.verdct})
		if results[0].Met != c.want || all != c.want {
			t.Errorf("%s: Met=%v all=%v, want %v (detail=%q)", c.name, results[0].Met, all, c.want, results[0].Detail)
		}
	}
}

// Conjunction across kinds: an acceptance criterion AND roadmap must both hold;
// either one unmet blocks convergence, matching Evaluate's all-met semantics.
func TestEvaluate_CriterionConjunction(t *testing.T) {
	sig := Signals{RoadmapCompletion: 1.0, Criteria: map[string]string{"test_pass": "PASS"}}

	if _, all := Evaluate([]asset.Criterion{crit("test_pass"), roadmap("==", 100)}, sig); !all {
		t.Error("test_pass PASS AND roadmap==100 must converge")
	}

	// Flip roadmap below threshold: still unmet despite test_pass green.
	sig.RoadmapCompletion = 0.5
	if _, all := Evaluate([]asset.Criterion{crit("test_pass"), roadmap("==", 100)}, sig); all {
		t.Error("a failing roadmap must block convergence even with test_pass green")
	}

	// Flip the criterion to FAIL: roadmap green is not enough on its own.
	sig = Signals{RoadmapCompletion: 1.0, Criteria: map[string]string{"test_pass": "FAIL"}}
	if _, all := Evaluate([]asset.Criterion{crit("test_pass"), roadmap("==", 100)}, sig); all {
		t.Error("a failing test_pass must block convergence even with roadmap==100")
	}
}

// Back-compat: with Criteria nil, the legacy roadmap/gates path is unchanged —
// a per-criterion metric simply degrades to unmet rather than panicking.
func TestEvaluate_NilCriteriaBackCompat(t *testing.T) {
	results, all := Evaluate(
		[]asset.Criterion{roadmap("==", 100), gates("green")},
		Signals{RoadmapCompletion: 1.0, GatesGreen: true}) // no Criteria map
	if !all {
		t.Errorf("legacy roadmap+gates conjunction must still converge; results=%+v", results)
	}
}

// greenDetail must render the HONEST exemption summary when a GateProof is wired:
// name the proven gates AND each waived gate with its category+reason, and never
// claim "all required gates green" once anything was exempted.
func TestGreenDetail_ListsExemptions(t *testing.T) {
	sig := Signals{
		GatesGreen: true,
		GateProof: GateProof{
			Proven: []string{"test", "complexity"},
			Exemptions: []GateExemption{
				{Name: "lint", Category: "no_tool", Reason: "eslint not installed"},
				{Name: "build", Category: "inapplicable", Reason: "no build step"},
			},
		},
	}
	got := greenDetail(sig)
	for _, want := range []string{"test", "complexity", "exempt", "lint", "no_tool", "build", "inapplicable"} {
		if !strings.Contains(got, want) {
			t.Errorf("greenDetail missing %q; got %q", want, got)
		}
	}
	if strings.Contains(got, "all required gates green") {
		t.Errorf("an exempted-gate detail must not claim blanket verification; got %q", got)
	}
}

// greenDetail with NO proof wired (the zero value — every legacy unit test that
// sets only GatesGreen) must keep the terse legacy strings, byte-for-byte.
func TestGreenDetail_LegacyFallback(t *testing.T) {
	if got := greenDetail(Signals{GatesGreen: true}); got != "all required gates green" {
		t.Errorf("green + no proof = %q, want legacy 'all required gates green'", got)
	}
	if got := greenDetail(Signals{GatesGreen: false}); got != "a required gate is not green" {
		t.Errorf("not-green = %q, want legacy 'a required gate is not green'", got)
	}
}

// A green verdict proven entirely by real PASSes (no exemptions) still reads as a
// clean summary, not a misleading "exempt" clause.
func TestGreenDetail_AllProvenNoExemptions(t *testing.T) {
	got := greenDetail(Signals{GatesGreen: true, GateProof: GateProof{Proven: []string{"test", "lint"}}})
	if strings.Contains(got, "exempt") {
		t.Errorf("no exemptions must not render an 'exempt' clause; got %q", got)
	}
	if !strings.Contains(got, "test") || !strings.Contains(got, "lint") {
		t.Errorf("proven gates must be named; got %q", got)
	}
}

// --- human_gate (the design->build approval gate) ----------------------------

// humanStop builds the design.yml-shaped human_gate stop condition.
func humanStop() asset.StopCondition {
	return asset.StopCondition{
		Type:          HumanGateType,
		HumanApproval: "required",
		OnApproved:    asset.OnApproved{NextStage: "build"},
	}
}

// IsHumanGate recognizes a human_gate by an explicit type OR a human_approval
// requirement, and does NOT mistake a conjunction/external for one.
func TestIsHumanGate(t *testing.T) {
	cases := []struct {
		name string
		stop asset.StopCondition
		want bool
	}{
		{"explicit type", asset.StopCondition{Type: HumanGateType}, true},
		{"human_approval only (no type)", asset.StopCondition{HumanApproval: "required"}, true},
		{"design.yml shape", humanStop(), true},
		{"conjunction is not a human gate", asset.StopCondition{Type: "conjunction", AllOf: []asset.Criterion{roadmap("==", 100)}}, false},
		{"external is not a human gate", asset.StopCondition{Type: "external"}, false},
		{"empty is not a human gate", asset.StopCondition{}, false},
	}
	for _, c := range cases {
		if got := IsHumanGate(c.stop); got != c.want {
			t.Errorf("%s: IsHumanGate = %v, want %v", c.name, got, c.want)
		}
	}
}

// Converge on a human_gate is approval-gated: HumanApproved==true converges with
// a "granted" result; false stays NOT MET with the honest awaiting detail.
func TestConverge_HumanGate_ApprovalGated(t *testing.T) {
	// Approved => met.
	results, met := Converge(humanStop(), Signals{HumanApproved: true})
	if !met {
		t.Fatalf("approved human_gate must converge; results=%+v", results)
	}
	if len(results) != 1 || !results[0].Met {
		t.Errorf("approved human_gate should yield one met result; got %+v", results)
	}

	// Not approved => NOT MET with the exact awaiting detail.
	results, met = Converge(humanStop(), Signals{HumanApproved: false})
	if met {
		t.Fatal("an unapproved human_gate must NOT converge")
	}
	if len(results) != 1 || results[0].Met {
		t.Fatalf("unapproved human_gate should yield one unmet result; got %+v", results)
	}
	if results[0].Detail != awaitingApprovalDetail {
		t.Errorf("detail = %q, want %q", results[0].Detail, awaitingApprovalDetail)
	}
	if !strings.Contains(results[0].Detail, "non-bypassable") {
		t.Errorf("awaiting detail must mark the gate non-bypassable; got %q", results[0].Detail)
	}
}

// THE non-bypassable invariant. An unapproved human_gate must NEVER converge —
// not via the zero-criteria rule, not when other signals are maxed, not when it
// carries a stray all_of, and not when it is declared by human_approval alone.
// This is the load-bearing negative test that nails down non-bypassability.
func TestConverge_HumanGate_UnapprovedNeverConverges(t *testing.T) {
	maxed := Signals{
		RoadmapCompletion: 1.0,
		GatesGreen:        true,
		Criteria:          map[string]string{"test_pass": "PASS", "app_test_pass": "PASS", "architecture": "PASS"},
		HumanApproved:     false, // the ONLY thing missing
	}
	variants := []struct {
		name string
		stop asset.StopCondition
	}{
		{"bare human_gate (no criteria)", asset.StopCondition{Type: HumanGateType}},
		{"design.yml shape", humanStop()},
		{"human_approval only", asset.StopCondition{HumanApproval: "required"}},
		// A human_gate that ALSO carries a fully-satisfied conjunction must STILL
		// not converge without approval — the human branch wins, the all_of is not
		// a bypass.
		{"human_gate with satisfied all_of must not bypass", asset.StopCondition{
			Type:  HumanGateType,
			AllOf: []asset.Criterion{roadmap("==", 100), gates("green")},
		}},
	}
	for _, v := range variants {
		if _, met := Converge(v.stop, maxed); met {
			t.Errorf("%s: converged WITHOUT human approval — non-bypassable invariant broken", v.name)
		}
	}
	// And it flips to met the instant approval is present (proves the gate is the
	// sole lever, not a permanent block).
	approved := maxed
	approved.HumanApproved = true
	if _, met := Converge(humanStop(), approved); !met {
		t.Error("human_gate must converge once approved (the approval is the sole key)")
	}
}

// Converge must NOT change the conjunction/external paths: a non-human stop is
// still evaluated exactly by Evaluate(all_of). This guards the dispatch wrapper
// against regressing the existing behavior.
func TestConverge_NonHumanGate_DelegatesToEvaluate(t *testing.T) {
	stop := asset.StopCondition{Type: "conjunction", AllOf: []asset.Criterion{roadmap("==", 100), gates("green")}}
	sig := Signals{RoadmapCompletion: 1.0, GatesGreen: true}

	cResults, cMet := Converge(stop, sig)
	eResults, eMet := Evaluate(stop.AllOf, sig)
	if cMet != eMet || !cMet {
		t.Errorf("Converge must match Evaluate for a conjunction; cMet=%v eMet=%v", cMet, eMet)
	}
	if len(cResults) != len(eResults) {
		t.Errorf("Converge result count %d != Evaluate %d", len(cResults), len(eResults))
	}
	// External stop has no all_of: Converge delegates and reports not-converged
	// from zero criteria, exactly like Evaluate (the loop, not Converge, treats an
	// external stop's bound as the clean stop).
	ext := asset.StopCondition{Type: "external"}
	if _, met := Converge(ext, sig); met {
		t.Error("external stop has no criteria; Converge must report not-met (zero-criteria rule)")
	}
}

// ── requirement_confidence metric (discover.yml) ─────────────────────────

func TestEvaluate_RequirementConfidence(t *testing.T) {
	cases := []struct {
		name string
		crit asset.Criterion
		sig  Signals
		want bool
	}{
		{">= threshold met", asset.Criterion{Metric: "requirement_confidence", Operator: ">=", Threshold: ptr(80)},
			Signals{RequirementConfidence: 85}, true},
		{">= threshold unmet", asset.Criterion{Metric: "requirement_confidence", Operator: ">=", Threshold: ptr(80)},
			Signals{RequirementConfidence: 70}, false},
		{"zero confidence (no data) unmet", asset.Criterion{Metric: "requirement_confidence", Operator: ">=", Threshold: ptr(80)},
			Signals{RequirementConfidence: 0}, false},
		{"missing threshold unmet", asset.Criterion{Metric: "requirement_confidence", Operator: ">="},
			Signals{RequirementConfidence: 85}, false},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			results, _ := Evaluate([]asset.Criterion{c.crit}, c.sig)
			if results[0].Met != c.want {
				t.Errorf("Met=%v, want %v (detail=%q)", results[0].Met, c.want, results[0].Detail)
			}
		})
	}
}

// ── review_status metric (review.yml) ───────────────────────────────────

func TestEvaluate_ReviewStatus(t *testing.T) {
	cases := []struct {
		name string
		sig  Signals
		want bool
	}{
		{"approved is met", Signals{ReviewStatus: "approved"}, true},
		{"not approved is unmet", Signals{ReviewStatus: "request_changes"}, false},
		{"empty status (no review) is unmet", Signals{ReviewStatus: ""}, false},
	}
	crit := asset.Criterion{Metric: "review_status", Operator: "==", Value: "approved"}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			results, _ := Evaluate([]asset.Criterion{crit}, c.sig)
			if results[0].Met != c.want {
				t.Errorf("Met=%v, want %v (detail=%q)", results[0].Met, c.want, results[0].Detail)
			}
		})
	}
}
