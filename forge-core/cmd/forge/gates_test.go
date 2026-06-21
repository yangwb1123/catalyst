package main

import (
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

	sig := gatherSignals(root, wf, probe, nil, "mvp", false)

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
	pass := gatherSignals(root, wf, map[string]string{"test_pass": "PASS"}, nil, "mvp", false)
	results, met := converge.Evaluate(stop, pass)
	if !met || !results[0].Met {
		t.Errorf("test_pass=PASS should converge per-criterion; got met=%v results=%+v", met, results)
	}

	// FAIL probe -> unmet; the SAME reused probe map drives the verdict honestly.
	fail := gatherSignals(root, wf, map[string]string{"test_pass": "FAIL"}, nil, "mvp", false)
	if _, met := converge.Evaluate(stop, fail); met {
		t.Error("test_pass=FAIL must NOT converge")
	}

	// Absent verdict (nil/broken probe) -> unmet (absence is never satisfaction).
	absent := gatherSignals(root, wf, nil, nil, "mvp", false)
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
	sig := gatherSignals(root, asset.Workflow{}, probe, nil, "mvp", false)
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
	if sig := gatherSignals(root, asset.Workflow{}, nil, nil, "mvp", true); !sig.HumanApproved {
		t.Error("gatherSignals(approved=true) must set Signals.HumanApproved")
	}
	if sig := gatherSignals(root, asset.Workflow{}, nil, nil, "mvp", false); sig.HumanApproved {
		t.Error("gatherSignals(approved=false) must leave Signals.HumanApproved false")
	}
}

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
		reportConvergence(humanGateWorkflow(), root, nil, nil, "mvp", false)
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
		reportConvergence(humanGateWorkflow(), t.TempDir(), nil, nil, "mvp", true)
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
		reportConvergence(humanGateWorkflow(), root, nil, nil, "mvp", false)
	})
	if !strings.Contains(markerOut, "approved → unlocks next_stage=build") {
		t.Errorf("marker-approved report must unlock next_stage; got:\n%s", markerOut)
	}
}
