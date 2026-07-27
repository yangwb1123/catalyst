package tasklist

import (
	"strings"
	"testing"
)

func TestParseOrdersDependenciesAndRenders(t *testing.T) {
	text := `Planner notes.
TASK_LIST:
- [ ] T002: add DB ping — acceptance: "go test ./... passes" — files: src/health.go — depends_on: T001 — model: sonnet — roadmap: v1 health
- [ ] T001: add health endpoint — acceptance: "curl returns 200" — files: src/api.go, src/router.go — depends_on: none — model: haiku — roadmap: v1 health`
	plan, err := Parse(text)
	if err != nil {
		t.Fatal(err)
	}
	if len(plan.Tasks) != 2 || plan.Tasks[0].ID != "T001" || plan.Tasks[1].ID != "T002" {
		t.Fatalf("ordered tasks = %+v", plan.Tasks)
	}
	rendered := Render(plan)
	if !strings.HasPrefix(rendered, "TASK_LIST:\n- [ ] T001:") {
		t.Fatalf("rendered plan not normalized in dependency order:\n%s", rendered)
	}
}

func TestParseAllowsEmptyPlan(t *testing.T) {
	plan, err := Parse("No work remains.\nTASK_LIST:\n")
	if err != nil {
		t.Fatal(err)
	}
	if len(plan.Tasks) != 0 || Render(plan) != "TASK_LIST:" {
		t.Fatalf("empty plan = %+v rendered=%q", plan, Render(plan))
	}
}

func TestParseRejectsInvalidContracts(t *testing.T) {
	valid := "- [ ] T001: task — acceptance: pass — files: src/a.go — depends_on: none — model: sonnet — roadmap: v1"
	cases := map[string]string{
		"missing marker": valid,
		"bad id":         "TASK_LIST:\n" + strings.Replace(valid, "T001", "X1", 1),
		"unsafe path":    "TASK_LIST:\n" + strings.Replace(valid, "src/a.go", "../secret", 1),
		"bad model":      "TASK_LIST:\n" + strings.Replace(valid, "sonnet", "gpt", 1),
		"unknown dep":    "TASK_LIST:\n" + strings.Replace(valid, "none", "T999", 1),
		"duplicate":      "TASK_LIST:\n" + valid + "\n" + valid,
	}
	for name, text := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := Parse(text); err == nil {
				t.Fatalf("Parse unexpectedly accepted:\n%s", text)
			}
		})
	}
}

func TestParseRejectsDependencyCycle(t *testing.T) {
	text := `TASK_LIST:
- [ ] T001: one — acceptance: pass — files: a.go — depends_on: T002 — model: haiku — roadmap: v1
- [ ] T002: two — acceptance: pass — files: b.go — depends_on: T001 — model: sonnet — roadmap: v1`
	if _, err := Parse(text); err == nil || !strings.Contains(err.Error(), "cycle") {
		t.Fatalf("cycle error = %v", err)
	}
}
