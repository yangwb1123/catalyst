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
