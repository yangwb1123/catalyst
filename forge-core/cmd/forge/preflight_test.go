package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

// TestCheckCostEstimate_SkipsGateOnlyPhases is a regression test:
// checkCostEstimate used to sum a Sonnet/Opus tier over EVERY phase,
// including gate-only phases (len(RequiredGates)>0) that orchestrator.RunFrom
// never actually spawns an agent for — inflating the printed dollar estimate
// past the agent-call count checkWorkflowEstimates prints one line earlier.
// A workflow with 1 real agent phase + 1 gate-only phase must estimate cost
// for exactly 1 phase, not 2.
func TestCheckCostEstimate_SkipsGateOnlyPhases(t *testing.T) {
	wf := asset.Workflow{Phases: []asset.Phase{
		{Name: "implementer", Agent: "implementer"},
		{Name: "harness-gates", RequiredGates: []string{"test"}},
	}}
	rep := &preflightReport{}
	out := captureStdout(t, func() {
		checkCostEstimate(wf, "engineering", 1, rep)
	})
	// Exactly 1 phase (the agent phase) should be counted, never 2.
	if strings.Contains(out, "2 × Sonnet") || strings.Contains(out, "2 × Opus") {
		t.Errorf("gate-only phase counted toward cost estimate; got: %s", out)
	}
	if !strings.Contains(out, "1 × Sonnet") && !strings.Contains(out, "1 × Opus") {
		t.Errorf("expected exactly 1 phase counted; got: %s", out)
	}
}
