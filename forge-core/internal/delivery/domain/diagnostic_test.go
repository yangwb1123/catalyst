package domain

import (
	"errors"
	"reflect"
	"strings"
	"testing"
)

func TestCriterionCoverageDiagnosticIsDeterministic(t *testing.T) {
	objective, change, graph := validObjective(), validChange(), validGraph()
	change.AcceptanceCriteria = []AcceptanceCriterion{
		{CriterionID: "a-covered", Description: "covered but unverified",
			VerificationRequirements: []string{"security"}},
		{CriterionID: "b-uncovered", Description: "not referenced"},
	}
	for index := range graph.Items {
		graph.Items[index].AcceptanceCriterionIDs = []string{"a-covered"}
		graph.Items[index].VerificationRequirements = []string{"unit"}
	}
	want := ""
	for iteration := 0; iteration < 128; iteration++ {
		reverseValues(change.AcceptanceCriteria)
		reverseValues(graph.Items)
		err := ValidateWorkGraph(objective, change, graph)
		if !errors.Is(err, ErrInvalidDomain) {
			t.Fatalf("iteration %d error = %v", iteration, err)
		}
		if want == "" {
			want = err.Error()
		}
		if err.Error() != want {
			t.Fatalf("iteration %d diagnostic = %q, want %q", iteration, err, want)
		}
	}
	if !strings.Contains(want, "criterion verification") {
		t.Fatalf("deterministic diagnostic selected another relation: %q", want)
	}
}

func TestWorkItemDiagnosticIsIndependentOfInputOrder(t *testing.T) {
	objective, change, graph := validObjective(), validChange(), validGraph()
	graph.Items[0].Risk = "unknown"
	graph.Items[1].Budget.MaxAttempts = 0
	first := ValidateWorkGraph(objective, change, graph)
	if !errors.Is(first, ErrInvalidDomain) || !strings.Contains(first.Error(), "risk") {
		t.Fatalf("first diagnostic = %v", first)
	}
	reverseValues(graph.Items)
	second := ValidateWorkGraph(objective, change, graph)
	if !errors.Is(second, ErrInvalidDomain) || second.Error() != first.Error() {
		t.Fatalf("reordered diagnostic = %v, want %v", second, first)
	}
}

func TestInvalidWorkItemIDPrecedesSortingWithBoundedStableError(t *testing.T) {
	objective, change, graph := validObjective(), validChange(), validGraph()
	graph.Items[0].WorkItemID = strings.Repeat("x", 64*1024)
	graph.Items[1].WorkItemID = "bad"
	wantGraph := cloneWorkGraph(graph)
	wantError := "delivery domain value is invalid: WorkGraph contains an invalid WorkItem ID"
	first := ValidateWorkGraph(objective, change, graph)
	if !errors.Is(first, ErrInvalidDomain) || first.Error() != wantError {
		t.Fatalf("invalid-ID diagnostic = %v", first)
	}
	if !reflect.DeepEqual(graph, wantGraph) {
		t.Fatal("invalid-ID validation mutated its input")
	}
	reverseValues(graph.Items)
	wantGraph = cloneWorkGraph(graph)
	second := ValidateWorkGraph(objective, change, graph)
	if !errors.Is(second, ErrInvalidDomain) || second.Error() != wantError {
		t.Fatalf("reordered invalid-ID diagnostic = %v", second)
	}
	if !reflect.DeepEqual(graph, wantGraph) {
		t.Fatal("reordered invalid-ID validation mutated its input")
	}
}

func TestSourceDiagnosticEscapesAndBoundsUntrustedNames(t *testing.T) {
	diagnostic := quotedSourceDiagnostic(
		"line\nbidi\u202e" + strings.Repeat("\x01", maxSourceDiagnosticBytes),
	)
	if len(diagnostic) > maxSourceDiagnosticBytes || strings.Contains(diagnostic, "\n") ||
		strings.Contains(diagnostic, "\u202e") || !strings.Contains(diagnostic, `\n`) ||
		!strings.Contains(diagnostic, `\u202e`) || !strings.Contains(diagnostic, "truncated") {
		t.Fatalf("source diagnostic is unsafe or unbounded: %q", diagnostic)
	}
}
