package attribution

import "testing"

func TestTaskTypeForAgent_KnownRoles(t *testing.T) {
	want := map[string]string{
		"implementer": "implementation",
		"reviewer":    "reviewer",
		"qa":          "test",
		"planner":     "implementation",
		"architect":   "architecture",
	}
	for agent, tt := range want {
		got, ok := TaskTypeForAgent(agent)
		if !ok {
			t.Errorf("agent %q must map to a task_type", agent)
		}
		if got != tt {
			t.Errorf("TaskTypeForAgent(%q) = %q, want %q", agent, got, tt)
		}
	}
}

func TestTaskTypeForAgent_HarnessAndUnknownNotMapped(t *testing.T) {
	for _, agent := range []string{"harness", "gate", "", "secret-scan", "unknown-role"} {
		if tt, ok := TaskTypeForAgent(agent); ok || tt != "" {
			t.Errorf("agent %q must NOT map; got (%q,%v)", agent, tt, ok)
		}
	}
}

func TestTaskTypeForAgent_PlannerFoldsToImplementationNotRequirements(t *testing.T) {
	tt, ok := TaskTypeForAgent("planner")
	if !ok || tt != "implementation" {
		t.Errorf("planner must fold into implementation; got (%q,%v)", tt, ok)
	}
}
