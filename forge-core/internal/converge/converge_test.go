package converge

import (
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
