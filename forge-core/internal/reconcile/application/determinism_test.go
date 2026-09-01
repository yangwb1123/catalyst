package application

import (
	"reflect"
	"sync"
	"testing"

	domain "forgeos/forge-core/internal/delivery/domain"
	core "forgeos/forge-core/internal/platformcorecontract"
)

func TestDecideIsInvariantToSetLikeInputPermutations(t *testing.T) {
	left := multiFrontierSnapshot()
	right := multiFrontierSnapshot()
	reverseWorkItems(right.WorkGraph.Items)
	reverseAssessments(right.Assessments)
	reverseBindings(right.CurrentSnapshotBindings)
	reverseBindings(right.Change.SnapshotBindings)
	reverseBindings(right.WorkGraph.SnapshotBindings)
	reverseStrings(right.Objective.TargetProjectIDs)
	reverseStrings(right.WorkGraph.Items[0].RequestedEffects)
	reverseStrings(right.Assessments[0].RequestedEffects)
	reverseEvidence(&right.Assessments[1])
	leftDecision, leftErr := Decide(left)
	rightDecision, rightErr := Decide(right)
	if leftErr != nil || rightErr != nil || !reflect.DeepEqual(leftDecision, rightDecision) {
		t.Fatalf("permuted decision left=%#v/%v right=%#v/%v",
			leftDecision, leftErr, rightDecision, rightErr)
	}
	if leftDecision.WorkItemID != testID("wki", 1) {
		t.Fatalf("lexical frontier target = %q", leftDecision.WorkItemID)
	}
}

func multiFrontierSnapshot() ControlSnapshot {
	value := validSnapshot()
	secondBinding := domain.SnapshotBinding{
		ProjectID: testID("prj", 2), ProjectSnapshotID: testID("psn", 2),
	}
	value.Objective.TargetProjectIDs = append(value.Objective.TargetProjectIDs, secondBinding.ProjectID)
	value.Change.SnapshotBindings = append(value.Change.SnapshotBindings, secondBinding)
	value.WorkGraph.SnapshotBindings = append(value.WorkGraph.SnapshotBindings, secondBinding)
	value.CurrentSnapshotBindings = append(value.CurrentSnapshotBindings, secondBinding)
	value.WorkGraph.Items[1].Dependencies = nil
	value.WorkGraph.Items[1].RequestedEffects = []string{"test.run", "repo.write"}
	value.Assessments = append(value.Assessments, validAssessment(value, 1, 2))
	value.Assessments[0].EvidenceRefs = []core.RecordRef{testEvidence(2), testEvidence(1)}
	return value
}

func reverseWorkItems(values []domain.WorkItem) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}

func reverseAssessments(values []WorkItemAssessment) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}

func reverseBindings(values []domain.SnapshotBinding) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}

func reverseStrings(values []string) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}

func reverseEvidence(value *WorkItemAssessment) {
	for left, right := 0, len(value.EvidenceRefs)-1; left < right; left, right = left+1, right-1 {
		value.EvidenceRefs[left], value.EvidenceRefs[right] =
			value.EvidenceRefs[right], value.EvidenceRefs[left]
	}
}

func TestDecideIsRepeatableAndSafeForStableParallelCallers(t *testing.T) {
	value := validSnapshot()
	want, err := Decide(value)
	if err != nil {
		t.Fatal(err)
	}
	for iteration := 0; iteration < 1000; iteration++ {
		got, gotErr := Decide(value)
		if gotErr != nil || !reflect.DeepEqual(got, want) {
			t.Fatalf("iteration %d = %#v, %v", iteration, got, gotErr)
		}
	}
	assertParallelDecisions(t, value, want)
}

func assertParallelDecisions(t *testing.T, value ControlSnapshot, want Decision) {
	t.Helper()
	errors := make(chan string, 32)
	var group sync.WaitGroup
	for worker := 0; worker < cap(errors); worker++ {
		group.Add(1)
		go func() {
			defer group.Done()
			got, err := Decide(value)
			if err != nil || !reflect.DeepEqual(got, want) {
				errors <- "parallel decision differed"
			}
		}()
	}
	group.Wait()
	close(errors)
	if message := <-errors; message != "" {
		t.Fatal(message)
	}
}

func TestDecideDoesNotMutateOrAliasCallerStorage(t *testing.T) {
	value, wantInput := multiFrontierSnapshot(), multiFrontierSnapshot()
	decision, err := Decide(value)
	if err != nil || !reflect.DeepEqual(value, wantInput) {
		t.Fatalf("input changed: error=%v", err)
	}
	decision.EvidenceRefs[0].RecordID = "mutated:local/1"
	if value.Assessments[0].EvidenceRefs[0].RecordID == "mutated:local/1" {
		t.Fatal("Decision evidence aliases caller storage")
	}
}
