package graphscheduledreconcile

import (
	"bytes"
	"strings"
	"testing"
)

func TestReconcileClassifiesEveryDisposition(t *testing.T) {
	for _, test := range reconcileCases() {
		t.Run(test.name, func(t *testing.T) {
			value := validUnsignedSnapshot()
			test.arrange(&value)
			decision, err := Reconcile(decodeSignedSnapshot(t, value))
			if err != nil || decision.Disposition != test.want {
				t.Fatalf("Reconcile disposition = %q, %v; want %q", decision.Disposition, err, test.want)
			}
			assertNextFields(t, decision, test.nextOrdinal, test.nextNodeID)
			encoded, err := MarshalDecision(decision)
			if err != nil || len(encoded) == 0 || encoded[len(encoded)-1] == '\n' {
				t.Fatalf("MarshalDecision = %q, %v", encoded, err)
			}
		})
	}
}

type reconcileCase struct {
	name        string
	want        string
	nextOrdinal *uint16
	nextNodeID  *string
	arrange     func(*ProgressSnapshot)
}

func reconcileCases() []reconcileCase {
	zero, one := uint16(0), uint16(1)
	build, verify := "build", "verify"
	return []reconcileCase{
		{"empty ready", "ready", &zero, &build, func(*ProgressSnapshot) {}},
		{"prepared ready", "ready", &zero, &build, arrangePrepared},
		{"successor ready", "ready", &one, &verify, arrangeCompletedPrefix},
		{"claimed", "claimed_unknown", nil, nil, arrangeStatus("claimed", "")},
		{"quarantined", "manual_recovery_required", nil, nil, arrangeStatus("quarantined", "")},
		{"adjudicated", "manual_recovery_required", nil, nil, arrangeStatus("adjudicated", "")},
		{"failed", "failed", nil, nil, arrangeStatus("terminalized", "failed")},
		{"failed uncertain", "failed_uncertain", nil, nil, arrangeStatus("terminalized", "failed_uncertain")},
		{"completed", "completed", nil, nil, arrangeAllCompleted},
		{"future candidate", "incompatible_progress", nil, nil, arrangeFutureCandidate},
		{"completed after failure", "incompatible_progress", nil, nil, arrangeCompletedAfterFailure},
	}
}

func arrangePrepared(value *ProgressSnapshot) {
	materialize(&value.Nodes[0], "b", "c")
}

func arrangeCompletedPrefix(value *ProgressSnapshot) {
	setLifecycle(&value.Nodes[0], "terminalized", "completed")
}

func arrangeStatus(status, outcome string) func(*ProgressSnapshot) {
	return func(value *ProgressSnapshot) { setLifecycle(&value.Nodes[0], status, outcome) }
}

func arrangeAllCompleted(value *ProgressSnapshot) {
	setLifecycle(&value.Nodes[0], "terminalized", "completed")
	materialize(&value.Nodes[1], "e", "f")
	value.Nodes[1].LifecycleStatus = stringPointer("terminalized")
	value.Nodes[1].TerminalOutcome = stringPointer("completed")
	value.Nodes[1].TerminalReceiptSHA256 = stringPointer(strings.Repeat("1", 64))
}

func arrangeFutureCandidate(value *ProgressSnapshot) {
	materialize(&value.Nodes[1], "e", "f")
}

func arrangeCompletedAfterFailure(value *ProgressSnapshot) {
	setLifecycle(&value.Nodes[0], "terminalized", "failed")
	materialize(&value.Nodes[1], "e", "f")
	value.Nodes[1].LifecycleStatus = stringPointer("terminalized")
	value.Nodes[1].TerminalOutcome = stringPointer("completed")
	value.Nodes[1].TerminalReceiptSHA256 = stringPointer(strings.Repeat("1", 64))
}

func assertNextFields(t *testing.T, decision Decision, ordinal *uint16, nodeID *string) {
	t.Helper()
	if !equalUint16Pointer(decision.NextExecutionOrdinal, ordinal) ||
		!equalStringPointer(decision.NextNodeID, nodeID) {
		t.Fatalf("next fields = %v/%v; want %v/%v", decision.NextExecutionOrdinal, decision.NextNodeID, ordinal, nodeID)
	}
}

func equalUint16Pointer(left, right *uint16) bool {
	return left == nil && right == nil || left != nil && right != nil && *left == *right
}

func equalStringPointer(left, right *string) bool {
	return left == nil && right == nil || left != nil && right != nil && *left == *right
}

func TestMarshalDecisionRejectsMutation(t *testing.T) {
	decision, err := Reconcile(decodeSignedSnapshot(t, validUnsignedSnapshot()))
	if err != nil {
		t.Fatalf("Reconcile: %v", err)
	}
	decision.Disposition = "completed"
	decision.NextExecutionOrdinal, decision.NextNodeID = nil, nil
	if _, err := MarshalDecision(decision); err == nil {
		t.Fatal("MarshalDecision accepted stale digest")
	}
	decision.DecisionSHA256 = strings.Repeat("f", 64)
	if _, err := MarshalDecision(decision); err == nil {
		t.Fatal("MarshalDecision accepted forged digest")
	}
}

func TestReconcileRejectsMutatedSnapshot(t *testing.T) {
	snapshot := decodeSignedSnapshot(t, validUnsignedSnapshot())
	snapshot.Nodes[0].NodeID = "other"
	if _, err := Reconcile(snapshot); err == nil {
		t.Fatal("Reconcile accepted mutated snapshot")
	}
	if encoded, _ := canonicalBytes(snapshot); bytes.Contains(encoded, []byte(`"decision_sha256"`)) {
		t.Fatal("snapshot unexpectedly contains decision identity")
	}
}
