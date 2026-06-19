package asset

import (
	"os"
	"path/filepath"
	"testing"
)

// loadFixture reads the committed build.json fixture. It is intentionally a
// self-contained asset under testdata/ so this suite never depends on any
// out-of-band transcoder (e.g. harness/yaml2json.py) or a sibling agent.
func loadFixture(t *testing.T) Workflow {
	t.Helper()
	data, err := os.ReadFile(filepath.Join("testdata", "build.json"))
	if err != nil {
		t.Fatalf("read fixture: %v", err)
	}
	wf, err := LoadWorkflowJSON(data)
	if err != nil {
		t.Fatalf("LoadWorkflowJSON: %v", err)
	}
	return wf
}

func TestLoadWorkflowJSON_Fixture(t *testing.T) {
	wf := loadFixture(t)

	if wf.Stage != "build" {
		t.Errorf("stage = %q, want %q", wf.Stage, "build")
	}
	if got := len(wf.Phases); got != 5 {
		t.Fatalf("phase count = %d, want 5", got)
	}
	if wf.Phases[0].Name != "planner" || wf.Phases[0].Agent != "planner" {
		t.Errorf("phase[0] = %+v, want planner/planner", wf.Phases[0])
	}
	if !wf.Phases[0].Readonly {
		t.Error("planner phase should be readonly")
	}
	if wf.Phases[1].Readonly {
		t.Error("implementer phase should not be readonly")
	}
}

func TestLoadWorkflowJSON_RequiredGates(t *testing.T) {
	wf := loadFixture(t)

	// implementer carries no gates; harness phase carries the full set.
	if len(wf.Phases[1].RequiredGates) != 0 {
		t.Errorf("implementer gates = %v, want empty", wf.Phases[1].RequiredGates)
	}
	harness := wf.Phases[2]
	if harness.Agent != "harness" {
		t.Fatalf("phase[2].Agent = %q, want harness", harness.Agent)
	}
	if got := len(harness.RequiredGates); got != 6 {
		t.Errorf("harness gates = %d, want 6", got)
	}
	if harness.RequiredGates[0] != "lint" {
		t.Errorf("first gate = %q, want lint", harness.RequiredGates[0])
	}
}

// RequiredWhen is stored VERBATIM (fragment and all). The reviewer phase carries
// the mode-gating reference from build.yml; every other phase leaves it empty
// (the "always run" default), so adding the field regressed no phase.
func TestLoadWorkflowJSON_RequiredWhen(t *testing.T) {
	wf := loadFixture(t)

	reviewer := wf.Phases[3]
	if reviewer.Name != "reviewer" {
		t.Fatalf("phase[3].Name = %q, want reviewer", reviewer.Name)
	}
	if reviewer.RequiredWhen != "../policies/modes.yml#workflow_depth.reviewer" {
		t.Errorf("reviewer.RequiredWhen = %q, want the verbatim modes.yml fragment", reviewer.RequiredWhen)
	}
	// Every non-reviewer phase must leave RequiredWhen empty ("always run").
	for i, p := range wf.Phases {
		if i == 3 {
			continue
		}
		if p.RequiredWhen != "" {
			t.Errorf("phase[%d] (%s) RequiredWhen = %q, want empty (always run)", i, p.Name, p.RequiredWhen)
		}
	}
}

// ModelTier is parsed VERBATIM from a phase's model_tier (build.yml authors
// implementer: sonnet, reviewer: opus). A phase without the key loads as "" — the
// fault-tolerant default the orchestrator reads as "no override, use the agent's
// routed tier". This pins that the field is purely additive and that a tier hint
// is carried through to the runtime, not silently dropped.
func TestLoadWorkflowJSON_ModelTier(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"build","phases":[
		{"name":"implementer","agent":"implementer","model_tier":"sonnet"},
		{"name":"reviewer","agent":"reviewer","model_tier":"opus"},
		{"name":"planner","agent":"planner"}
	]}`))
	if err != nil {
		t.Fatalf("load model_tier doc: %v", err)
	}
	if got := wf.Phases[0].ModelTier; got != "sonnet" {
		t.Errorf("implementer ModelTier = %q, want sonnet", got)
	}
	if got := wf.Phases[1].ModelTier; got != "opus" {
		t.Errorf("reviewer ModelTier = %q, want opus", got)
	}
	// A phase that omits model_tier loads with the empty default ("no override").
	if got := wf.Phases[2].ModelTier; got != "" {
		t.Errorf("planner ModelTier = %q, want empty (no override)", got)
	}
}

// Back-compat: the committed fixture (build.json) authors NO model_tier on any
// phase, so every phase must load with ModelTier "" — adding the field dropped
// nothing and changed no existing phase's parse.
func TestLoadWorkflowJSON_ModelTierAbsentIsEmpty(t *testing.T) {
	wf := loadFixture(t)
	for i, p := range wf.Phases {
		if p.ModelTier != "" {
			t.Errorf("phase[%d] (%s) ModelTier = %q, want empty (fixture has no model_tier)", i, p.Name, p.ModelTier)
		}
	}
}

// OnFail is parsed for the gate phases that declare it (harness-gates/reviewer/qa
// in build.yml all loop_back to implementer) and is nil for phases without the key
// (planner/implementer) — the fault-tolerant default the orchestrator reads as
// "no directed loop-back, abort on a red gate" (back-compat).
func TestLoadWorkflowJSON_OnFail(t *testing.T) {
	wf := loadFixture(t)

	// planner and implementer carry no on_fail -> nil.
	for _, i := range []int{0, 1} {
		if wf.Phases[i].OnFail != nil {
			t.Errorf("phase[%d] (%s) OnFail = %+v, want nil (no directed loop-back)", i, wf.Phases[i].Name, wf.Phases[i].OnFail)
		}
	}
	// harness-gates declares a directed loop-back to implementer.
	harness := wf.Phases[2]
	if harness.OnFail == nil {
		t.Fatalf("harness-gates OnFail = nil, want a loop_back directive")
	}
	if harness.OnFail.Action != "loop_back" || harness.OnFail.TargetPhase != "implementer" {
		t.Errorf("harness-gates OnFail = %+v, want {loop_back, implementer}", *harness.OnFail)
	}
	// reviewer and qa likewise loop back to implementer.
	for _, i := range []int{3, 4} {
		of := wf.Phases[i].OnFail
		if of == nil || of.Action != "loop_back" || of.TargetPhase != "implementer" {
			t.Errorf("phase[%d] (%s) OnFail = %+v, want {loop_back, implementer}", i, wf.Phases[i].Name, of)
		}
	}
}

// OnUnmet is parsed for the conjunction stop's directed restart (loop to the
// planner for the next roadmap item).
func TestLoadWorkflowJSON_OnUnmet(t *testing.T) {
	wf := loadFixture(t)
	ou := wf.Stop.OnUnmet
	if ou == nil {
		t.Fatalf("stop.OnUnmet = nil, want a loop_to_next_roadmap_item directive")
	}
	if ou.Action != "loop_to_next_roadmap_item" || ou.TargetPhase != "planner" {
		t.Errorf("stop.OnUnmet = %+v, want {loop_to_next_roadmap_item, planner}", *ou)
	}
}

// Fault tolerance: a workflow whose phases/stop omit on_fail/on_unmet loads with
// those pointers nil — the back-compat default (abort on red, full replay), never
// a parse error. This pins that the new fields are PURELY additive.
func TestLoadWorkflowJSON_OnFailOnUnmetAbsentIsNil(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"build","phases":[
		{"name":"harness-gates","agent":"harness","required_gates":["test"]}
	],"stop_condition":{"type":"conjunction","all_of":[
		{"metric":"roadmap_completion","operator":"==","threshold":100}
	]}}`))
	if err != nil {
		t.Fatalf("absent on_fail/on_unmet should load, got error: %v", err)
	}
	if wf.Phases[0].OnFail != nil {
		t.Errorf("absent on_fail must be nil; got %+v", wf.Phases[0].OnFail)
	}
	if wf.Stop.OnUnmet != nil {
		t.Errorf("absent on_unmet must be nil; got %+v", wf.Stop.OnUnmet)
	}
}

func TestLoadWorkflowJSON_Stop(t *testing.T) {
	wf := loadFixture(t)

	if wf.Stop.Type != "conjunction" {
		t.Errorf("stop.Type = %q, want conjunction", wf.Stop.Type)
	}
	if len(wf.Stop.AllOf) != 2 {
		t.Fatalf("stop.AllOf = %d criteria, want 2", len(wf.Stop.AllOf))
	}
	// The real build.yml authors all_of as typed objects, not bare strings.
	c0 := wf.Stop.AllOf[0]
	if c0.Metric != "roadmap_completion" || c0.Operator != "==" || c0.Threshold == nil || *c0.Threshold != 100 {
		t.Errorf("criterion[0] = %+v, want roadmap_completion == 100", c0)
	}
	c1 := wf.Stop.AllOf[1]
	if c1.Metric != "gates_status" || c1.Value != "green" {
		t.Errorf("criterion[1] = %+v, want gates_status == green", c1)
	}
	if wf.Stop.AntiPattern != "round_count" {
		t.Errorf("stop.AntiPattern = %q, want round_count", wf.Stop.AntiPattern)
	}
}

// Criterion accepts BOTH a JSON object and a bare JSON string. The object
// populates the typed fields; the bare string is preserved in Raw so a
// human-string fixture is never silently dropped.
func TestCriterion_UnmarshalBothForms(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stop_condition":{"type":"conjunction","all_of":[
		{"metric":"roadmap_completion","operator":">=","threshold":80},
		"roadmap_completion == 100%"
	]}}`))
	if err != nil {
		t.Fatalf("load mixed all_of: %v", err)
	}
	if len(wf.Stop.AllOf) != 2 {
		t.Fatalf("all_of = %d, want 2", len(wf.Stop.AllOf))
	}
	obj := wf.Stop.AllOf[0]
	if obj.Metric != "roadmap_completion" || obj.Operator != ">=" || obj.Threshold == nil || *obj.Threshold != 80 {
		t.Errorf("object form mis-parsed: %+v", obj)
	}
	if obj.Raw != "" {
		t.Errorf("object form should not set Raw; got %q", obj.Raw)
	}
	bare := wf.Stop.AllOf[1]
	if bare.Raw != "roadmap_completion == 100%" {
		t.Errorf("bare-string form should land in Raw; got %+v", bare)
	}
	if bare.Metric != "" {
		t.Errorf("bare-string form should have empty Metric; got %q", bare.Metric)
	}
}

// A human_gate stop condition (design.yml) parses its human_approval flag and
// the next_stage an approval unlocks, while leaving the conjunction fields
// (all_of) absent — the human gate is not a conjunction.
func TestLoadWorkflowJSON_HumanGate(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"design","stop_condition":{
		"type":"human_gate",
		"human_approval":"required",
		"durable_wait":true,
		"on_approved":{"next_stage":"build","emit":[".agent/PROJECT.md"]},
		"on_rejected":{"action":"loop_back"}
	}}`))
	if err != nil {
		t.Fatalf("load human_gate: %v", err)
	}
	if wf.Stop.Type != "human_gate" {
		t.Errorf("stop.Type = %q, want human_gate", wf.Stop.Type)
	}
	if wf.Stop.HumanApproval != "required" {
		t.Errorf("stop.HumanApproval = %q, want required", wf.Stop.HumanApproval)
	}
	if wf.Stop.OnApproved.NextStage != "build" {
		t.Errorf("stop.OnApproved.NextStage = %q, want build", wf.Stop.OnApproved.NextStage)
	}
	// A human_gate carries no all_of; the unmodeled keys (durable_wait, emit,
	// on_rejected) are ignored by the fault-tolerant loader, not an error.
	if len(wf.Stop.AllOf) != 0 {
		t.Errorf("human_gate should have no all_of; got %v", wf.Stop.AllOf)
	}
}

// Back-compat: the conjunction fixture (build.json) must still parse with the new
// human-gate fields simply zero-valued — adding them dropped nothing.
func TestLoadWorkflowJSON_ConjunctionUnaffectedByHumanFields(t *testing.T) {
	wf := loadFixture(t)
	if wf.Stop.HumanApproval != "" {
		t.Errorf("conjunction stop must have empty HumanApproval; got %q", wf.Stop.HumanApproval)
	}
	if wf.Stop.OnApproved.NextStage != "" {
		t.Errorf("conjunction stop must have empty OnApproved.NextStage; got %q", wf.Stop.OnApproved.NextStage)
	}
	// The pre-existing conjunction assertions still hold (nothing regressed).
	if wf.Stop.Type != "conjunction" || len(wf.Stop.AllOf) != 2 {
		t.Errorf("conjunction fixture changed: type=%q all_of=%d", wf.Stop.Type, len(wf.Stop.AllOf))
	}
}

// Fault tolerance: missing fields must not crash; only bad syntax errors.
func TestLoadWorkflowJSON_FaultTolerant(t *testing.T) {
	wf, err := LoadWorkflowJSON([]byte(`{"stage":"design"}`))
	if err != nil {
		t.Fatalf("partial doc should load, got error: %v", err)
	}
	if wf.Stage != "design" {
		t.Errorf("stage = %q, want design", wf.Stage)
	}
	if wf.Phases != nil {
		t.Errorf("missing phases should be nil, got %v", wf.Phases)
	}

	if _, err := LoadWorkflowJSON([]byte(`{not json`)); err == nil {
		t.Error("invalid JSON should return an error")
	}
}
