package application

import "testing"

func TestDecideUsesTheFrozenSafetyPrecedence(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*ControlSnapshot)
		kind   DecisionKind
		reason ReasonCode
	}{
		{"uncertainty before drift", uncertainAndDrift, DecisionEscalateUncertain, reasonUncertainAssessment},
		{"drift before lifecycle", driftAndPause, DecisionReplanChange, reasonSnapshotDrift},
		{"in-flight before terminal", inFlightAndTerminal, DecisionNoOp, reasonWorkInFlight},
		{"policy before approval", denyPolicyAndUnknownApproval, DecisionBlockWorkItem, reasonPolicyUnsatisfied},
		{"approval before budget", unknownApprovalAndNoBudget, DecisionAwaitApproval, reasonApprovalUnknown},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validSnapshot()
			test.mutate(&value)
			got, err := Decide(value)
			if err != nil {
				t.Fatal(err)
			}
			if got.Kind != test.kind || got.ReasonCode != test.reason {
				t.Fatalf("decision = %#v, want %s/%s", got, test.kind, test.reason)
			}
		})
	}
}

func uncertainAndDrift(value *ControlSnapshot) {
	value.Assessments[0].Budget = AssessmentStatusUncertain
	driftCurrentSnapshot(value)
}

func driftAndPause(value *ControlSnapshot) {
	driftCurrentSnapshot(value)
	value.Change.DesiredState = "paused"
}

func inFlightAndTerminal(value *ControlSnapshot) {
	value.WorkGraph.Items[0].State = stateRunning
	value.WorkGraph.Items[1].State = stateBlocked
}

func denyPolicyAndUnknownApproval(value *ControlSnapshot) {
	value.Assessments[0].Policy = AssessmentStatusUnsatisfiedDeclared
	value.Assessments[0].Approval = AssessmentStatusUnknown
}

func unknownApprovalAndNoBudget(value *ControlSnapshot) {
	value.Assessments[0].Approval = AssessmentStatusUnknown
	value.Assessments[0].Budget = AssessmentStatusUnsatisfiedDeclared
}

func TestDecideNeverSkipsAnEarlierCandidateForItsAssessment(t *testing.T) {
	value := validSnapshot()
	value.WorkGraph.Items[1].Dependencies = nil
	second := validAssessment(value, 1, 2)
	value.Assessments[0].Approval = AssessmentStatusUnknown
	value.Assessments = append(value.Assessments, second)
	got, err := Decide(value)
	if err != nil {
		t.Fatal(err)
	}
	if got.Kind != DecisionAwaitApproval || got.WorkItemID != testID("wki", 1) {
		t.Fatalf("later allowed candidate bypassed the earlier pending candidate: %#v", got)
	}
}

func TestDecideClassifiesEveryDeclaredPreconditionStop(t *testing.T) {
	tests := []struct {
		name   string
		mutate func(*WorkItemAssessment)
		kind   DecisionKind
		reason ReasonCode
	}{
		{"policy unknown", func(v *WorkItemAssessment) { v.Policy = AssessmentStatusUnknown }, DecisionBlockWorkItem, reasonPolicyUnknown},
		{"policy denied", func(v *WorkItemAssessment) { v.Policy = AssessmentStatusUnsatisfiedDeclared }, DecisionBlockWorkItem, reasonPolicyUnsatisfied},
		{"approval unknown", func(v *WorkItemAssessment) { v.Approval = AssessmentStatusUnknown }, DecisionAwaitApproval, reasonApprovalUnknown},
		{"approval denied", func(v *WorkItemAssessment) { v.Approval = AssessmentStatusUnsatisfiedDeclared }, DecisionBlockWorkItem, reasonApprovalUnsatisfied},
		{"budget unknown", func(v *WorkItemAssessment) { v.Budget = AssessmentStatusUnknown }, DecisionBlockWorkItem, reasonBudgetUnknown},
		{"budget exhausted", func(v *WorkItemAssessment) { v.Budget = AssessmentStatusUnsatisfiedDeclared }, DecisionBlockWorkItem, reasonBudgetUnsatisfied},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			value := validSnapshot()
			test.mutate(&value.Assessments[0])
			got, err := Decide(value)
			if err != nil || got.Kind != test.kind || got.ReasonCode != test.reason {
				t.Fatalf("decision = %#v, error = %v", got, err)
			}
		})
	}
}
