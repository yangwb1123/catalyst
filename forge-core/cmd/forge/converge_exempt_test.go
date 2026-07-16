package main

import (
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
)

// exemptWorkflow builds a workflow whose single phase requires the given gates —
// the de-duplicated required set gate.GatesGreen judges.
//
// The gate.GatesGreen-only unit tests (the lifecycle-aware N/A exemption matrix's
// four invariants) moved to internal/gate/resolve_test.go alongside GatesGreen
// itself (2026-07-02). This file keeps only the end-to-end test below, since it
// also exercises gatherSignals — cmd/forge orchestration, not gate-resolution.
func exemptWorkflow(gates ...string) asset.Workflow {
	return asset.Workflow{Phases: []asset.Phase{{Name: "verify", RequiredGates: gates}}}
}

// END-TO-END. gatherSignals → converge.Converge on gates_status==green must be
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

	sig := gatherSignals(root, wf, probe, cats, "mvp", false, nil)
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
