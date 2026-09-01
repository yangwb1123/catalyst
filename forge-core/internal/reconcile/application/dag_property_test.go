package application

import (
	"math/rand"
	"reflect"
	"testing"

	domain "forgeos/forge-core/internal/delivery/domain"
)

func TestDecideIsDeterministicAcrossRandomDAGPermutations(t *testing.T) {
	for seed := int64(1); seed <= 50; seed++ {
		count := 3 + int(seed%20)
		completed := int(seed % int64(count-1))
		left := generatedDAGSnapshot(seed, count, completed)
		right := generatedDAGSnapshot(seed, count, completed)
		shuffleDAGSnapshot(rand.New(rand.NewSource(seed+1000)), &right)
		leftDecision, leftErr := Decide(left)
		rightDecision, rightErr := Decide(right)
		if leftErr != nil || rightErr != nil || !reflect.DeepEqual(leftDecision, rightDecision) {
			t.Fatalf("seed %d left=%#v/%v right=%#v/%v",
				seed, leftDecision, leftErr, rightDecision, rightErr)
		}
		if leftDecision.WorkItemID != testID("wki", completed+1) {
			t.Fatalf("seed %d selected %q", seed, leftDecision.WorkItemID)
		}
	}
}

func TestDecideAcceptsExactWorkItemAndAssessmentBounds(t *testing.T) {
	value := generatedDAGSnapshot(99, maxAssessments, maxAssessments-1)
	got, err := Decide(value)
	if err != nil {
		t.Fatal(err)
	}
	if got.Kind != DecisionReadyWorkItem || got.WorkItemID != testID("wki", maxAssessments) {
		t.Fatalf("maximum bounded graph decision = %#v", got)
	}
}

func generatedDAGSnapshot(seed int64, count, completed int) ControlSnapshot {
	value := validSnapshot()
	random := rand.New(rand.NewSource(seed))
	items := make([]domain.WorkItem, count)
	for index := range items {
		items[index] = validWorkItem(index+1, value.WorkGraph.SnapshotBindings[0])
		items[index].Budget = domain.BudgetLimit{MaxAttempts: 1, MaxDurationMS: 1}
		items[index].Dependencies = randomPredecessors(random, items, index)
		if index < completed {
			items[index].State = stateCompleted
		}
	}
	value.WorkGraph.Items = items
	value.Change.Budget = domain.BudgetLimit{
		MaxAttempts: int64(count), MaxDurationMS: int64(count),
	}
	value.Assessments = make([]WorkItemAssessment, count)
	for index := range value.Assessments {
		value.Assessments[index] = validAssessment(value, index, index+1)
	}
	return value
}

func randomPredecessors(
	random *rand.Rand, items []domain.WorkItem, index int,
) []string {
	result := make([]string, 0, 3)
	for predecessor := 0; predecessor < index && len(result) < 3; predecessor++ {
		if random.Intn(4) == 0 {
			result = append(result, items[predecessor].WorkItemID)
		}
	}
	return result
}

func shuffleDAGSnapshot(random *rand.Rand, value *ControlSnapshot) {
	for index := range value.WorkGraph.Items {
		random.Shuffle(len(value.WorkGraph.Items[index].Dependencies), func(left, right int) {
			dependencies := value.WorkGraph.Items[index].Dependencies
			dependencies[left], dependencies[right] = dependencies[right], dependencies[left]
		})
	}
	random.Shuffle(len(value.WorkGraph.Items), func(left, right int) {
		value.WorkGraph.Items[left], value.WorkGraph.Items[right] =
			value.WorkGraph.Items[right], value.WorkGraph.Items[left]
	})
	random.Shuffle(len(value.Assessments), func(left, right int) {
		value.Assessments[left], value.Assessments[right] =
			value.Assessments[right], value.Assessments[left]
	})
}
