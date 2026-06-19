package main

import (
	"path/filepath"
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

	sig := gatherSignals(root, wf, probe)

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
	pass := gatherSignals(root, wf, map[string]string{"test_pass": "PASS"})
	results, met := converge.Evaluate(stop, pass)
	if !met || !results[0].Met {
		t.Errorf("test_pass=PASS should converge per-criterion; got met=%v results=%+v", met, results)
	}

	// FAIL probe -> unmet; the SAME reused probe map drives the verdict honestly.
	fail := gatherSignals(root, wf, map[string]string{"test_pass": "FAIL"})
	if _, met := converge.Evaluate(stop, fail); met {
		t.Error("test_pass=FAIL must NOT converge")
	}

	// Absent verdict (nil/broken probe) -> unmet (absence is never satisfaction).
	absent := gatherSignals(root, wf, nil)
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
	sig := gatherSignals(root, asset.Workflow{}, probe)
	if sig.RoadmapCompletion != 0 {
		t.Errorf("RoadmapCompletion = %v, want 0 (no ROADMAP)", sig.RoadmapCompletion)
	}
	if sig.Criteria["test_pass"] != "PASS" {
		t.Errorf("Criteria not wired when ROADMAP absent: %v", sig.Criteria)
	}
}
