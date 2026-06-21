package main

import (
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

// The lifecycle-aware N/A exemption matrix (gatesGreen) is what finally lets an
// ADAPTER-LESS project converge objectively — without ever weakening production.
// These four tests pin the architect's four invariants:
//
//	① an adapter-less language (lint/build N/A) CONVERGES at an immature lifecycle;
//	② production stays STRICT (a no_tool N/A blocks; only an inapplicable one waives);
//	③ a FAIL is NEVER exempted, at any lifecycle / category;
//	④ the vacuous-green guard: all-N/A (even fully exempted) is NOT green.
//
// They drive gatesGreen with probe-backed gates only (lint/build/security read
// their status straight from the probe map, and `test` ANDs test_pass+app_test_pass
// from it) so the matrix is exercised hermetically — no structural gate (complexity/
// arch) is required, so the tests never shell node/python and stay deterministic.

// exemptWorkflow builds a workflow whose single phase requires the given gates —
// the de-duplicated required set gatesGreen judges.
func exemptWorkflow(gates ...string) asset.Workflow {
	return asset.Workflow{Phases: []asset.Phase{{Name: "verify", RequiredGates: gates}}}
}

// ① ADAPTER-LESS CONVERGENCE. test_pass+app_test_pass PASS (the proving non-NA
// gate), lint N/A:no_tool, build N/A:inapplicable, at lifecycle=mvp → green. This
// is the固化: an objective convergence with no manual required_gates surgery.
func TestGatesGreen_AdapterlessConverges(t *testing.T) {
	probe := map[string]string{
		"test_pass": "PASS", "app_test_pass": "PASS",
		"lint": "NA", "build": "NA",
	}
	cats := map[string]string{
		"lint": "no_tool", "build": "inapplicable",
	}
	names := []string{"lint", "test", "build"}

	green, proof := gatesGreen("", names, probe, cats, "mvp")
	if !green {
		t.Fatalf("adapter-less project at mvp must converge; proof=%+v", proof)
	}
	// The proof must be HONEST: `test` proven, lint+build listed as exemptions with
	// their categories (so the report never claims they were verified).
	if len(proof.Proven) != 1 || proof.Proven[0] != "test" {
		t.Errorf("proven gates = %v, want [test]", proof.Proven)
	}
	gotCat := map[string]string{}
	for _, e := range proof.Exemptions {
		gotCat[e.Name] = e.Category
	}
	if gotCat["lint"] != "no_tool" || gotCat["build"] != "inapplicable" {
		t.Errorf("exemption categories = %v, want lint:no_tool build:inapplicable", gotCat)
	}
}

// ① END-TO-END. gatherSignals → converge.Converge on gates_status==green must be
// MET, and the rendered detail must NAME the waived gates (explicit honesty, not a
// blanket "all green").
func TestGatesStatus_AdapterlessEndToEnd(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "- [x] done\n")

	probe := map[string]string{
		"test_pass": "PASS", "app_test_pass": "PASS",
		"lint": "NA", "build": "NA",
	}
	cats := map[string]string{"lint": "no_tool", "build": "inapplicable"}
	wf := exemptWorkflow("lint", "test", "build")
	wf.Stop = asset.StopCondition{Type: "conjunction", AllOf: []asset.Criterion{
		{Metric: "gates_status", Operator: "==", Value: "green"},
	}}

	sig := gatherSignals(root, wf, probe, cats, "mvp", false)
	results, met := converge.Converge(wf.Stop, sig)
	if !met {
		t.Fatalf("gates_status must converge for an adapter-less mvp project; results=%+v", results)
	}
	detail := results[0].Detail
	if !strings.Contains(detail, "exempt") || !strings.Contains(detail, "lint") || !strings.Contains(detail, "build") {
		t.Errorf("convergence detail must explicitly list the waived gates; got %q", detail)
	}
	// And it must NOT pretend a never-run check was verified.
	if strings.Contains(detail, "all required gates green") {
		t.Errorf("detail must not claim blanket verification when gates were exempted; got %q", detail)
	}
}

// ② PRODUCTION STAYS STRICT — the heart of "never weaken production". The SAME
// adapter-less probe that converges at mvp must NOT converge at production while
// lint is no_tool (a fixable gap production must close). The CONTRAST proves the
// matrix is two-dimensional and not a blunt "production blocks every N/A": flip
// lint's category to inapplicable and production converges again (the language
// genuinely has no linter — honest at every lifecycle).
func TestGatesGreen_ProductionStrictButPrecise(t *testing.T) {
	probe := map[string]string{
		"test_pass": "PASS", "app_test_pass": "PASS",
		"lint": "NA", "build": "NA",
	}
	names := []string{"lint", "test", "build"}

	// no_tool lint @ production -> blocked.
	noTool := map[string]string{"lint": "no_tool", "build": "inapplicable"}
	if green, proof := gatesGreen("", names, probe, noTool, "production"); green {
		t.Errorf("production must NOT converge with a no_tool N/A (missing tool); proof=%+v", proof)
	}

	// Same everything, lint inapplicable -> production converges (precise, not a
	// one-size-fits-all production veto).
	inapplicable := map[string]string{"lint": "inapplicable", "build": "inapplicable"}
	if green, _ := gatesGreen("", names, probe, inapplicable, "production"); !green {
		t.Error("production MUST converge when the N/A is inapplicable (no such concept), proving the matrix is precise")
	}
}

// ②b UNKNOWN / EMPTY lifecycle is fail-safe toward production. A no_tool N/A under
// an unrecognised (or empty) lifecycle must block exactly as production does — a
// typo'd maturity over-enforces rather than silently waiving the tool.
func TestGatesGreen_UnknownLifecycleFailsSafe(t *testing.T) {
	probe := map[string]string{"test_pass": "PASS", "app_test_pass": "PASS", "lint": "NA"}
	cats := map[string]string{"lint": "no_tool"}
	names := []string{"lint", "test"}
	for _, lc := range []string{"", "prod", "stable", "garbage"} {
		if green, _ := gatesGreen("", names, probe, cats, lc); green {
			t.Errorf("lifecycle %q must be treated as strict (no no_tool waiver), but converged", lc)
		}
	}
}

// ③ FAIL IS NEVER EXEMPTED. A lint FAIL must block convergence at EVERY lifecycle
// and regardless of any (irrelevant) category — an exemption applies only to N/A,
// never to a check that ran and FAILED.
func TestGatesGreen_FailNeverExempted(t *testing.T) {
	probe := map[string]string{"test_pass": "PASS", "app_test_pass": "PASS", "lint": "FAIL"}
	names := []string{"lint", "test"}
	// Try every lifecycle and a spread of categories (including the waiving ones):
	for _, lc := range []string{"idea", "mvp", "growth", "production", "", "weird"} {
		for _, cat := range []string{"no_tool", "inapplicable", "", "applicable"} {
			cats := map[string]string{"lint": cat}
			if green, _ := gatesGreen("", names, probe, cats, lc); green {
				t.Errorf("a lint FAIL must block (lifecycle=%q category=%q) but converged", lc, cat)
			}
		}
	}
}

// ④ VACUOUS-GREEN GUARD. When EVERY required gate is N/A — even a mix of fully
// exemptible categories at the LOOSEST lifecycle (idea) — nothing was proven, so
// the verdict is NOT green. "Green" must rest on at least one real PASS.
func TestGatesGreen_VacuousAllNAIsNotGreen(t *testing.T) {
	probe := map[string]string{"lint": "NA", "build": "NA"}
	cats := map[string]string{"lint": "no_tool", "build": "inapplicable"}
	names := []string{"lint", "build"}
	if green, proof := gatesGreen("", names, probe, cats, "idea"); green {
		t.Errorf("all-N/A (even all-exempted) must NOT be green — nothing proven; proof=%+v", proof)
	}
	// Adding a single real PASS flips it green (proves the guard is precisely the
	// "≥1 non-NA PASS" rule, not a blanket block on any exemption).
	probe["test_pass"], probe["app_test_pass"] = "PASS", "PASS"
	if green, _ := gatesGreen("", append(names, "test"), probe, cats, "idea"); !green {
		t.Error("one proven (non-NA PASS) gate plus exempted N/As must be green")
	}
}

// Zero required gates is not green (nothing proven) — preserved from the prior
// allRequiredGatesPass contract.
func TestGatesGreen_ZeroGatesNotGreen(t *testing.T) {
	if green, _ := gatesGreen("", nil, nil, nil, "mvp"); green {
		t.Error("zero required gates must not be green")
	}
}
