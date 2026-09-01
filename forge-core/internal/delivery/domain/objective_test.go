package domain

import (
	"errors"
	"reflect"
	"strings"
	"testing"
)

func TestValidateObjectiveAcceptsBoundedValueWithoutMutation(t *testing.T) {
	value := validObjective()
	want := validObjective()
	if err := ValidateObjective(value); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(value, want) {
		t.Fatal("ValidateObjective mutated its input")
	}
}

func TestValidateObjectiveRejectsSemanticMutations(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*Objective)
	}{
		{"objective id", func(v *Objective) { v.ObjectiveID = testID("chg", 1) }},
		{"space id", func(v *Objective) { v.SpaceID = "bad" }},
		{"empty title", func(v *Objective) { v.Title = "" }},
		{"trim", func(v *Objective) { v.DesiredOutcome += " " }},
		{"bidi", func(v *Objective) { v.Title += "\u202e" }},
		{"constraint duplicate", func(v *Objective) { v.Constraints[1] = v.Constraints[0] }},
		{"project duplicate", func(v *Objective) { v.TargetProjectIDs[1] = v.TargetProjectIDs[0] }},
		{"success duplicate", func(v *Objective) { v.SuccessMeasures[1] = v.SuccessMeasures[0] }},
		{"actor", func(v *Objective) { v.CreatedBy.ActorID = "bad" }},
		{"timestamp", func(v *Objective) { v.CreatedAtUnixMS = -1 }},
		{"state", func(v *Objective) { v.State = "unknown" }},
		{"version", func(v *Objective) { v.Version = 0 }},
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

func TestValidateObjectiveEnforcesCollectionBounds(t *testing.T) {
	value := validObjective()
	value.TargetProjectIDs = nil
	if err := ValidateObjective(value); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("empty projects error = %v", err)
	}
	value = validObjective()
	value.Constraints = make([]string, maxConstraints+1)
	for index := range value.Constraints {
		value.Constraints[index] = strings.Repeat("x", index+1)
	}
	if err := ValidateObjective(value); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("constraint bound error = %v", err)
	}
}
