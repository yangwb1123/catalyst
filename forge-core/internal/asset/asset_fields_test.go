package asset

import "testing"

// This file holds decode tests for Phase/Workflow fields added to carry
// requires_tools, readonly, and secondary_template through to JSON — split out
// of asset_test.go to keep both files under the repo's 500-line gate. Shared
// fixture helper (loadFixture) lives in asset_test.go, same package.

// RequiresTools is parsed VERBATIM from a phase's requires_tools (discover.yml's
// market-research: requires_tools: [web_search, web_fetch]). A phase without the
// key loads as nil/empty — the fault-tolerant default ("no declared tool
// requirement"). The command prompt builder consumes it through the
// degrade-to-advisory guard; this decode test pins the lower schema boundary.
func TestLoadWorkflowJSON_RequiresTools(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"discover","phases":[
		{"name":"market-research","agent":"researcher","requires_tools":["web_search","web_fetch"]},
		{"name":"requirement-discovery","agent":"product-manager"}
	]}`))
	if err != nil {
		t.Fatalf("load requires_tools doc: %v", err)
	}
	got := wf.Phases[0].RequiresTools
	if len(got) != 2 || got[0] != "web_search" || got[1] != "web_fetch" {
		t.Errorf("market-research RequiresTools = %v, want [web_search web_fetch]", got)
	}
	// A phase that omits requires_tools loads with a nil/empty default.
	if len(wf.Phases[1].RequiresTools) != 0 {
		t.Errorf("requirement-discovery RequiresTools = %v, want empty (no requires_tools key)", wf.Phases[1].RequiresTools)
	}
}

// Readonly is parsed VERBATIM at BOTH workflow level and phase level (every
// .agent/workflows/*.yml authors it at the top level; build.yml varies it per
// phase: false on implementer, true on planner/harness-gates/reviewer/qa).
// The command executor enforces the effective phase boundary with `dontAsk` and
// validated write scopes; this test pins that both schema levels survive decode.
func TestLoadWorkflowJSON_Readonly(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"review","readonly":true,"phases":[
		{"name":"security-review","agent":"security-engineer","readonly":true},
		{"name":"implementer","agent":"implementer","readonly":false},
		{"name":"planner","agent":"planner"}
	]}`))
	if err != nil {
		t.Fatalf("load readonly doc: %v", err)
	}
	if !wf.Readonly {
		t.Error("workflow Readonly = false, want true (workflow-level readonly: true)")
	}
	if !wf.Phases[0].Readonly {
		t.Error("security-review Readonly = false, want true")
	}
	if wf.Phases[1].Readonly {
		t.Error("implementer Readonly = true, want false (explicit readonly: false)")
	}
	// A phase that omits readonly loads with the false default.
	if wf.Phases[2].Readonly {
		t.Error("planner Readonly = true, want false (no readonly key)")
	}
}

// Back-compat: the committed build.json fixture already authors readonly on
// every phase (planner/harness-gates/reviewer/qa: true, implementer: false),
// but no workflow-level readonly — this pins the fixture's real values decode
// correctly now that the field exists, and the workflow level stays the
// fault-tolerant false default.
func TestLoadWorkflowJSON_ReadonlyFixture(t *testing.T) {
	wf := loadFixture(t)
	if wf.Readonly {
		t.Errorf("workflow Readonly = true, want false (fixture has no top-level readonly)")
	}
	wantReadonly := map[string]bool{
		"planner":       true,
		"implementer":   false,
		"harness-gates": true,
		"reviewer":      true,
		"qa":            true,
	}
	for _, p := range wf.Phases {
		want, ok := wantReadonly[p.Name]
		if !ok {
			t.Fatalf("unexpected phase %q in fixture", p.Name)
		}
		if p.Readonly != want {
			t.Errorf("phase %q Readonly = %v, want %v", p.Name, p.Readonly, want)
		}
	}
}

// SecondaryTemplate is parsed VERBATIM from a phase's secondary_template
// (review.yml's performance-reliability-review pairs it with uses_template).
// A phase without the key loads as "" — the fault-tolerant default. Prompt
// injection and doctor validation consume the field; this test pins decode.
func TestLoadWorkflowJSON_SecondaryTemplate(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"review","phases":[
		{"name":"performance-reliability-review","agent":"performance-engineer",
		 "uses_template":".ai/prompts/05-performance-review.md",
		 "secondary_template":".ai/prompts/06-production-readiness.md"},
		{"name":"security-review","agent":"security-engineer","uses_template":".ai/prompts/02-security-rfc-review.md"}
	]}`))
	if err != nil {
		t.Fatalf("load secondary_template doc: %v", err)
	}
	p0 := wf.Phases[0]
	if p0.UsesTemplate != ".ai/prompts/05-performance-review.md" {
		t.Errorf("performance-reliability-review UsesTemplate = %q, want the primary template", p0.UsesTemplate)
	}
	if p0.SecondaryTemplate != ".ai/prompts/06-production-readiness.md" {
		t.Errorf("performance-reliability-review SecondaryTemplate = %q, want the production-readiness template", p0.SecondaryTemplate)
	}
	// A phase that omits secondary_template loads with the empty default.
	if wf.Phases[1].SecondaryTemplate != "" {
		t.Errorf("security-review SecondaryTemplate = %q, want empty (no secondary_template key)", wf.Phases[1].SecondaryTemplate)
	}
}

// Deploy uses only existing workflow fields: concrete emits, a validation
// on_fail loop-back, and a human gate whose approval advances to Evolve.
// This test proves the generic loader preserves that declaration without
// adding cloud/provider-specific runtime types or behavior.
func TestLoadWorkflowJSON_DeclarativeDeployBoundary(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"deploy","readonly":true,"phases":[
		{"name":"release-planning","agent":"release-engineer","readonly":true,
		 "emits":["docs/release/release-manifest.yml","docs/release/deployment-plan.md"]},
		{"name":"release-plan-validation","agent":"release-engineer","readonly":true,
		 "emits":["docs/release/deployment-validation.md"],
		 "on_fail":{"action":"loop_back","target_phase":"release-planning"}}
	],"stop_condition":{"type":"human_gate","human_approval":"required",
	 "on_rejected":{"action":"loop_back","target_phase":"release-planning"},
	 "on_approved":{"next_stage":"evolve"}}}`))
	if err != nil {
		t.Fatalf("load deploy workflow: %v", err)
	}
	if wf.Stage != "deploy" || !wf.Readonly || len(wf.Phases) != 2 {
		t.Fatalf("deploy shape = stage %q readonly %v phases %d", wf.Stage, wf.Readonly, len(wf.Phases))
	}
	if got := wf.Phases[0].Emits; len(got) != 2 || got[0] != "docs/release/release-manifest.yml" {
		t.Errorf("planning emits = %v, want concrete docs/release paths", got)
	}
	if fail := wf.Phases[1].OnFail; fail == nil || fail.TargetPhase != "release-planning" {
		t.Errorf("validation on_fail = %+v, want release-planning", fail)
	}
	if rejected := wf.Stop.OnRejected; rejected == nil || rejected.TargetPhase != "release-planning" {
		t.Errorf("stop on_rejected = %+v, want release-planning", rejected)
	}
	if wf.Stop.HumanApproval != "required" || wf.Stop.OnApproved.NextStage != "evolve" {
		t.Errorf("deploy stop = %+v, want required approval -> evolve", wf.Stop)
	}
}
