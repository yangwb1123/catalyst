package main

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

// gatherSignals must hand the SAME acceptance probe map it already used for the
// gate verdicts to Signals.Criteria — the wiring that lets a workflow converge on
// an individual acceptance criterion (e.g. test_pass) and not only the coarse
// GatesGreen aggregate. The probe is reused, never re-spawned: one map drives
// both GatesGreen and the per-criterion Criteria, so they stay consistent.
func TestGatherSignals_PopulatesCriteriaFromProbe(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	// One done, one open item -> 50% roadmap completion (sanity that the rest of
	// the signal is still measured alongside the new Criteria wiring).
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"),
		"- [x] shipped\n- [ ] pending\n")

	// The already-probed acceptance map for THIS run (criterion -> PASS/FAIL/NA),
	// exactly as gate.ProbeAll emits it. test requires test_pass AND app_test_pass.
	probe := map[string]string{
		"test_pass":     "PASS",
		"app_test_pass": "FAIL",
		"architecture":  "PASS",
	}
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "verify", RequiredGates: []string{"test"}},
	}}

	sig := gatherSignals(root, wf, probe, nil, "mvp", false, nil)

	// The probe map must be carried through verbatim — same length and values.
	if len(sig.Criteria) != len(probe) {
		t.Fatalf("Criteria len = %d, want %d (the probe map passed through)", len(sig.Criteria), len(probe))
	}
	for k, want := range probe {
		if got := sig.Criteria[k]; got != want {
			t.Errorf("Criteria[%q] = %q, want %q", k, got, want)
		}
	}
	if sig.RoadmapCompletion != 0.5 {
		t.Errorf("RoadmapCompletion = %v, want 0.5 (1 of 2 items)", sig.RoadmapCompletion)
	}
	// 'test' gate needs BOTH test_pass and app_test_pass PASS; app_test_pass is
	// FAIL, so GatesGreen must be false (honest aggregate over the same probe).
	if sig.GatesGreen {
		t.Error("GatesGreen = true, want false (app_test_pass FAIL fails the test gate)")
	}
}

// End to end: a workflow whose stop condition is a SINGLE acceptance criterion
// (test_pass) must converge from gatherSignals' Criteria — proving the new
// Criteria wiring reaches converge.Evaluate and decides per-criterion, with
// honest PASS/FAIL/absent semantics, all off the one reused probe map.
func TestGatherSignals_PerCriterionConvergenceEndToEnd(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "- [x] done\n")

	// Stop on the single acceptance criterion test_pass == PASS (per-criterion
	// convergence, not the gates_status aggregate).
	stop := []asset.Criterion{{Metric: "test_pass", Operator: "==", Value: "PASS"}}
	wf := asset.Workflow{Phases: []asset.Phase{{Name: "verify", RequiredGates: []string{"test"}}}}

	// PASS probe -> the criterion is Met and the conjunction converges.
	pass := gatherSignals(root, wf, map[string]string{"test_pass": "PASS"}, nil, "mvp", false, nil)
	results, met := converge.Evaluate(stop, pass)
	if !met || !results[0].Met {
		t.Errorf("test_pass=PASS should converge per-criterion; got met=%v results=%+v", met, results)
	}

	// FAIL probe -> unmet; the SAME reused probe map drives the verdict honestly.
	fail := gatherSignals(root, wf, map[string]string{"test_pass": "FAIL"}, nil, "mvp", false, nil)
	if _, met := converge.Evaluate(stop, fail); met {
		t.Error("test_pass=FAIL must NOT converge")
	}

	// Absent verdict (nil/broken probe) -> unmet (absence is never satisfaction).
	absent := gatherSignals(root, wf, nil, nil, "mvp", false, nil)
	if _, met := converge.Evaluate(stop, absent); met {
		t.Error("absent test_pass verdict must NOT converge (honest unmet)")
	}
	if absent.Criteria != nil {
		t.Errorf("nil probe should leave Criteria nil, got %v", absent.Criteria)
	}
}

// readRoadmapAbsent: gatherSignals must tolerate a missing ROADMAP.md (0%
// completion) and still wire Criteria — the file read is best-effort, the probe
// wiring is not conditional on it.
func TestGatherSignals_MissingRoadmapStillWiresCriteria(t *testing.T) {
	root := t.TempDir() // no .agent/ROADMAP.md at all
	probe := map[string]string{"test_pass": "PASS"}
	sig := gatherSignals(root, asset.Workflow{}, probe, nil, "mvp", false, nil)
	if sig.RoadmapCompletion != 0 {
		t.Errorf("RoadmapCompletion = %v, want 0 (no ROADMAP)", sig.RoadmapCompletion)
	}
	if sig.Criteria["test_pass"] != "PASS" {
		t.Errorf("Criteria not wired when ROADMAP absent: %v", sig.Criteria)
	}
}

// humanApproved resolves the approval signal from EITHER the --approved flag OR
// a <root>/.forge/<stage>.approved marker, and is false when NEITHER is present
// (fail-closed). This is the v1 approval signal source for a human_gate.
func TestHumanApproved_FlagOrMarkerOrNeither(t *testing.T) {
	root := t.TempDir()
	const stage = "design"

	// Neither source -> false (an unapproved gate must not auto-converge).
	if humanApproved(root, stage, false) {
		t.Error("no flag and no marker must resolve to NOT approved (fail-closed)")
	}
	// Flag alone -> true, even with no marker on disk.
	if !humanApproved(root, stage, true) {
		t.Error("--approved flag alone must resolve to approved")
	}
	// Marker file alone -> true (operator dropped <root>/.forge/<stage>.approved).
	mkdir(t, filepath.Dir(approvalPath(root, stage)))
	writeFile(t, approvalPath(root, stage), "")
	if !humanApproved(root, stage, false) {
		t.Error("an on-disk .forge/<stage>.approved marker must resolve to approved")
	}
	// A marker for a DIFFERENT stage must not approve this one (stage-scoped).
	other := t.TempDir()
	mkdir(t, filepath.Dir(approvalPath(other, "build")))
	writeFile(t, approvalPath(other, "build"), "")
	if humanApproved(other, stage, false) {
		t.Error("a marker for another stage must not approve this stage")
	}
}

// gatherSignals must thread the resolved approval bit into Signals.HumanApproved
// so a human_gate's convergence is driven by it (and only it).
func TestGatherSignals_CarriesHumanApproved(t *testing.T) {
	root := t.TempDir()
	if sig := gatherSignals(root, asset.Workflow{}, nil, nil, "mvp", true, nil); !sig.HumanApproved {
		t.Error("gatherSignals(approved=true) must set Signals.HumanApproved")
	}
	if sig := gatherSignals(root, asset.Workflow{}, nil, nil, "mvp", false, nil); sig.HumanApproved {
		t.Error("gatherSignals(approved=false) must leave Signals.HumanApproved false")
	}
}

// reviewStatus must normalize the CTO's recorded executive-review verdict into the
// vocabulary evalReviewStatus consumes: APPROVE/APPROVE_WITH_SIMPLIFICATION -> "approved";
// REDESIGN/DELAY/REJECT -> their own lowercase token (a meaningful unmet detail, not a
// blank); not-yet-recorded (including a nil ledger) -> "" (honest absence).
func TestReviewStatus_NormalizesExecutiveVerdict(t *testing.T) {
	cases := []struct {
		verdict string
		want    string
	}{
		{VerdictApprove, "approved"},
		{VerdictApproveWithSimplification, "approved"},
		{VerdictRedesign, "redesign"},
		{VerdictDelay, "delay"},
		{VerdictReject, "reject"},
	}
	for _, c := range cases {
		verdicts := newVerdictLedger()
		verdicts.record("executive-review", c.verdict)
		if got := reviewStatus(verdicts); got != c.want {
			t.Errorf("reviewStatus() after recording %q = %q, want %q", c.verdict, got, c.want)
		}
	}
	if got := reviewStatus(newVerdictLedger()); got != "" {
		t.Errorf("no recorded verdict must yield \"\" (honest absence); got %q", got)
	}
	if got := reviewStatus(nil); got != "" {
		t.Errorf("a nil verdictLedger must yield \"\" (nil-safe); got %q", got)
	}
	// A verdict recorded for a DIFFERENT phase (e.g. the build reviewer) must not
	// leak into review_status — it is keyed strictly on "executive-review".
	other := newVerdictLedger()
	other.record("reviewer", VerdictApprove)
	if got := reviewStatus(other); got != "" {
		t.Errorf("a verdict recorded for a different phase must not resolve; got %q", got)
	}
}

// END-TO-END: gatherSignals -> converge.Converge on review_status=="approved" must
// flow through a populated verdictLedger exactly as review.yml's stop_condition
// expects — this is the concrete proof the previously-permanent "" (FC gap) is
// closed for a REAL recorded executive-review verdict, not just the parser unit.
func TestGatherSignals_ReviewStatusEndToEnd(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "- [x] done\n")

	wf := asset.Workflow{Stage: "review", Stop: asset.StopCondition{
		Type: "conjunction",
		AllOf: []asset.Criterion{
			{Metric: "review_status", Operator: "==", Value: "approved"},
		},
	}}

	// No verdict recorded yet -> unmet, honest "no review phase data" detail.
	unset := gatherSignals(root, wf, nil, nil, "mvp", false, newVerdictLedger())
	if _, met := converge.Evaluate(wf.Stop.AllOf, unset); met {
		t.Error("an unrecorded executive-review verdict must NOT converge")
	}

	// APPROVE_WITH_SIMPLIFICATION recorded -> "approved" -> converges.
	approved := newVerdictLedger()
	approved.record("executive-review", VerdictApproveWithSimplification)
	sig := gatherSignals(root, wf, nil, nil, "mvp", false, approved)
	if sig.ReviewStatus != "approved" {
		t.Errorf("Signals.ReviewStatus = %q, want %q", sig.ReviewStatus, "approved")
	}
	if _, met := converge.Evaluate(wf.Stop.AllOf, sig); !met {
		t.Error("review_status==approved must converge once the executive verdict is APPROVE_WITH_SIMPLIFICATION")
	}

	// REDESIGN recorded -> meaningful non-"approved" detail, still unmet.
	redesign := newVerdictLedger()
	redesign.record("executive-review", VerdictRedesign)
	sig2 := gatherSignals(root, wf, nil, nil, "mvp", false, redesign)
	results, met := converge.Evaluate(wf.Stop.AllOf, sig2)
	if met {
		t.Error("review_status==approved must NOT converge on REDESIGN")
	}
	if !strings.Contains(results[0].Detail, "review_status=redesign") {
		t.Errorf("detail must name the real verdict, not a blank; got %q", results[0].Detail)
	}
}

// requirementConfidence must normalize the product-manager's recorded numeric
// requirement-discovery verdict — the mirror of TestReviewStatus_NormalizesExecutiveVerdict,
// but for a numeric string payload instead of a fixed token vocabulary. No phase
// in noConfidenceMetricWF declares confidence_metric, so confidenceMetricPhase
// falls back to the literal requirementDiscoveryPhase throughout — this test's
// contract is unchanged from before wf became field-driven.
func TestRequirementConfidence_NormalizesRecordedScore(t *testing.T) {
	var noConfidenceMetricWF asset.Workflow
	for _, want := range []float64{0, 50, 85, 100} {
		verdicts := newVerdictLedger()
		verdicts.record(requirementDiscoveryPhase, fmt.Sprintf("%.0f", want))
		if got := requirementConfidence(noConfidenceMetricWF, verdicts); got != want {
			t.Errorf("requirementConfidence() after recording %.0f = %v, want %v", want, got, want)
		}
	}
	if got := requirementConfidence(noConfidenceMetricWF, newVerdictLedger()); got != 0 {
		t.Errorf("no recorded score must yield 0 (honest absence); got %v", got)
	}
	if got := requirementConfidence(noConfidenceMetricWF, nil); got != 0 {
		t.Errorf("a nil verdictLedger must yield 0 (nil-safe); got %v", got)
	}
	// A verdict recorded for a DIFFERENT phase (e.g. the executive review) must not
	// leak into requirement_confidence — it is keyed strictly on
	// "requirement-discovery".
	other := newVerdictLedger()
	other.record(executiveReviewPhase, "85")
	if got := requirementConfidence(noConfidenceMetricWF, other); got != 0 {
		t.Errorf("a verdict recorded for a different phase must not resolve; got %v", got)
	}
	// A non-numeric stored value (defensive: should never happen, since only
	// parseConfidenceScore ever writes this phase's slot) must degrade to 0 rather
	// than panic.
	garbage := newVerdictLedger()
	garbage.record(requirementDiscoveryPhase, "not-a-number")
	if got := requirementConfidence(noConfidenceMetricWF, garbage); got != 0 {
		t.Errorf("an unparseable stored value must degrade to 0; got %v", got)
	}
}

// END-TO-END: gatherSignals -> converge.Converge on requirement_confidence >= 80 must
// flow through a populated verdictLedger exactly as discover.yml's stop_condition
// expects — the concrete proof the previously-permanent 0 (the sibling gap to
// ReviewStatus's) is closed for a REAL recorded confidence score, not just the
// parser unit. Mirrors TestGatherSignals_ReviewStatusEndToEnd.
func TestGatherSignals_RequirementConfidenceEndToEnd(t *testing.T) {
	root := t.TempDir()
	mkdir(t, filepath.Join(root, ".agent"))
	writeFile(t, filepath.Join(root, ".agent", "ROADMAP.md"), "- [x] done\n")

	wf := asset.Workflow{Stage: "discover", Stop: asset.StopCondition{
		Type: "conjunction",
		AllOf: []asset.Criterion{
			{Metric: "requirement_confidence", Operator: ">=", Threshold: ptrFloat(80)},
		},
	}}

	// No verdict recorded yet -> unmet, honest "no discover phase data" detail.
	unset := gatherSignals(root, wf, nil, nil, "mvp", false, newVerdictLedger())
	if _, met := converge.Evaluate(wf.Stop.AllOf, unset); met {
		t.Error("an unrecorded requirement-discovery confidence must NOT converge")
	}

	// 85 recorded -> >= 80 -> converges.
	confident := newVerdictLedger()
	confident.record(requirementDiscoveryPhase, "85")
	sig := gatherSignals(root, wf, nil, nil, "mvp", false, confident)
	if sig.RequirementConfidence != 85 {
		t.Errorf("Signals.RequirementConfidence = %v, want 85", sig.RequirementConfidence)
	}
	if _, met := converge.Evaluate(wf.Stop.AllOf, sig); !met {
		t.Error("requirement_confidence>=80 must converge once the recorded score is 85")
	}

	// 50 recorded -> below threshold, still unmet.
	unsure := newVerdictLedger()
	unsure.record(requirementDiscoveryPhase, "50")
	sig2 := gatherSignals(root, wf, nil, nil, "mvp", false, unsure)
	results, met := converge.Evaluate(wf.Stop.AllOf, sig2)
	if met {
		t.Error("requirement_confidence>=80 must NOT converge on a 50 score")
	}
	if !strings.Contains(results[0].Detail, "requirement_confidence=50") {
		t.Errorf("detail must name the real score, not a blank; got %q", results[0].Detail)
	}
}

// requirementConfidence must stay BYTE-FOR-BYTE unchanged for discover.yml's
// real shape: its requirement-discovery phase already declares
// confidence_metric: requirement_confidence, so confidenceMetricPhase finds it
// directly (not via the requirementDiscoveryPhase fallback) and resolves to
// that very same phase name — proving the generalization does not disturb the
// one workflow that exists today.
func TestRequirementConfidence_DiscoverYMLShapeUnchanged(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "requirement-discovery", ConfidenceMetric: "requirement_confidence"},
		{Name: "market-research"},
		{Name: "product-design"},
	}}
	verdicts := newVerdictLedger()
	verdicts.record("requirement-discovery", "85")
	if got := requirementConfidence(wf, verdicts); got != 85 {
		t.Errorf("requirementConfidence() on discover.yml's real shape = %v, want 85", got)
	}
}

// requirementConfidence must be FIELD-DRIVEN, not hardcoded to the literal
// phase name "requirement-discovery": a synthetic workflow whose confidence
// score comes from a DIFFERENTLY-NAMED phase that declares
// confidence_metric: requirement_confidence must still resolve correctly —
// the generalization this task exists for.
func TestRequirementConfidence_FieldDrivenDispatch(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "spec-intake", ConfidenceMetric: "requirement_confidence"},
	}}
	verdicts := newVerdictLedger()
	verdicts.record("spec-intake", "72")
	if got := requirementConfidence(wf, verdicts); got != 72 {
		t.Errorf("requirementConfidence() with a differently-named confidence_metric phase = %v, want 72", got)
	}

	// A verdict recorded under the OLD hardcoded literal name, but NOT under
	// the phase the workflow actually declares the metric on, must NOT
	// resolve — proof this is genuinely field-driven, not merely trying both
	// names.
	stale := newVerdictLedger()
	stale.record(requirementDiscoveryPhase, "99")
	if got := requirementConfidence(wf, stale); got != 0 {
		t.Errorf("a verdict recorded under the hardcoded literal name must NOT leak through when the workflow names a different phase; got %v", got)
	}
}

// ptrFloat is a small threshold-literal helper local to this test file (the asset
// package's Criterion.Threshold is a *float64).
func ptrFloat(f float64) *float64 { return &f }

// captureStdout runs fn with os.Stdout redirected to a pipe and returns what it
// printed, so a test can assert on the human-readable report lines.
func captureStdout(t *testing.T, fn func()) string {
	t.Helper()
	old := os.Stdout
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("pipe: %v", err)
	}
	os.Stdout = w
	fn()
	w.Close()
	os.Stdout = old
	out, err := io.ReadAll(r)
	if err != nil {
		t.Fatalf("read captured stdout: %v", err)
	}
	return string(out)
}

// humanGateWorkflow is the design.yml-shaped human_gate stop condition used by
// the report tests (type + required approval + the stage it unlocks).
func humanGateWorkflow() asset.Workflow {
	return asset.Workflow{
		Stage: "design",
		Stop: asset.StopCondition{
			Type:          "human_gate",
			HumanApproval: "required",
			OnApproved:    asset.OnApproved{NextStage: "build"},
		},
	}
}

// reportConvergence on a human_gate must print the HONEST awaiting message when
// there is no approval — distinct from a gate FAIL — and must NOT print a "MET".
// This is the CLI-level proof of the non-bypassable stop: no flag, no marker.
func TestReportConvergence_HumanGate_Awaiting(t *testing.T) {
	root := t.TempDir() // no --approved, no .forge/design.approved marker
	out := captureStdout(t, func() {
		reportConvergence(humanGateWorkflow(), root, nil, nil, "mvp", false, nil)
	})
	if !strings.Contains(out, "awaiting human approval (non-bypassable)") {
		t.Errorf("unapproved human_gate must report the awaiting message; got:\n%s", out)
	}
	if !strings.Contains(out, "NOT MET") {
		t.Errorf("unapproved human_gate must read NOT MET; got:\n%s", out)
	}
	if strings.Contains(out, "MET (human_gate) — approved") {
		t.Errorf("unapproved human_gate must NOT report approved; got:\n%s", out)
	}
}

// reportConvergence on a human_gate must report "approved -> unlocks <next_stage>"
// once the signal is present — via BOTH the flag and an on-disk marker.
func TestReportConvergence_HumanGate_Approved(t *testing.T) {
	// Approval via the flag.
	flagOut := captureStdout(t, func() {
		reportConvergence(humanGateWorkflow(), t.TempDir(), nil, nil, "mvp", true, nil)
	})
	for _, want := range []string{"MET (human_gate)", "approved", "next_stage=build"} {
		if !strings.Contains(flagOut, want) {
			t.Errorf("--approved report missing %q; got:\n%s", want, flagOut)
		}
	}

	// Approval via the on-disk marker (no flag).
	root := t.TempDir()
	mkdir(t, filepath.Dir(approvalPath(root, "design")))
	writeFile(t, approvalPath(root, "design"), "")
	markerOut := captureStdout(t, func() {
		reportConvergence(humanGateWorkflow(), root, nil, nil, "mvp", false, nil)
	})
	if !strings.Contains(markerOut, "approved → unlocks next_stage=build") {
		t.Errorf("marker-approved report must unlock next_stage; got:\n%s", markerOut)
	}
}

// TestIsTestPath_NonRootPythonConvention is a regression test: pytest's
// dominant layout puts tests in a tests/ subdirectory (e.g. "tests/test_foo.py"),
// which a full-path HasPrefix("test_") check misses entirely since the path
// starts with "tests/", not "test_". The check must run against the basename.
func TestIsTestPath_NonRootPythonConvention(t *testing.T) {
	cases := []struct {
		path string
		want bool
	}{
		{"test_foo.py", true},                        // root-level: basename == full path
		{"tests/test_foo.py", true},                  // pytest's dominant layout
		{"app/tests/test_bar.py", true},              // nested tests/ subdirectory
		{"forge-core/cmd/forge/gates_test.go", true}, // Go convention, unaffected
		{"internal/foo.go", false},
		{"app/testify.py", false}, // "test" prefix but not the "test_" convention
	}
	for _, c := range cases {
		if got := isTestPath(c.path); got != c.want {
			t.Errorf("isTestPath(%q) = %v, want %v", c.path, got, c.want)
		}
	}
}
