package domain

import (
	"errors"
	"reflect"
	"testing"
)

func TestValidateWorkGraphAcceptsExactPlanWithoutMutation(t *testing.T) {
	graph := validGraph()
	want := validGraph()
	if err := ValidateWorkGraph(validObjective(), validChange(), graph); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(graph, want) {
		t.Fatal("ValidateWorkGraph mutated its input")
	}
	order, err := TopologicalOrder(graph.Items)
	if err != nil || !reflect.DeepEqual(order, []string{testID("wki", 1), testID("wki", 2)}) {
		t.Fatalf("topological order = %v, %v", order, err)
	}
}

func TestValidateWorkGraphRejectsGraphMutations(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*WorkGraph)
	}{
		{"graph id", func(v *WorkGraph) { v.WorkGraphID = testID("wki", 1) }},
		{"change", func(v *WorkGraph) { v.ChangeID = testID("chg", 2) }},
		{"change version", func(v *WorkGraph) { v.ChangeVersion++ }},
		{"objective", func(v *WorkGraph) { v.ObjectiveID = testID("obj", 2) }},
		{"space", func(v *WorkGraph) { v.SpaceID = testID("spc", 2) }},
		{"snapshot", func(v *WorkGraph) { v.SnapshotBindings[0].ProjectSnapshotID = testID("psn", 9) }},
		{"snapshot project", func(v *WorkGraph) { v.SnapshotBindings[0].ProjectID = testID("prj", 9) }},
		{"no items", func(v *WorkGraph) { v.Items = nil }},
		{"duplicate item", func(v *WorkGraph) { v.Items[1].WorkItemID = v.Items[0].WorkItemID }},
		{"missing dependency", func(v *WorkGraph) { v.Items[1].Dependencies[0] = testID("wki", 9) }},
		{"duplicate dependency", func(v *WorkGraph) {
			v.Items[1].Dependencies = append(v.Items[1].Dependencies, v.Items[0].WorkItemID)
		}},
		{"self dependency", func(v *WorkGraph) { v.Items[1].Dependencies[0] = v.Items[1].WorkItemID }},
		{"cycle", func(v *WorkGraph) { v.Items[0].Dependencies = []string{v.Items[1].WorkItemID} }},
		{"uncovered criterion", func(v *WorkGraph) { v.Items[1].AcceptanceCriterionIDs = []string{"domain-valid"} }},
		{"unknown criterion", func(v *WorkGraph) { v.Items[1].AcceptanceCriterionIDs = []string{"unknown"} }},
		{"wrong target", func(v *WorkGraph) { v.Items[0].ProjectSnapshotID = testID("psn", 2) }},
		{"empty target", func(v *WorkGraph) {
			v.Items[0].ProjectID = ""
			v.Items[0].ProjectSnapshotID = ""
		}},
		{"effect", func(v *WorkGraph) { v.Items[0].RequestedEffects = []string{"repo write"} }},
		{"risk", func(v *WorkGraph) { v.Items[0].Risk = "unknown" }},
		{"verification", func(v *WorkGraph) { v.Items[0].VerificationRequirements = nil }},
		{"state", func(v *WorkGraph) { v.Items[0].State = "unknown" }},
		{"author", func(v *WorkGraph) { v.AuthoredBy.ActorID = "bad" }},
		{"graph state", func(v *WorkGraph) { v.State = "running" }},
		{"version", func(v *WorkGraph) { v.Version = 0 }},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			graph := validGraph()
			test.mutate(&graph)
			if err := ValidateWorkGraph(
				validObjective(), validChange(), graph,
			); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("ValidateWorkGraph() error = %v", err)
			}
		})
	}
}

func TestValidateWorkGraphEnforcesBudgetsAndArtifactSnapshot(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*WorkGraph)
	}{
		{"attempt sum", func(v *WorkGraph) {
			v.Items[0].Budget.MaxAttempts = 3
			v.Items[1].Budget.MaxAttempts = 3
		}},
		{"cost sum", func(v *WorkGraph) {
			v.Items[0].Budget.MaxCostMicroUSD = 600
			v.Items[1].Budget.MaxCostMicroUSD = 600
		}},
		{"duration", func(v *WorkGraph) { v.Items[0].Budget.MaxDurationMS = 60001 }},
		{"artifact snapshot", func(v *WorkGraph) {
			v.Items[0].ContextArtifactRef.SourceSnapshotRef.EntityID = testID("psn", 2)
		}},
		{"artifact invalid", func(v *WorkGraph) {
			v.Items[0].ContextArtifactRef.ContentID = "sha256:wrong"
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			graph := validGraph()
			test.mutate(&graph)
			if err := ValidateWorkGraph(
				validObjective(), validChange(), graph,
			); !errors.Is(err, ErrInvalidDomain) {
				t.Fatalf("ValidateWorkGraph() error = %v", err)
			}
		})
	}
}

func TestValidateWorkGraphEnforcesCriticalPathDuration(t *testing.T) {
	graph := validGraph()
	graph.Items[0].Budget.MaxDurationMS = 30001
	graph.Items[1].Budget.MaxDurationMS = 30001
	if err := ValidateWorkGraph(
		validObjective(), validChange(), graph,
	); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("over-budget critical path error = %v", err)
	}
	graph = validGraph()
	graph.Items[0].Budget.MaxDurationMS = 60000
	graph.Items[1].Budget.MaxDurationMS = 60000
	graph.Items[1].Dependencies = nil
	if err := ValidateWorkGraph(validObjective(), validChange(), graph); err != nil {
		t.Fatalf("parallel duration ceilings: %v", err)
	}
}

func TestValidateWorkGraphCoversCriterionVerificationRequirements(t *testing.T) {
	change := validChange()
	change.AcceptanceCriteria[0].VerificationRequirements = []string{"go.test", "security.scan"}
	graph := validGraph()
	if err := ValidateWorkGraph(
		validObjective(), change, graph,
	); !errors.Is(err, ErrInvalidDomain) {
		t.Fatalf("missing criterion verification error = %v", err)
	}
	graph.Items[0].VerificationRequirements = append(
		graph.Items[0].VerificationRequirements, "security.scan",
	)
	if err := ValidateWorkGraph(validObjective(), change, graph); err != nil {
		t.Fatalf("covered criterion verification: %v", err)
	}
}
