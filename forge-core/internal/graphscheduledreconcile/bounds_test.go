package graphscheduledreconcile

import (
	"bytes"
	"fmt"
	"testing"
)

func TestReconcileClassifiesTheFullNodeBound(t *testing.T) {
	tests := []struct {
		name         string
		completed    int
		want         string
		wantOrdinal  *uint16
		wantNextNode *string
	}{
		{"completed prefix leaves final ready", 31, "ready", uint16Pointer(31), stringPointer("node-31")},
		{"all nodes completed", 32, "completed", nil, nil},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			snapshot := maxBoundSnapshot(test.completed)
			encoded := signedSnapshotBytes(t, snapshot)
			if len(encoded) > MaxProgressSnapshotBytes {
				t.Fatalf("canonical snapshot bytes = %d; maximum %d", len(encoded), MaxProgressSnapshotBytes)
			}
			decoded, err := DecodeSnapshot(bytes.NewReader(encoded))
			if err != nil {
				t.Fatalf("DecodeSnapshot: %v", err)
			}
			decision, err := Reconcile(decoded)
			if err != nil || decision.Disposition != test.want {
				t.Fatalf("Reconcile disposition = %q, %v; want %q", decision.Disposition, err, test.want)
			}
			assertNextFields(t, decision, test.wantOrdinal, test.wantNextNode)
		})
	}
}

func maxBoundSnapshot(completed int) ProgressSnapshot {
	value := validUnsignedSnapshot()
	value.NodeCount = 32
	value.Nodes = make([]ProgressNode, value.NodeCount)
	for index := range value.Nodes {
		value.Nodes[index] = ProgressNode{
			ExecutionOrdinal: uint16(index),
			NodeID:           fmt.Sprintf("node-%02d", index),
			Attempt:          1,
		}
		if index < completed {
			completeBoundNode(&value.Nodes[index], index)
		}
	}
	return value
}

func completeBoundNode(node *ProgressNode, index int) {
	candidateDigest := boundDigest(index*3 + 1)
	requestDigest := boundDigest(index*3 + 2)
	node.CandidateID = stringPointer(candidateIDPrefix + candidateDigest)
	node.CandidateSHA256 = stringPointer(candidateDigest)
	node.ProviderRequestID = stringPointer(providerIDPrefix + requestDigest)
	node.PreparedRequestSHA256 = stringPointer(requestDigest)
	node.LifecycleStatus = stringPointer("terminalized")
	node.TerminalOutcome = stringPointer("completed")
	node.TerminalReceiptSHA256 = stringPointer(boundDigest(index*3 + 3))
}

func boundDigest(value int) string {
	return fmt.Sprintf("%064x", value)
}

func uint16Pointer(value uint16) *uint16 {
	return &value
}
