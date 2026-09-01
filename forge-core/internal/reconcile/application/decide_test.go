package application

import (
	"errors"
	"reflect"
	"testing"

	pcstate "forgeos/forge-core/internal/platformcorecontract/state"
)

func TestDecideReachesEveryPreEffectDecisionKind(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*ControlSnapshot)
		kind   DecisionKind
		reason ReasonCode
		target string
	}{
		{"ready", func(*ControlSnapshot) {}, DecisionReadyWorkItem, reasonReadyCandidate, testID("wki", 1)},
		{"no-op", func(v *ControlSnapshot) { v.Objective.State = "draft" }, DecisionNoOp, reasonObjectiveInactive, ""},
		{"await", func(v *ControlSnapshot) { v.Assessments[0].Approval = AssessmentStatusUnknown }, DecisionAwaitApproval, reasonApprovalUnknown, testID("wki", 1)},
		{"block", func(v *ControlSnapshot) { v.Assessments[0].Policy = AssessmentStatusUnsatisfiedDeclared }, DecisionBlockWorkItem, reasonPolicyUnsatisfied, testID("wki", 1)},
		{"replan", driftCurrentSnapshot, DecisionReplanChange, reasonSnapshotDrift, ""},
		{"uncertain", func(v *ControlSnapshot) { v.WorkGraph.Items[0].State = stateUncertain }, DecisionEscalateUncertain, reasonUncertainWorkItem, testID("wki", 1)},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validSnapshot()
			test.mutate(&value)
			got, err := Decide(value)
			if err != nil {
				t.Fatal(err)
			}
			assertDecision(t, value, got, test.kind, test.reason, test.target)
		})
	}
}

func driftCurrentSnapshot(value *ControlSnapshot) {
	value.CurrentSnapshotBindings[0].ProjectSnapshotID = testID("psn", 9)
}

func assertDecision(
	t *testing.T, input ControlSnapshot, got Decision,
	kind DecisionKind, reason ReasonCode, target string,
) {
	t.Helper()
	if got.Kind != kind || got.ReasonCode != reason || got.WorkItemID != target {
		t.Fatalf("decision = %#v, want kind=%s reason=%s target=%s", got, kind, reason, target)
	}
	if got.ObjectiveID != input.Objective.ObjectiveID ||
		got.ObjectiveVersion != input.Objective.Version || got.ChangeID != input.Change.ChangeID ||
		got.ChangeVersion != input.Change.Version || got.WorkGraphID != input.WorkGraph.WorkGraphID ||
		got.WorkGraphVersion != input.WorkGraph.Version {
		t.Fatalf("decision lost input identity or versions: %#v", got)
	}
}

func TestDecideStopsOnInFlightTerminalAndUnjoinedCompletion(t *testing.T) {
	tests := []struct {
		name   string
		states [2]string
		kind   DecisionKind
		reason ReasonCode
		target string
	}{
		{"in-flight", [2]string{"running", "blocked"}, DecisionNoOp, reasonWorkInFlight, testID("wki", 1)},
		{"terminal", [2]string{"completed", "failed"}, DecisionBlockWorkItem, reasonTerminalBlocker, testID("wki", 2)},
		{"all completed", [2]string{"completed", "completed"}, DecisionNoOp, reasonAllCompletedUnjoined, ""},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validSnapshot()
			value.WorkGraph.Items[0].State = stateFromString(test.states[0])
			value.WorkGraph.Items[1].State = stateFromString(test.states[1])
			got, err := Decide(value)
			if err != nil {
				t.Fatal(err)
			}
			assertDecision(t, value, got, test.kind, test.reason, test.target)
		})
	}
}

func TestDecideStopsAtDraftFrontier(t *testing.T) {
	value := validSnapshot()
	value.WorkGraph.Items[0].State = stateDraft
	got, err := Decide(value)
	if err != nil {
		t.Fatal(err)
	}
	assertDecision(
		t, value, got, DecisionNoOp, reasonWorkItemNotPlanned, testID("wki", 1),
	)
}

func TestFrontierInvariantFailsClosed(t *testing.T) {
	value := validSnapshot()
	firstID := value.WorkGraph.Items[0].WorkItemID
	secondID := value.WorkGraph.Items[1].WorkItemID
	value.WorkGraph.Items[0].Dependencies = []string{secondID}
	value.WorkGraph.Items[1].Dependencies = []string{firstID}
	got, err := frontierDecision(Decision{}, validatedSnapshot{
		order: []string{firstID, secondID}, items: indexWorkItems(value.WorkGraph.Items),
	})
	if !errors.Is(err, ErrInvalidControlSnapshot) || !reflect.DeepEqual(got, Decision{}) {
		t.Fatalf("frontier invariant returned decision=%#v error=%v", got, err)
	}
}

func TestDecideStopsOnInactiveAggregateLifecycle(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*ControlSnapshot)
		kind   DecisionKind
		reason ReasonCode
	}{
		{"change approval", func(v *ControlSnapshot) { v.Change.DesiredState = "awaiting_approval" }, DecisionAwaitApproval, reasonChangeAwaitingApproval},
		{"change paused", func(v *ControlSnapshot) { v.Change.DesiredState = "paused" }, DecisionNoOp, reasonChangeInactive},
		{"graph proposed", func(v *ControlSnapshot) { v.WorkGraph.State = "proposed" }, DecisionNoOp, reasonGraphInactive},
		{"observation blocked", func(v *ControlSnapshot) { v.Change.ObservedState = "blocked" }, DecisionNoOp, reasonObservationInactive},
		{"change uncertain", func(v *ControlSnapshot) { v.Change.ObservedState = "uncertain" }, DecisionEscalateUncertain, reasonUncertainChange},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validSnapshot()
			test.mutate(&value)
			got, err := Decide(value)
			if err != nil || got.Kind != test.kind || got.ReasonCode != test.reason {
				t.Fatalf("decision = %#v, error = %v", got, err)
			}
		})
	}
}

func stateFromString(value string) pcstate.WorkItemState {
	return pcstate.WorkItemState(value)
}
