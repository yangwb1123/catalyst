package doctor

import (
	"strings"
	"testing"
)

// TestCheckWorkflowAgents_UnknownAgent_Flagged mirrors
// TestEvaluateWorkflowModels_UnknownAgent_Flagged: `forge validate` (no
// --models) must FAIL a workflow whose `agent` references a card that does
// not exist, on the same compact (single-line, no-space-after-colon) JSON
// shape this codebase's YAML->JSON transcoders actually produce.
func TestCheckWorkflowAgents_UnknownAgent_Flagged(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{
			{"name": "p1", "agent": "totally-not-a-real-agent"},
		},
	}
	out := compactWorkflowJSON(t, doc)
	known := map[string]bool{"planner": true}

	ok, findings := CheckWorkflowAgents("wf.yml", out, known)

	if ok {
		t.Fatal("CheckWorkflowAgents: ok=true, want false for an unknown agent reference")
	}
	if len(findings) != 1 || findings[0].Level != "FAIL" {
		t.Fatalf("findings = %+v, want exactly one FAIL", findings)
	}
	if !strings.Contains(findings[0].Message, "totally-not-a-real-agent") {
		t.Errorf("finding message = %q, want it to name the unknown agent", findings[0].Message)
	}
}

// TestCheckWorkflowAgents_KnownAgent_Passes: a known agent (plus the always-
// allowed "harness" role) produces no FAIL findings.
func TestCheckWorkflowAgents_KnownAgent_Passes(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{
			{"name": "p1", "agent": "planner"},
			{"name": "p2", "agent": "harness"}, // "harness" is always allowed, never a card
		},
	}
	out := compactWorkflowJSON(t, doc)
	known := map[string]bool{"planner": true}

	ok, findings := CheckWorkflowAgents("wf.yml", out, known)

	if !ok {
		t.Fatalf("ok = false, want true; findings=%+v", findings)
	}
	if len(findings) != 0 {
		t.Errorf("unexpected findings: %+v", findings)
	}
}

// TestCheckWorkflowAgents_LoopPhases covers evolve.yml's shape: phases
// nested under `loop.phases` rather than a top-level `phases` key.
func TestCheckWorkflowAgents_LoopPhases(t *testing.T) {
	doc := map[string]any{
		"loop": map[string]any{
			"loop_back_to": "scan",
			"phases": []map[string]any{
				{"name": "scan", "agent": "explorer"},
				{"name": "implement", "agent": "unknown-agent-in-loop"},
			},
		},
	}
	out := compactWorkflowJSON(t, doc)
	known := map[string]bool{"explorer": true}

	ok, findings := CheckWorkflowAgents("evolve.yml", out, known)

	if ok {
		t.Fatal("ok = true, want false for unknown agent nested under loop.phases")
	}
	found := false
	for _, f := range findings {
		if strings.Contains(f.Message, "unknown-agent-in-loop") {
			found = true
		}
	}
	if !found {
		t.Errorf("findings = %+v, want a FAIL naming unknown-agent-in-loop", findings)
	}
}

// TestCheckWorkflowAgents_UnparseableJSON covers the fail-closed path: if the
// input is not valid JSON at all, CheckWorkflowAgents must report a FAIL
// rather than silently treating it as an empty/passing workflow.
func TestCheckWorkflowAgents_UnparseableJSON(t *testing.T) {
	ok, findings := CheckWorkflowAgents("bad.yml", []byte("{not valid json"), nil)
	if ok {
		t.Fatal("ok = true, want false for unparseable JSON")
	}
	if len(findings) != 1 || findings[0].Level != "FAIL" {
		t.Fatalf("findings = %+v, want exactly one FAIL", findings)
	}
}

// TestCheckWorkflowAgents_LabelRoundTrips ensures the label argument (the
// caller's chosen identifier — full glob path for `forge validate`, relative
// path for `forge validate --models`) is echoed verbatim into finding
// messages, never recomputed or normalized internally.
func TestCheckWorkflowAgents_LabelRoundTrips(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{{"name": "p1", "agent": "ghost"}},
	}
	out := compactWorkflowJSON(t, doc)
	_, findings := CheckWorkflowAgents("/abs/path/to/wf.yml", out, nil)
	if len(findings) != 1 || !strings.HasPrefix(findings[0].Message, "/abs/path/to/wf.yml") {
		t.Errorf("findings = %+v, want message prefixed with the given label", findings)
	}
}
