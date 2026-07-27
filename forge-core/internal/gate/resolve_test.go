package gate

import "testing"

// The lifecycle-aware N/A exemption matrix (GatesGreen) is what finally lets an
// ADAPTER-LESS project converge objectively — without ever weakening production.
// These six tests pin the architect's four invariants:
//
//	① an adapter-less language (lint/build N/A) CONVERGES at an immature lifecycle;
//	② production stays STRICT (a no_tool N/A blocks; only an inapplicable one waives);
//	③ a FAIL is NEVER exempted, at any lifecycle / category;
//	④ the vacuous-green guard: all-N/A (even fully exempted) is NOT green.
//
// They drive GatesGreen with probe-backed gates only (lint/build/security read
// their status straight from the probe map, and `test` ANDs test_pass+app_test_pass
// from it) so the matrix is exercised hermetically — no structural gate (complexity/
// arch) is required, so the tests never shell node/python and stay deterministic.
//
// Relocated from cmd/forge/converge_exempt_test.go when GatesGreen moved into this
// package (2026-07-02); the end-to-end test that also exercises gatherSignals stayed
// in cmd/forge, since gatherSignals is orchestration, not gate-resolution, logic.

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

	green, proof := GatesGreen("", names, probe, cats, "mvp")
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
	if green, proof := GatesGreen("", names, probe, noTool, "production"); green {
		t.Errorf("production must NOT converge with a no_tool N/A (missing tool); proof=%+v", proof)
	}

	// Same everything, lint inapplicable -> production converges (precise, not a
	// one-size-fits-all production veto).
	inapplicable := map[string]string{"lint": "inapplicable", "build": "inapplicable"}
	if green, _ := GatesGreen("", names, probe, inapplicable, "production"); !green {
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
		if green, _ := GatesGreen("", names, probe, cats, lc); green {
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
			if green, _ := GatesGreen("", names, probe, cats, lc); green {
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
	if green, proof := GatesGreen("", names, probe, cats, "idea"); green {
		t.Errorf("all-N/A (even all-exempted) must NOT be green — nothing proven; proof=%+v", proof)
	}
	// Adding a single real PASS flips it green (proves the guard is precisely the
	// "≥1 non-NA PASS" rule, not a blanket block on any exemption).
	probe["test_pass"], probe["app_test_pass"] = "PASS", "PASS"
	if green, _ := GatesGreen("", append(names, "test"), probe, cats, "idea"); !green {
		t.Error("one proven (non-NA PASS) gate plus exempted N/As must be green")
	}
}

// Zero required gates is not green (nothing proven) — preserved from the prior
// allRequiredGatesPass contract.
func TestGatesGreen_ZeroGatesNotGreen(t *testing.T) {
	if green, _ := GatesGreen("", nil, nil, nil, "mvp"); green {
		t.Error("zero required gates must not be green")
	}
}

func TestResolveGateSecurityRequiresSecretScanAndSCA(t *testing.T) {
	cases := []struct {
		name  string
		probe map[string]string
		want  string
	}{
		{"both pass", map[string]string{
			"security_findings": StatusPass, "dependency_vulnerabilities": StatusPass,
		}, StatusPass},
		{"secret fails", map[string]string{
			"security_findings": StatusFail, "dependency_vulnerabilities": StatusPass,
		}, StatusFail},
		{"SCA fails", map[string]string{
			"security_findings": StatusPass, "dependency_vulnerabilities": StatusFail,
		}, StatusFail},
		{"SCA unavailable", map[string]string{
			"security_findings": StatusPass, "dependency_vulnerabilities": StatusNA,
		}, StatusNA},
		{"SCA missing", map[string]string{
			"security_findings": StatusPass,
		}, StatusNA},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := ResolveGate("", "security", tc.probe); got.Status != tc.want {
				t.Fatalf("security status = %s, want %s (%s)", got.Status, tc.want, got.Output)
			}
		})
	}
}

func TestGatesGreenSecuritySCAIsLifecycleAware(t *testing.T) {
	probe := map[string]string{
		"test_pass": StatusPass, "app_test_pass": StatusPass,
		"security_findings": StatusPass, "dependency_vulnerabilities": StatusNA,
	}
	categories := map[string]string{"dependency_vulnerabilities": catNoTool}
	names := []string{"test", "security"}
	if green, _ := GatesGreen("", names, probe, categories, "mvp"); !green {
		t.Error("an immature project may honestly exempt a missing SCA database")
	}
	if green, _ := GatesGreen("", names, probe, categories, "production"); green {
		t.Error("production must not converge without an SCA database")
	}

	probe["dependency_vulnerabilities"] = StatusFail
	if green, _ := GatesGreen("", names, probe, categories, "mvp"); green {
		t.Error("a detected dependency vulnerability must never be exempted")
	}
}

func TestCompositeCategoryDoesNotMaskMissingSecretProbe(t *testing.T) {
	probe := map[string]string{"dependency_vulnerabilities": StatusNA, "test_pass": StatusPass, "app_test_pass": StatusPass}
	categories := map[string]string{
		"security_findings":          "applicable",
		"dependency_vulnerabilities": catNoTool,
	}
	if green, _ := GatesGreen("", []string{"test", "security"}, probe, categories, "mvp"); green {
		t.Error("a missing secret-scan result must not borrow the SCA no_tool exemption")
	}
}
