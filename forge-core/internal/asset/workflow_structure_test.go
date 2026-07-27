package asset

import (
	"strings"
	"testing"
)

func TestLoadWorkflowJSONRejectsAmbiguousPhaseIdentity(t *testing.T) {
	tests := []struct {
		name string
		data string
		want string
	}{
		{
			name: "empty",
			data: `{"stage":"build","phases":[{"name":"","agent":"planner"}]}`,
			want: "empty name",
		},
		{
			name: "blank",
			data: `{"stage":"build","phases":[{"name":" \t ","agent":"planner"}]}`,
			want: "empty name",
		},
		{
			name: "duplicate",
			data: `{"stage":"build","phases":[
				{"name":"implement","agent":"implementer"},
				{"name":"implement","agent":"reviewer"}
			]}`,
			want: `duplicates phase name "implement"`,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if _, err := LoadWorkflowJSON([]byte(tc.data)); err == nil ||
				!strings.Contains(err.Error(), tc.want) {
				t.Fatalf("LoadWorkflowJSON error = %v, want substring %q", err, tc.want)
			}
		})
	}
}

func TestLoadWorkflowJSONRejectsDuplicateNormalizedEmitWithinPhase(t *testing.T) {
	tests := []struct {
		name   string
		second string
	}{
		{name: "exact duplicate", second: "b.md"},
		{name: "parent alias", second: "a/../b.md"},
		{name: "dot alias", second: "./b.md"},
		{name: "portable backslash alias", second: `a\..\b.md`},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			data := `{"stage":"build","phases":[{"name":"implement","agent":"implementer",` +
				`"emits":["b.md",` + quotedJSON(tc.second) + `]}]}`
			if _, err := LoadWorkflowJSON([]byte(data)); err == nil ||
				!strings.Contains(err.Error(), `duplicates normalized target "b.md"`) {
				t.Fatalf("LoadWorkflowJSON error = %v, want normalized duplicate", err)
			}
		})
	}
}

func TestValidateWorkflowStructureAllowsCrossPhaseEmitReuse(t *testing.T) {
	wf := Workflow{Stage: "review", Phases: []Phase{
		{Name: "draft", Agent: "architect", Emits: []string{"docs/review.md"}},
		{Name: "revise", Agent: "reviewer", Emits: []string{"docs/review.md"}},
	}}
	if err := ValidateWorkflowStructure(wf); err != nil {
		t.Fatalf("cross-phase emit reuse is legal, got %v", err)
	}
}

func TestValidateWorkflowStructureRestrictsReleaseEngineerToDeliveryStages(t *testing.T) {
	phase := Phase{Name: "release-planning", Agent: "release-engineer"}
	for _, stage := range []string{"evolve", "build", "review"} {
		err := ValidateWorkflowStructure(Workflow{Stage: stage, Phases: []Phase{phase}})
		if err == nil || !strings.Contains(err.Error(), "only permitted in deploy/rollback") {
			t.Fatalf("stage %q release-engineer error = %v", stage, err)
		}
	}
	for _, stage := range []string{"deploy", "rollback"} {
		if err := ValidateWorkflowStructure(Workflow{Stage: stage, Phases: []Phase{phase}}); err != nil {
			t.Fatalf("stage %q rejected release-engineer: %v", stage, err)
		}
	}
}

func TestLoadWorkflowJSONValidatesHoistedLoopPhases(t *testing.T) {
	data := []byte(`{"stage":"evolve","loop":{"loop_back_to":"scan","phases":[
		{"name":"scan","agent":"explorer"},
		{"name":"scan","agent":"reviewer"}
	]}}`)
	if _, err := LoadWorkflowJSON(data); err == nil ||
		!strings.Contains(err.Error(), `duplicates phase name "scan"`) {
		t.Fatalf("hoisted loop phases must be validated, got %v", err)
	}
}

// quotedJSON is sufficient for the small portable-path fixtures above.
func quotedJSON(value string) string {
	return `"` + strings.ReplaceAll(value, `\`, `\\`) + `"`
}
