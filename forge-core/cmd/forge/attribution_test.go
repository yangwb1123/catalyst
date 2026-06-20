package main

import "testing"

// taskTypeForAgent maps each LLM agent role to its scorecard task_type. The mappings are
// the ones the wind-down keys a scorecard row on; this pins the proxy table (including the
// deliberate planner->implementation fold) so a silent change to attribution is caught.
func TestTaskTypeForAgent_KnownRoles(t *testing.T) {
	want := map[string]string{
		"implementer": "implementation",
		"reviewer":    "reviewer",
		"qa":          "test",
		"planner":     "implementation", // folded into implementation, NOT requirements
		"architect":   "architecture",
	}
	for agent, tt := range want {
		got, ok := taskTypeForAgent(agent)
		if !ok {
			t.Errorf("agent %q must map to a task_type", agent)
		}
		if got != tt {
			t.Errorf("taskTypeForAgent(%q) = %q, want %q", agent, got, tt)
		}
	}
}

// HONESTY: a non-LLM/unknown role (notably a harness/gate phase) has NO mapping, so the
// lookup reports ok=false — the wind-down reads this as "skip", never attributing a
// scorecard row to a phase that did not bill a model.
func TestTaskTypeForAgent_HarnessAndUnknownNotMapped(t *testing.T) {
	for _, agent := range []string{"harness", "gate", "", "secret-scan", "unknown-role"} {
		if tt, ok := taskTypeForAgent(agent); ok || tt != "" {
			t.Errorf("agent %q must NOT map (non-LLM/unknown -> skip); got (%q,%v)", agent, tt, ok)
		}
	}
}

// planner is folded into implementation ON PURPOSE (not requirements) to avoid attributing
// planning spend to requirements' opus-floor band. This pins that decision explicitly.
func TestTaskTypeForAgent_PlannerFoldsToImplementationNotRequirements(t *testing.T) {
	tt, ok := taskTypeForAgent("planner")
	if !ok || tt != "implementation" {
		t.Errorf("planner must fold into implementation (not requirements); got (%q,%v)", tt, ok)
	}
}
