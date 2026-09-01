package domain

import (
	"errors"
	"testing"

	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

type stateMachineCase struct {
	name     string
	states   []string
	edges    map[string][]string
	validate func(string, string) error
}

func TestEveryDeliveryStatePairMatchesTheDeclaredMatrix(t *testing.T) {
	for _, machine := range deliveryStateMachineCases() {
		t.Run(machine.name, func(t *testing.T) {
			expected := edgeSet(machine.edges)
			states := append(append([]string(nil), machine.states...), "unknown")
			for _, from := range states {
				for _, to := range states {
					err := machine.validate(from, to)
					if expected[from+"\x00"+to] && err != nil {
						t.Fatalf("declared edge %s -> %s: %v", from, to, err)
					}
					if !expected[from+"\x00"+to] && !errors.Is(err, ErrInvalidTransition) {
						t.Fatalf("undeclared edge %s -> %s error = %v", from, to, err)
					}
				}
			}
		})
	}
}

func deliveryStateMachineCases() []stateMachineCase {
	return []stateMachineCase{
		objectiveStateMachine(), changeStateMachine(), observationStateMachine(),
		graphStateMachine(),
	}
}

func objectiveStateMachine() stateMachineCase {
	return stateMachineCase{
		name: "objective", states: []string{"draft", "active", "satisfied", "cancelled"},
		edges: map[string][]string{"draft": {"active", "cancelled"}, "active": {"satisfied", "cancelled"}},
		validate: func(from, to string) error {
			return ValidateObjectiveTransition(ObjectiveState(from), ObjectiveState(to))
		},
	}
}

func changeStateMachine() stateMachineCase {
	return stateMachineCase{
		name:   "change",
		states: []string{"proposed", "awaiting_approval", "approved", "active", "paused", "cancelled"},
		edges: map[string][]string{
			"proposed": {"awaiting_approval", "cancelled"}, "awaiting_approval": {"approved", "cancelled"},
			"approved": {"active", "cancelled"}, "active": {"paused", "cancelled"},
			"paused": {"active", "cancelled"},
		},
		validate: func(from, to string) error {
			return ValidateChangeTransition(ChangeState(from), ChangeState(to))
		},
	}
}

func observationStateMachine() stateMachineCase {
	return stateMachineCase{
		name:   "change observation",
		states: []string{"not_started", "in_progress", "blocked", "verifying", "completed", "failed", "uncertain", "cancelled"},
		edges: map[string][]string{
			"not_started": {"in_progress", "blocked", "cancelled"},
			"in_progress": {"blocked", "verifying", "failed", "uncertain", "cancelled"},
			"blocked":     {"in_progress", "failed", "uncertain", "cancelled"},
			"verifying":   {"in_progress", "blocked", "completed", "failed", "uncertain", "cancelled"},
			"uncertain":   {"blocked", "failed", "cancelled"},
		},
		validate: func(from, to string) error {
			return ValidateChangeObservationTransition(ChangeObservation(from), ChangeObservation(to))
		},
	}
}

func graphStateMachine() stateMachineCase {
	return stateMachineCase{
		name: "work graph", states: []string{"draft", "proposed", "accepted", "superseded"},
		edges: map[string][]string{
			"draft": {"proposed"}, "proposed": {"accepted", "superseded"}, "accepted": {"superseded"},
		},
		validate: func(from, to string) error {
			return ValidateWorkGraphTransition(WorkGraphState(from), WorkGraphState(to))
		},
	}
}

func TestWorkItemTransitionMatchesThePlatformCoreMatrix(t *testing.T) {
	states := pcstate.WorkItemStateValues()
	states = append(states, "unknown")
	for _, from := range states {
		for _, to := range states {
			upstream := pcstate.ValidateWorkItemTransition(from, to)
			delivery := ValidateWorkItemTransition(from, to)
			if (upstream == nil) != (delivery == nil) {
				t.Fatalf("WorkItem edge %s -> %s differs: %v / %v", from, to, upstream, delivery)
			}
			if delivery != nil && !errors.Is(delivery, ErrInvalidTransition) {
				t.Fatalf("WorkItem edge %s -> %s relation = %v", from, to, delivery)
			}
		}
	}
}
