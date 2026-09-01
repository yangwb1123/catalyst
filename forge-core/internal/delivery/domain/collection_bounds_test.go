package domain

import (
	"testing"

	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

func TestMinimumLegalObjectiveChangeAndWorkGraphValidate(t *testing.T) {
	objective := minimumObjective()
	change := minimumChange(objective)
	graph := minimumGraph(change)
	if err := ValidateObjective(objective); err != nil {
		t.Fatalf("minimum Objective: %v", err)
	}
	if err := ValidateChange(objective, change); err != nil {
		t.Fatalf("minimum Change: %v", err)
	}
	if err := ValidateWorkGraph(objective, change, graph); err != nil {
		t.Fatalf("minimum WorkGraph: %v", err)
	}
	if drift, err := CompareSnapshotBindings(change.SnapshotBindings, nil); err != nil ||
		len(drift.MissingProjectIDs) != 1 {
		t.Fatalf("minimum bound and empty current = %+v, %v", drift, err)
	}
}

func TestRequiredCollectionsRejectBelowMinimum(t *testing.T) {
	objective := minimumObjective()
	change := minimumChange(objective)
	graph := minimumGraph(change)
	tests := []domainErrorCase{
		{"target projects", func() error { value := objective; value.TargetProjectIDs = nil; return ValidateObjective(value) }},
		{"success measures", func() error { value := objective; value.SuccessMeasures = nil; return ValidateObjective(value) }},
		{"snapshots", func() error { value := change; value.SnapshotBindings = nil; return ValidateChange(objective, value) }},
		{"criteria", func() error { value := change; value.AcceptanceCriteria = nil; return ValidateChange(objective, value) }},
		{"items", func() error { value := graph; value.Items = nil; return ValidateWorkGraph(objective, change, value) }},
		{"criterion refs", func() error {
			value := minimumGraph(change)
			value.Items[0].AcceptanceCriterionIDs = nil
			return ValidateWorkGraph(objective, change, value)
		}},
		{"verification", func() error {
			value := minimumGraph(change)
			value.Items[0].VerificationRequirements = nil
			return ValidateWorkGraph(objective, change, value)
		}},
		{"comparison bound", func() error { _, err := CompareSnapshotBindings(nil, nil); return err }},
	}
	assertInvalidDomainCases(t, tests)
}

func TestSnapshotComparisonAcceptsExactCurrentMaximum(t *testing.T) {
	current := make([]SnapshotBinding, maxComparedSnapshots)
	for index := range current {
		current[index] = SnapshotBinding{
			ProjectID: testID("prj", index+1), ProjectSnapshotID: testID("psn", index+1),
		}
	}
	drift, err := CompareSnapshotBindings(current[:1], current)
	if err != nil || len(drift.UnexpectedProjectIDs) != maxComparedSnapshots-1 {
		t.Fatalf("maximum current declarations = %+v, %v", drift, err)
	}
}

func minimumObjective() Objective {
	value := validObjective()
	value.Title, value.DesiredOutcome = "a", "a"
	value.Constraints = nil
	value.TargetProjectIDs = value.TargetProjectIDs[:1]
	value.SuccessMeasures = value.SuccessMeasures[:1]
	value.SuccessMeasures[0] = "a"
	value.CreatedAtUnixMS = 0
	return value
}

func minimumChange(objective Objective) Change {
	value := validChange()
	value.ObjectiveVersion = objective.Version
	value.Title = "a"
	value.SnapshotBindings = value.SnapshotBindings[:1]
	value.AcceptanceCriteria = []AcceptanceCriterion{{
		CriterionID: "a", Description: "a",
	}}
	value.ImpactAssessmentRef = nil
	value.PolicyProfile = "a"
	value.Budget = BudgetLimit{MaxAttempts: 1, MaxDurationMS: 1}
	value.ProposedAtUnixMS = 0
	return value
}

func minimumGraph(change Change) WorkGraph {
	value := validGraph()
	value.ChangeVersion = change.Version
	value.SnapshotBindings = value.SnapshotBindings[:1]
	value.Items = []WorkItem{{
		WorkItemID: testID("wki", 1), Purpose: "a",
		ProjectID:              change.SnapshotBindings[0].ProjectID,
		ProjectSnapshotID:      change.SnapshotBindings[0].ProjectSnapshotID,
		AcceptanceCriterionIDs: []string{"a"}, Risk: "low",
		Budget:                   BudgetLimit{MaxAttempts: 1, MaxDurationMS: 1},
		VerificationRequirements: []string{"a"}, State: pcstate.WorkItemState("planned"),
	}}
	value.AuthoredAtUnixMS = 0
	return value
}
