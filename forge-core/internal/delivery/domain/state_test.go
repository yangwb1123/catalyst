package domain

import (
	"errors"
	"testing"

	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

func TestDeliveryStateEdgesAcceptOnlyDeclaredTransitions(t *testing.T) {
	tests := []struct {
		name string
		call func() error
	}{
		{"objective", func() error { return ValidateObjectiveTransition("draft", "active") }},
		{"change approval", func() error { return ValidateChangeTransition("awaiting_approval", "approved") }},
		{"change pause", func() error { return ValidateChangeTransition("active", "paused") }},
		{"observation", func() error { return ValidateChangeObservationTransition("verifying", "completed") }},
		{"graph", func() error { return ValidateWorkGraphTransition("proposed", "accepted") }},
		{"work item", func() error {
			return ValidateWorkItemTransition(
				pcstate.WorkItemState("planned"), pcstate.WorkItemState("ready"),
			)
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if err := test.call(); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func TestDeliveryStateEdgesRejectUnknownReverseAndTerminalTransitions(t *testing.T) {
	tests := []struct {
		name string
		call func() error
	}{
		{"unknown", func() error { return ValidateObjectiveTransition("unknown", "active") }},
		{"objective reverse", func() error { return ValidateObjectiveTransition("active", "draft") }},
		{"change skip", func() error { return ValidateChangeTransition("proposed", "active") }},
		{"change terminal", func() error { return ValidateChangeTransition("cancelled", "active") }},
		{"observation reverse", func() error { return ValidateChangeObservationTransition("completed", "verifying") }},
		{"graph reverse", func() error { return ValidateWorkGraphTransition("accepted", "proposed") }},
		{"work item reverse", func() error {
			return ValidateWorkItemTransition(
				pcstate.WorkItemState("completed"), pcstate.WorkItemState("running"),
			)
		}},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			if err := test.call(); !errors.Is(err, ErrInvalidTransition) {
				t.Fatalf("transition error = %v", err)
			}
		})
	}
}

func TestWorkGraphAcceptsEveryPlatformWorkItemStateValue(t *testing.T) {
	for _, state := range pcstate.WorkItemStateValues() {
		t.Run(string(state), func(t *testing.T) {
			graph := validGraph()
			graph.Items[0].State = state
			if err := ValidateWorkGraph(validObjective(), validChange(), graph); err != nil {
				t.Fatalf("WorkItem state %q: %v", state, err)
			}
		})
	}
}
