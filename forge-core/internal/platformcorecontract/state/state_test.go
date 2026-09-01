package state

import (
	"reflect"
	"strings"
	"testing"
)

func TestValidateWorkItemStateUsesTheFrozenVocabulary(t *testing.T) {
	for value := range workItemStates() {
		if err := ValidateWorkItemState(WorkItemState(value)); err != nil {
			t.Fatalf("state %q: %v", value, err)
		}
	}
	if err := ValidateWorkItemState("unknown"); err == nil {
		t.Fatal("unknown WorkItem state was accepted")
	}
	huge := WorkItemState(strings.Repeat("x", 1<<20))
	if err := ValidateWorkItemState(huge); err == nil || len(err.Error()) > 256 {
		t.Fatalf("oversized WorkItem state error = %v", err)
	}
}

func TestWorkItemStateValuesAreDeterministicAndDefensive(t *testing.T) {
	want := WorkItemStateValues()
	for index := 0; index < 100; index++ {
		if got := WorkItemStateValues(); !reflect.DeepEqual(got, want) {
			t.Fatalf("state values changed order: %v / %v", got, want)
		}
	}
	want[0] = "mutated"
	if WorkItemStateValues()[0] == "mutated" {
		t.Fatal("caller mutation changed the Platform state corpus")
	}
}
