package domain

import (
	"errors"
	"strings"
	"testing"
)

func TestObjectiveRejectsEveryFieldProfileViolation(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Objective)
	}{
		{"title bytes", func(v *Objective) { v.Title = strings.Repeat("t", 257) }},
		{"outcome empty", func(v *Objective) { v.DesiredOutcome = "" }},
		{"outcome bytes", func(v *Objective) { v.DesiredOutcome = strings.Repeat("o", 4097) }},
		{"constraint bytes", func(v *Objective) { v.Constraints[0] = strings.Repeat("c", 1025) }},
		{"constraint control", func(v *Objective) { v.Constraints[0] = "bad\nvalue" }},
		{"project type", func(v *Objective) { v.TargetProjectIDs[0] = testID("obj", 8) }},
		{"success empty", func(v *Objective) { v.SuccessMeasures = nil }},
		{"success bytes", func(v *Objective) { v.SuccessMeasures[0] = strings.Repeat("s", 1025) }},
		{"timestamp maximum", func(v *Objective) { v.CreatedAtUnixMS = maxUnixMilliseconds + 1 }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validObjective()
			test.mutate(&value)
			if err := ValidateObjective(value); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("ValidateObjective() error = %v", err)
			}
		})
	}
}

func TestChangeRejectsEveryFieldProfileViolation(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Change)
	}{
		{"title empty", func(v *Change) { v.Title = "" }},
		{"title bytes", func(v *Change) { v.Title = strings.Repeat("c", 257) }},
		{"project type", func(v *Change) { v.SnapshotBindings[0].ProjectID = testID("obj", 7) }},
		{"snapshot type", func(v *Change) { v.SnapshotBindings[0].ProjectSnapshotID = testID("prj", 7) }},
		{"criteria empty", func(v *Change) { v.AcceptanceCriteria = nil }},
		{"description empty", func(v *Change) { v.AcceptanceCriteria[0].Description = "" }},
		{"description bytes", func(v *Change) { v.AcceptanceCriteria[0].Description = strings.Repeat("d", 2049) }},
		{"criterion verification", func(v *Change) {
			v.AcceptanceCriteria[0].VerificationRequirements = numberedTokens("verify", 17)
		}},
		{"criterion duplicate verification", func(v *Change) {
			v.AcceptanceCriteria[0].VerificationRequirements = []string{"go.test", "go.test"}
		}},
		{"timestamp", func(v *Change) { v.ProposedAtUnixMS = -1 }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validChange()
			test.mutate(&value)
			if err := ValidateChange(validObjective(), value); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("ValidateChange() error = %v", err)
			}
		})
	}
}

func TestWorkGraphRejectsEveryWorkItemProfileViolation(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*WorkGraph)
	}{
		{"graph duplicate project", func(v *WorkGraph) { v.SnapshotBindings[1].ProjectID = v.SnapshotBindings[0].ProjectID }},
		{"graph duplicate snapshot", func(v *WorkGraph) { v.SnapshotBindings[1].ProjectSnapshotID = v.SnapshotBindings[0].ProjectSnapshotID }},
		{"purpose empty", func(v *WorkGraph) { v.Items[0].Purpose = "" }},
		{"purpose bytes", func(v *WorkGraph) { v.Items[0].Purpose = strings.Repeat("p", 2049) }},
		{"dependency id", func(v *WorkGraph) { v.Items[1].Dependencies = []string{"bad"} }},
		{"criterion duplicate", func(v *WorkGraph) { v.Items[0].AcceptanceCriterionIDs = []string{"domain-valid", "domain-valid"} }},
		{"effect duplicate", func(v *WorkGraph) { v.Items[0].RequestedEffects = []string{"repo.read", "repo.read"} }},
		{"item budget", func(v *WorkGraph) { v.Items[0].Budget.MaxDurationMS = 0 }},
		{"agent control", func(v *WorkGraph) { v.Items[0].AgentRequirements = []string{"bad\nagent"} }},
		{"agent duplicate", func(v *WorkGraph) { v.Items[0].AgentRequirements = []string{"agent", "agent"} }},
		{"verification duplicate", func(v *WorkGraph) { v.Items[0].VerificationRequirements = []string{"go.test", "go.test"} }},
		{"authored timestamp", func(v *WorkGraph) { v.AuthoredAtUnixMS = -1 }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			graph := validGraph()
			test.mutate(&graph)
			if err := ValidateWorkGraph(validObjective(), validChange(), graph); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("ValidateWorkGraph() error = %v", err)
			}
		})
	}
}
