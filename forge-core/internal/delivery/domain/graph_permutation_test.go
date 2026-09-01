package domain

import (
	"errors"
	"testing"

	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

type graphPermutationCase struct {
	name      string
	wantValid bool
	mutate    func(*Change, *WorkGraph)
}

func TestValidateWorkGraphIsInputOrderInvariant(t *testing.T) {
	tests := []graphPermutationCase{
		{"exact aggregate and diamond critical-path limits", true, func(*Change, *WorkGraph) {}},
		{"attempt total over limit", false, func(value *Change, _ *WorkGraph) {
			value.Budget.MaxAttempts--
		}},
		{"cost total over limit", false, func(value *Change, _ *WorkGraph) {
			value.Budget.MaxCostMicroUSD--
		}},
		{"critical path over limit", false, func(value *Change, _ *WorkGraph) {
			value.Budget.MaxDurationMS--
		}},
		{"split criterion requirement missing", false, func(_ *Change, value *WorkGraph) {
			value.Items[1].VerificationRequirements = []string{"other"}
		}},
		{"wrong-criterion verification cannot cover", false, func(_ *Change, value *WorkGraph) {
			value.Items[1].VerificationRequirements = []string{"other"}
			value.Items[2].VerificationRequirements = []string{"integration", "security"}
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			objective, change, graph := graphPermutationFixture()
			test.mutate(&change, &graph)
			assertGraphPermutations(t, objective, change, graph, test.wantValid)
		})
	}
}

func assertGraphPermutations(
	t *testing.T, objective Objective, change Change, graph WorkGraph, wantValid bool,
) {
	t.Helper()
	permutations := workItemPermutations(graph.Items)
	if len(permutations) != 24 {
		t.Fatalf("permutation count = %d", len(permutations))
	}
	for index, items := range permutations {
		candidate := graph
		candidate.Items = items
		err := ValidateWorkGraph(objective, change, candidate)
		if wantValid && err != nil {
			t.Fatalf("permutation %d: %v", index, err)
		}
		if !wantValid && !errors.Is(err, ErrInvalidDomain) {
			t.Fatalf("permutation %d invalid-domain error = %v", index, err)
		}
	}
}

func graphPermutationFixture() (Objective, Change, WorkGraph) {
	objective, change, graph := validObjective(), validChange(), validGraph()
	change.AcceptanceCriteria = []AcceptanceCriterion{
		{CriterionID: "criterion-a", Description: "unit and security pass",
			VerificationRequirements: []string{"unit", "security"}},
		{CriterionID: "criterion-b", Description: "integration passes",
			VerificationRequirements: []string{"integration"}},
	}
	change.Budget = BudgetLimit{MaxAttempts: 4, MaxDurationMS: 30, MaxCostMicroUSD: 400}
	firstID, secondID, thirdID := testID("wki", 1), testID("wki", 2), testID("wki", 3)
	items := []WorkItem{
		permutationWorkItem(1, 1, 10, "criterion-a", "unit", nil),
		permutationWorkItem(2, 1, 10, "criterion-a", "security", []string{firstID}),
		permutationWorkItem(3, 2, 5, "criterion-b", "integration", []string{firstID}),
		permutationWorkItem(4, 2, 10, "criterion-b", "integration", []string{secondID, thirdID}),
	}
	items[3].AcceptanceCriterionIDs = []string{"criterion-b", "criterion-a"}
	items[3].VerificationRequirements = []string{"integration", "unit"}
	items[3].RequestedEffects = []string{"repo.read", "repo.write"}
	items[3].AgentRequirements = []string{"Go implementation", "Security review"}
	graph.Items = items
	return objective, change, graph
}

func permutationWorkItem(
	serial, projectSerial int, duration int64,
	criterion, verification string, dependencies []string,
) WorkItem {
	return WorkItem{
		WorkItemID: testID("wki", serial), Purpose: "permutation-safe work",
		Dependencies: dependencies, ProjectID: testID("prj", projectSerial),
		ProjectSnapshotID:      testID("psn", projectSerial),
		AcceptanceCriterionIDs: []string{criterion}, Risk: "low",
		Budget:                   BudgetLimit{MaxAttempts: 1, MaxDurationMS: duration, MaxCostMicroUSD: 100},
		VerificationRequirements: []string{verification}, State: pcstate.WorkItemState("planned"),
	}
}

func TestValidateWorkGraphAcceptsAggregateDeclarationOrderings(t *testing.T) {
	assertValidSetLikePermutation(t, "objective constraints", func(value *Objective, _ *Change, _ *WorkGraph) {
		reverseValues(value.Constraints)
	})
	assertValidSetLikePermutation(t, "objective projects", func(value *Objective, _ *Change, _ *WorkGraph) {
		reverseValues(value.TargetProjectIDs)
	})
	assertValidSetLikePermutation(t, "objective measures", func(value *Objective, _ *Change, _ *WorkGraph) {
		reverseValues(value.SuccessMeasures)
	})
	assertValidSetLikePermutation(t, "change snapshots", func(_ *Objective, value *Change, _ *WorkGraph) {
		reverseValues(value.SnapshotBindings)
	})
	assertValidSetLikePermutation(t, "change criteria", func(_ *Objective, value *Change, _ *WorkGraph) {
		reverseValues(value.AcceptanceCriteria)
	})
	assertValidSetLikePermutation(t, "graph snapshots", func(_ *Objective, _ *Change, value *WorkGraph) {
		reverseValues(value.SnapshotBindings)
	})
}

func TestValidateWorkGraphAcceptsDeclarationEntryOrderings(t *testing.T) {
	assertValidSetLikePermutation(t, "criterion verification", func(_ *Objective, value *Change, _ *WorkGraph) {
		reverseValues(value.AcceptanceCriteria[0].VerificationRequirements)
	})
	assertValidSetLikePermutation(t, "dependencies", func(_ *Objective, _ *Change, value *WorkGraph) {
		reverseValues(value.Items[3].Dependencies)
	})
	assertValidSetLikePermutation(t, "criterion refs", func(_ *Objective, _ *Change, value *WorkGraph) {
		reverseValues(value.Items[3].AcceptanceCriterionIDs)
	})
	assertValidSetLikePermutation(t, "requested effects", func(_ *Objective, _ *Change, value *WorkGraph) {
		reverseValues(value.Items[3].RequestedEffects)
	})
	assertValidSetLikePermutation(t, "agent requirements", func(_ *Objective, _ *Change, value *WorkGraph) {
		reverseValues(value.Items[3].AgentRequirements)
	})
	assertValidSetLikePermutation(t, "item verification", func(_ *Objective, _ *Change, value *WorkGraph) {
		reverseValues(value.Items[3].VerificationRequirements)
	})
}

func assertValidSetLikePermutation(
	t *testing.T, name string, mutate func(*Objective, *Change, *WorkGraph),
) {
	t.Helper()
	t.Run(name, func(t *testing.T) {
		objective, change, graph := graphPermutationFixture()
		mutate(&objective, &change, &graph)
		if err := ValidateWorkGraph(objective, change, graph); err != nil {
			t.Fatal(err)
		}
	})
}

func reverseValues[T any](values []T) {
	for left, right := 0, len(values)-1; left < right; left, right = left+1, right-1 {
		values[left], values[right] = values[right], values[left]
	}
}

func workItemPermutations(items []WorkItem) [][]WorkItem {
	values := append([]WorkItem(nil), items...)
	result := make([][]WorkItem, 0, 24)
	var visit func(int)
	visit = func(index int) {
		if index == len(values) {
			result = append(result, append([]WorkItem(nil), values...))
			return
		}
		for candidate := index; candidate < len(values); candidate++ {
			values[index], values[candidate] = values[candidate], values[index]
			visit(index + 1)
			values[index], values[candidate] = values[candidate], values[index]
		}
	}
	visit(0)
	return result
}
