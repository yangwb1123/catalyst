package domain

import (
	"errors"
	"fmt"
	"math"
	"strings"
	"testing"

	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

type domainErrorCase struct {
	name string
	call func() error
}

func TestMaximumLegalObjectiveChangeAndGraphValidate(t *testing.T) {
	objective := maximumObjective()
	change := maximumChange(objective)
	graph := maximumGraph(change)
	if err := ValidateObjective(objective); err != nil {
		t.Fatalf("maximum Objective: %v", err)
	}
	if err := ValidateChange(objective, change); err != nil {
		t.Fatalf("maximum Change: %v", err)
	}
	if err := ValidateWorkGraph(objective, change, graph); err != nil {
		t.Fatalf("maximum WorkGraph: %v", err)
	}
}

func TestEntityCollectionUpperBoundsRejectOneMore(t *testing.T) {
	objective := maximumObjective()
	change := maximumChange(objective)
	graph := maximumGraph(change)
	tests := []domainErrorCase{
		{"constraints", func() error {
			value := objective
			value.Constraints = append(value.Constraints, "one-more")
			return ValidateObjective(value)
		}},
		{"projects", func() error {
			value := objective
			value.TargetProjectIDs = append(value.TargetProjectIDs, testID("prj", 17))
			return ValidateObjective(value)
		}},
		{"success", func() error {
			value := objective
			value.SuccessMeasures = append(value.SuccessMeasures, "one-more")
			return ValidateObjective(value)
		}},
		{"criteria", func() error {
			value := change
			value.AcceptanceCriteria = append(value.AcceptanceCriteria, AcceptanceCriterion{
				CriterionID: "criterion-064", Description: "one more",
			})
			return ValidateChange(objective, value)
		}},
		{"snapshots", func() error {
			value := change
			value.SnapshotBindings = append(value.SnapshotBindings, SnapshotBinding{
				ProjectID: testID("prj", 17), ProjectSnapshotID: testID("psn", 17),
			})
			return ValidateChange(objective, value)
		}},
		{"items", func() error {
			value := graph
			extra := value.Items[len(value.Items)-1]
			extra.WorkItemID = testID("wki", maxWorkItems+1)
			value.Items = append(value.Items, extra)
			return ValidateWorkGraph(objective, change, value)
		}},
	}
	assertInvalidDomainCases(t, tests)
}

func assertInvalidDomainCases(t *testing.T, tests []domainErrorCase) {
	t.Helper()
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if err := test.call(); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("invalid domain case error = %v", err)
			}
		})
	}
}

func TestWorkItemListUpperBoundsRejectOneMore(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*WorkItem)
	}{
		{"dependencies", func(value *WorkItem) { value.Dependencies = numberedIDs("wki", maxItemListEntries+1) }},
		{"criteria", func(value *WorkItem) {
			value.AcceptanceCriterionIDs = numberedTokens("criterion", maxItemListEntries+1)
		}},
		{"effects", func(value *WorkItem) { value.RequestedEffects = numberedTokens("effect", maxItemListEntries+1) }},
		{"agents", func(value *WorkItem) { value.AgentRequirements = numberedTexts("agent", maxItemListEntries+1) }},
		{"verification", func(value *WorkItem) { value.VerificationRequirements = numberedTokens("verify", maxItemListEntries+1) }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			graph := validGraph()
			test.mutate(&graph.Items[0])
			if err := ValidateWorkGraph(validObjective(), validChange(), graph); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("one-over-maximum error = %v", err)
			}
		})
	}
}

func maximumObjective() Objective {
	value := validObjective()
	value.Title = strings.Repeat("t", 256)
	value.DesiredOutcome = strings.Repeat("o", 4096)
	value.Constraints = numberedTexts("constraint", maxConstraints)
	value.Constraints[0] = strings.Repeat("c", 1024)
	value.TargetProjectIDs = numberedIDs("prj", maxTargetProjects)
	value.SuccessMeasures = numberedTexts("measure", maxSuccessMeasures)
	value.SuccessMeasures[0] = strings.Repeat("s", 1024)
	value.CreatedAtUnixMS = maxUnixMilliseconds
	value.Version = math.MaxInt64
	return value
}

func maximumChange(objective Objective) Change {
	value := validChange()
	value.ObjectiveVersion = objective.Version
	value.Title = strings.Repeat("c", 256)
	value.SnapshotBindings = make([]SnapshotBinding, len(objective.TargetProjectIDs))
	for index, projectID := range objective.TargetProjectIDs {
		value.SnapshotBindings[index] = SnapshotBinding{
			ProjectID: projectID, ProjectSnapshotID: testID("psn", index+1),
		}
	}
	value.AcceptanceCriteria = make([]AcceptanceCriterion, maxCriteria)
	for index := range value.AcceptanceCriteria {
		value.AcceptanceCriteria[index] = AcceptanceCriterion{
			CriterionID: fmt.Sprintf("criterion-%03d", index), Description: "required condition",
		}
	}
	value.AcceptanceCriteria[0].Description = strings.Repeat("d", 2048)
	value.AcceptanceCriteria[0].VerificationRequirements = numberedTokens("verify", 16)
	value.PolicyProfile = strings.Repeat("p", 64)
	value.Budget = BudgetLimit{maxAttempts, maxDurationMS, maxCostMicroUSD}
	value.ProposedAtUnixMS = maxUnixMilliseconds
	value.Version = math.MaxInt64
	return value
}

func maximumGraph(change Change) WorkGraph {
	value := validGraph()
	value.ChangeVersion = change.Version
	value.SnapshotBindings = append([]SnapshotBinding(nil), change.SnapshotBindings...)
	bare := denseDAG(maxDependencyEdges)
	value.Items = make([]WorkItem, len(bare))
	for index := range bare {
		binding := change.SnapshotBindings[index%len(change.SnapshotBindings)]
		value.Items[index] = WorkItem{
			WorkItemID: bare[index].WorkItemID, Purpose: "bounded work",
			Dependencies: bare[index].Dependencies, ProjectID: binding.ProjectID,
			ProjectSnapshotID:      binding.ProjectSnapshotID,
			AcceptanceCriterionIDs: []string{change.AcceptanceCriteria[index%maxCriteria].CriterionID},
			Risk:                   "low", Budget: BudgetLimit{1, 1, 0},
			VerificationRequirements: []string{"go.test"}, State: pcstate.WorkItemState("planned"),
		}
	}
	maximizeFirstWorkItem(&value.Items[0], change.AcceptanceCriteria)
	value.Items[0].Budget.MaxCostMicroUSD = maxCostMicroUSD
	value.Items[len(value.Items)-1].Budget.MaxDurationMS = maxDurationMS
	value.AuthoredAtUnixMS = maxUnixMilliseconds
	value.State = "accepted"
	value.Version = math.MaxInt64
	return value
}

func maximizeFirstWorkItem(value *WorkItem, criteria []AcceptanceCriterion) {
	value.Purpose = strings.Repeat("w", 2048)
	value.AcceptanceCriterionIDs = make([]string, maxItemListEntries)
	for index := range value.AcceptanceCriterionIDs {
		value.AcceptanceCriterionIDs[index] = criteria[index].CriterionID
	}
	value.RequestedEffects = numberedTokens("effect", maxItemListEntries)
	value.AgentRequirements = numberedTexts("agent", maxItemListEntries)
	value.VerificationRequirements = numberedTokens("verify", maxItemListEntries)
	value.Risk = "critical"
}

func numberedIDs(prefix string, count int) []string {
	values := make([]string, count)
	for index := range values {
		values[index] = testID(prefix, index+1)
	}
	return values
}

func numberedTokens(prefix string, count int) []string {
	values := make([]string, count)
	for index := range values {
		values[index] = fmt.Sprintf("%s-%03d", prefix, index)
	}
	return values
}

func numberedTexts(prefix string, count int) []string {
	values := make([]string, count)
	for index := range values {
		values[index] = fmt.Sprintf("%s %03d", prefix, index)
	}
	return values
}
