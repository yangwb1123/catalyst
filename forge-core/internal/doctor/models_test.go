package doctor

import (
	"encoding/json"
	"strings"
	"testing"
)

// compactWorkflowJSON mimics what this codebase's real JSON producers emit:
// json.Marshal on a decoded YAML document (native yaml2json.Decode, and the
// harness/yaml2json.py shim's default json.dumps) — a single-line, COMPACT
// document with no space after ":". A regression here is exactly the bug
// this package shipped with: a `strings.Split` + `"key": "value"` (WITH a
// space) line-scan that only matches pretty-printed JSON and silently finds
// nothing against this shape.
func compactWorkflowJSON(t *testing.T, v any) []byte {
	t.Helper()
	data, err := json.Marshal(v)
	if err != nil {
		t.Fatalf("json.Marshal fixture: %v", err)
	}
	if strings.Contains(string(data), ": ") {
		t.Fatalf("fixture is not compact JSON (contains space after colon): %s", data)
	}
	return data
}

// TestEvaluateWorkflowModels_UnknownAgent_Flagged is the regression test for
// the headline bug: forge validate --models must FAIL a workflow whose
// `agent` references a card that does not exist, even though the input is
// compact (single-line, no-space-after-colon) JSON — the shape the old
// line-scan silently passed straight through.
func TestEvaluateWorkflowModels_UnknownAgent_Flagged(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{
			{"name": "p1", "agent": "totally-not-a-real-agent"},
		},
	}
	out := compactWorkflowJSON(t, doc)
	known := map[string]bool{"planner": true}

	ok, findings := EvaluateWorkflowModels("wf.yml", out, known, nil)

	if ok {
		t.Fatal("EvaluateWorkflowModels: ok=true, want false for an unknown agent reference")
	}
	if len(findings) != 1 || findings[0].Level != "FAIL" {
		t.Fatalf("findings = %+v, want exactly one FAIL", findings)
	}
	if !strings.Contains(findings[0].Message, "totally-not-a-real-agent") {
		t.Errorf("finding message = %q, want it to name the unknown agent", findings[0].Message)
	}
}

// TestEvaluateWorkflowModels_KnownAgent_Passes is the positive-case sibling
// of the regression test above: a known agent on compact JSON must produce
// no FAIL findings at all.
func TestEvaluateWorkflowModels_KnownAgent_Passes(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{
			{"name": "p1", "agent": "planner"},
			{"name": "p2", "agent": "harness"}, // "harness" is always allowed, never a card
		},
	}
	out := compactWorkflowJSON(t, doc)
	known := map[string]bool{"planner": true}

	ok, findings := EvaluateWorkflowModels("wf.yml", out, known, nil)

	if !ok {
		t.Fatalf("ok = false, want true; findings=%+v", findings)
	}
	for _, f := range findings {
		if f.Level == "FAIL" {
			t.Errorf("unexpected FAIL finding: %+v", f)
		}
	}
}

// TestEvaluateWorkflowModels_LoopPhases covers evolve.yml's shape: phases
// nested under `loop.phases` rather than a top-level `phases` key.
func TestEvaluateWorkflowModels_LoopPhases(t *testing.T) {
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

	ok, findings := EvaluateWorkflowModels("evolve.yml", out, known, nil)

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

// TestEvaluateWorkflowModels_UsesTemplate covers both the WARN (missing) and
// PASS (present) outcomes for uses_template — neither of which flips the
// workflow's overall ok verdict.
func TestEvaluateWorkflowModels_UsesTemplate(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{
			{"name": "p1", "agent": "harness", "uses_template": ".ai/prompts/present.md"},
			{"name": "p2", "agent": "harness", "uses_template": ".ai/prompts/missing.md"},
		},
	}
	out := compactWorkflowJSON(t, doc)
	aiTemplates := map[string]bool{"present.md": true}

	ok, findings := EvaluateWorkflowModels("wf.yml", out, map[string]bool{}, aiTemplates)

	if !ok {
		t.Fatalf("ok = false, want true (uses_template never fails the verdict); findings=%+v", findings)
	}
	var gotPass, gotWarn bool
	for _, f := range findings {
		switch {
		case f.Level == "PASS" && strings.Contains(f.Message, "present.md"):
			gotPass = true
		case f.Level == "WARN" && strings.Contains(f.Message, "missing.md"):
			gotWarn = true
		}
	}
	if !gotPass || !gotWarn {
		t.Errorf("findings = %+v, want a PASS for present.md and a WARN for missing.md", findings)
	}
}

// TestEvaluateWorkflowModels_SecondaryTemplate mirrors
// TestEvaluateWorkflowModels_UsesTemplate exactly, for the OPTIONAL
// secondary_template field review.yml's performance-reliability-review phase
// pairs alongside uses_template (05-performance-review.md +
// 06-production-readiness.md). Both the WARN (missing) and PASS (present)
// outcomes must fire, and neither flips the workflow's overall ok verdict —
// byte-for-byte the same contract as uses_template.
func TestEvaluateWorkflowModels_SecondaryTemplate(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{
			{"name": "p1", "agent": "harness", "uses_template": ".ai/prompts/present.md", "secondary_template": ".ai/prompts/present2.md"},
			{"name": "p2", "agent": "harness", "secondary_template": ".ai/prompts/missing2.md"},
		},
	}
	out := compactWorkflowJSON(t, doc)
	aiTemplates := map[string]bool{"present.md": true, "present2.md": true}

	ok, findings := EvaluateWorkflowModels("wf.yml", out, map[string]bool{}, aiTemplates)

	if !ok {
		t.Fatalf("ok = false, want true (secondary_template never fails the verdict); findings=%+v", findings)
	}
	var gotPass, gotWarn bool
	for _, f := range findings {
		switch {
		case f.Level == "PASS" && strings.Contains(f.Message, "secondary_template") && strings.Contains(f.Message, "present2.md"):
			gotPass = true
		case f.Level == "WARN" && strings.Contains(f.Message, "secondary_template") && strings.Contains(f.Message, "missing2.md"):
			gotWarn = true
		}
	}
	if !gotPass || !gotWarn {
		t.Errorf("findings = %+v, want a PASS for present2.md and a WARN for missing2.md, both naming secondary_template", findings)
	}
}

// TestEvaluateWorkflowModels_NoSecondaryTemplate_NoFindings confirms a phase
// that omits secondary_template (the default — every phase except review.yml's
// performance-reliability-review) produces no secondary_template finding at
// all, so the new field is byte-for-byte additive and never emits noise for
// workflows that never set it.
func TestEvaluateWorkflowModels_NoSecondaryTemplate_NoFindings(t *testing.T) {
	doc := map[string]any{
		"phases": []map[string]any{
			{"name": "p1", "agent": "harness", "uses_template": ".ai/prompts/present.md"},
		},
	}
	out := compactWorkflowJSON(t, doc)
	aiTemplates := map[string]bool{"present.md": true}

	_, findings := EvaluateWorkflowModels("wf.yml", out, map[string]bool{}, aiTemplates)

	for _, f := range findings {
		if strings.Contains(f.Message, "secondary_template") {
			t.Errorf("unexpected secondary_template finding for a phase that never set it: %+v", f)
		}
	}
}

// targetPhaseCase is one TestEvaluateWorkflowModels_TargetPhase table row.
type targetPhaseCase struct {
	name       string
	phases     []map[string]any
	wantOK     bool
	wantTarget string // substring expected in the FAIL message, "" if wantOK
}

// targetPhaseCases covers on_fail.target_phase: a reference to an earlier
// phase passes, and a forward-reference (or a name that never appears at
// all) is reported as not-found.
func targetPhaseCases() []targetPhaseCase {
	backward := []map[string]any{
		{"name": "implement", "agent": "harness"},
		{"name": "review", "agent": "harness", "on_fail": map[string]any{"target_phase": "implement"}},
	}
	forward := []map[string]any{
		{"name": "review", "agent": "harness", "on_fail": map[string]any{"target_phase": "implement"}},
		{"name": "implement", "agent": "harness"},
	}
	unknown := []map[string]any{
		{"name": "review", "agent": "harness", "on_fail": map[string]any{"target_phase": "no-such-phase"}},
	}
	// A phase whose on_fail loops back to ITSELF (e.g. review.yml's
	// security-review re-running on its own failure) is a real, legitimate
	// shape and must pass — the phase's own name is "seen" before its own
	// on_fail is checked.
	selfRef := []map[string]any{
		{"name": "security-review", "agent": "harness", "on_fail": map[string]any{"target_phase": "security-review"}},
	}
	return []targetPhaseCase{
		{name: "backward reference passes", phases: backward, wantOK: true},
		{name: "forward reference fails", phases: forward, wantOK: false, wantTarget: "implement"},
		{name: "unknown phase name fails", phases: unknown, wantOK: false, wantTarget: "no-such-phase"},
		{name: "self reference passes", phases: selfRef, wantOK: true},
	}
}

// TestEvaluateWorkflowModels_TargetPhase runs the targetPhaseCases table.
func TestEvaluateWorkflowModels_TargetPhase(t *testing.T) {
	for _, tc := range targetPhaseCases() {
		t.Run(tc.name, func(t *testing.T) {
			out := compactWorkflowJSON(t, map[string]any{"phases": tc.phases})
			ok, findings := EvaluateWorkflowModels("wf.yml", out, map[string]bool{}, nil)
			if ok != tc.wantOK {
				t.Fatalf("ok = %v, want %v; findings=%+v", ok, tc.wantOK, findings)
			}
			if !tc.wantOK {
				var matched bool
				for _, f := range findings {
					if f.Level == "FAIL" && strings.Contains(f.Message, tc.wantTarget) {
						matched = true
					}
				}
				if !matched {
					t.Errorf("findings = %+v, want a FAIL mentioning %q", findings, tc.wantTarget)
				}
			}
		})
	}
}

// TestEvaluateWorkflowModels_UnparseableJSON covers the fail-closed path: if
// the input is not valid JSON at all, EvaluateWorkflowModels must report a
// FAIL rather than silently treating it as an empty/passing workflow.
func TestEvaluateWorkflowModels_UnparseableJSON(t *testing.T) {
	ok, findings := EvaluateWorkflowModels("bad.yml", []byte("{not valid json"), nil, nil)
	if ok {
		t.Fatal("ok = true, want false for unparseable JSON")
	}
	if len(findings) != 1 || findings[0].Level != "FAIL" {
		t.Fatalf("findings = %+v, want exactly one FAIL", findings)
	}
}
