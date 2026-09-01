package domain

import (
	"errors"
	"reflect"
	"testing"
)

func TestValidateChangeAcceptsExactObjectiveCoverage(t *testing.T) {
	value := validChange()
	want := validChange()
	if err := ValidateChange(validObjective(), value); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(value, want) {
		t.Fatal("ValidateChange mutated its input")
	}
}

func TestValidateChangeRejectsSemanticMutations(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Change)
	}{
		{"change id", func(v *Change) { v.ChangeID = testID("obj", 1) }},
		{"objective", func(v *Change) { v.ObjectiveID = testID("obj", 2) }},
		{"objective version", func(v *Change) { v.ObjectiveVersion++ }},
		{"space", func(v *Change) { v.SpaceID = testID("spc", 2) }},
		{"snapshot coverage", func(v *Change) { v.SnapshotBindings = v.SnapshotBindings[:1] }},
		{"snapshot project replacement", func(v *Change) {
			v.SnapshotBindings[1].ProjectID = testID("prj", 9)
		}},
		{"duplicate project", func(v *Change) { v.SnapshotBindings[1].ProjectID = v.SnapshotBindings[0].ProjectID }},
		{"duplicate snapshot", func(v *Change) { v.SnapshotBindings[1].ProjectSnapshotID = v.SnapshotBindings[0].ProjectSnapshotID }},
		{"criterion id", func(v *Change) { v.AcceptanceCriteria[0].CriterionID = "Bad" }},
		{"criterion duplicate", func(v *Change) { v.AcceptanceCriteria[1].CriterionID = v.AcceptanceCriteria[0].CriterionID }},
		{"impact", func(v *Change) { v.ImpactAssessmentRef.RecordSHA256 = "bad" }},
		{"impact type", func(v *Change) { v.ImpactAssessmentRef.RecordType = "forge.runtime.artifact_provenance" }},
		{"policy", func(v *Change) { v.PolicyProfile = "bad profile" }},
		{"budget", func(v *Change) { v.Budget.MaxAttempts = 0 }},
		{"actor", func(v *Change) { v.ProposedBy.ActorType = "unknown" }},
		{"desired", func(v *Change) { v.DesiredState = "completed" }},
		{"observed", func(v *Change) { v.ObservedState = "approved" }},
		{"version", func(v *Change) { v.Version = 0 }},
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

func TestValidateChangeRejectsInvalidObjective(t *testing.T) {
	objective := validObjective()
	objective.State = "unknown"
	if err := ValidateChange(objective, validChange()); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("invalid Objective error = %v", err)
	}
}
