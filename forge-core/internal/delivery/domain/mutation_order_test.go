package domain

import (
	"reflect"
	"testing"
)

func TestValidateObjectiveDoesNotMutateNoncanonicalOrder(t *testing.T) {
	value := validObjective()
	reverseValues(value.Constraints)
	reverseValues(value.TargetProjectIDs)
	reverseValues(value.SuccessMeasures)
	want := cloneObjective(value)
	if err := ValidateObjective(value); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(value, want) {
		t.Fatal("ValidateObjective mutated noncanonical input order")
	}
}

func TestValidateChangeDoesNotMutateNoncanonicalOrder(t *testing.T) {
	objective, value := validObjective(), validChange()
	reverseValues(objective.TargetProjectIDs)
	reverseValues(value.SnapshotBindings)
	reverseValues(value.AcceptanceCriteria)
	value.AcceptanceCriteria[0].VerificationRequirements = []string{"security.scan", "go.test"}
	wantObjective, wantChange := cloneObjective(objective), cloneChange(value)
	if err := ValidateChange(objective, value); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(objective, wantObjective) || !reflect.DeepEqual(value, wantChange) {
		t.Fatal("ValidateChange mutated a noncanonical input graph")
	}
}

func TestValidateWorkGraphDoesNotMutateNoncanonicalReachableGraph(t *testing.T) {
	objective, change, graph := graphPermutationFixture()
	reverseEveryAggregateDeclaration(&objective, &change, &graph)
	wantObjective := cloneObjective(objective)
	wantChange, wantGraph := cloneChange(change), cloneWorkGraph(graph)
	if err := ValidateWorkGraph(objective, change, graph); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(objective, wantObjective) || !reflect.DeepEqual(change, wantChange) ||
		!reflect.DeepEqual(graph, wantGraph) {
		t.Fatal("ValidateWorkGraph mutated a noncanonical reachable graph")
	}
}

func reverseEveryAggregateDeclaration(
	objective *Objective, change *Change, graph *WorkGraph,
) {
	reverseValues(objective.Constraints)
	reverseValues(objective.TargetProjectIDs)
	reverseValues(objective.SuccessMeasures)
	reverseValues(change.SnapshotBindings)
	reverseValues(change.AcceptanceCriteria)
	for index := range change.AcceptanceCriteria {
		reverseValues(change.AcceptanceCriteria[index].VerificationRequirements)
	}
	reverseValues(graph.SnapshotBindings)
	reverseValues(graph.Items)
	for index := range graph.Items {
		reverseWorkItemDeclarations(&graph.Items[index])
	}
}

func reverseWorkItemDeclarations(value *WorkItem) {
	reverseValues(value.Dependencies)
	reverseValues(value.AcceptanceCriterionIDs)
	reverseValues(value.RequestedEffects)
	reverseValues(value.AgentRequirements)
	reverseValues(value.VerificationRequirements)
}
