package domain

import (
	"errors"
	"reflect"
	"testing"
)

func TestCompareSnapshotBindingsReportsSortedDeclaredDifferences(t *testing.T) {
	bound := []SnapshotBinding{
		{ProjectID: testID("prj", 3), ProjectSnapshotID: testID("psn", 3)},
		{ProjectID: testID("prj", 1), ProjectSnapshotID: testID("psn", 1)},
		{ProjectID: testID("prj", 2), ProjectSnapshotID: testID("psn", 2)},
	}
	current := []SnapshotBinding{
		{ProjectID: testID("prj", 4), ProjectSnapshotID: testID("psn", 4)},
		{ProjectID: testID("prj", 2), ProjectSnapshotID: testID("psn", 9)},
		{ProjectID: testID("prj", 3), ProjectSnapshotID: testID("psn", 3)},
	}
	wantBound := append([]SnapshotBinding(nil), bound...)
	wantCurrent := append([]SnapshotBinding(nil), current...)
	drift, err := CompareSnapshotBindings(bound, current)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(drift.MissingProjectIDs, []string{testID("prj", 1)}) ||
		!reflect.DeepEqual(drift.ChangedProjectIDs, []string{testID("prj", 2)}) ||
		!reflect.DeepEqual(drift.UnexpectedProjectIDs, []string{testID("prj", 4)}) {
		t.Fatalf("snapshot drift = %+v", drift)
	}
	if !reflect.DeepEqual(bound, wantBound) || !reflect.DeepEqual(current, wantCurrent) {
		t.Fatal("CompareSnapshotBindings mutated its inputs")
	}
}

func TestCompareSnapshotBindingsExactMatchHasNoFreshnessClaim(t *testing.T) {
	bindings := validChange().SnapshotBindings
	drift, err := CompareSnapshotBindings(bindings, append([]SnapshotBinding(nil), bindings...))
	if err != nil || drift.MissingProjectIDs != nil || drift.ChangedProjectIDs != nil ||
		drift.UnexpectedProjectIDs != nil {
		t.Fatalf("exact supplied comparison = %+v, %v", drift, err)
	}
}

func TestCompareSnapshotBindingsAllowsEmptyCurrentDeclarations(t *testing.T) {
	bindings := validChange().SnapshotBindings
	drift, err := CompareSnapshotBindings(bindings, nil)
	if err != nil || !reflect.DeepEqual(drift.MissingProjectIDs, []string{
		testID("prj", 1), testID("prj", 2),
	}) {
		t.Fatalf("empty current declarations = %+v, %v", drift, err)
	}
}

func TestCompareSnapshotBindingsRejectsAmbiguousInputs(t *testing.T) {
	bound := validChange().SnapshotBindings
	current := append([]SnapshotBinding(nil), bound...)
	current[1].ProjectID = current[0].ProjectID
	if _, err := CompareSnapshotBindings(bound, current); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("ambiguous current bindings error = %v", err)
	}
}

func TestCompareSnapshotBindingsReportsUnexpectedBeyondTargetMaximum(t *testing.T) {
	objective := maximumObjective()
	bound := maximumChange(objective).SnapshotBindings
	current := append([]SnapshotBinding(nil), bound...)
	current = append(current, SnapshotBinding{
		ProjectID: testID("prj", 17), ProjectSnapshotID: testID("psn", 17),
	})
	drift, err := CompareSnapshotBindings(bound, current)
	if err != nil || !reflect.DeepEqual(drift.UnexpectedProjectIDs, []string{testID("prj", 17)}) {
		t.Fatalf("maximum targets plus unexpected = %+v, %v", drift, err)
	}
}

func TestCompareSnapshotBindingsBoundsCurrentDeclarations(t *testing.T) {
	current := make([]SnapshotBinding, maxComparedSnapshots+1)
	for index := range current {
		current[index] = SnapshotBinding{
			ProjectID: testID("prj", index+1), ProjectSnapshotID: testID("psn", index+1),
		}
	}
	if _, err := CompareSnapshotBindings(
		validChange().SnapshotBindings, current,
	); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("over-bound current declarations error = %v", err)
	}
}
