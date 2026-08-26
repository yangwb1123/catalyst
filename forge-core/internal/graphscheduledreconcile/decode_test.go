package graphscheduledreconcile

import (
	"bytes"
	"strings"
	"testing"
)

func TestDecodeSnapshotAcceptsOnlyExactCanonicalBytes(t *testing.T) {
	valid := signedSnapshotBytes(t, validUnsignedSnapshot())
	if _, err := DecodeSnapshot(bytes.NewReader(valid)); err != nil {
		t.Fatalf("DecodeSnapshot(valid): %v", err)
	}
	cases := map[string][]byte{
		"leading whitespace": append([]byte(" "), valid...),
		"trailing newline":   append(append([]byte(nil), valid...), '\n'),
		"trailing value":     append(append([]byte(nil), valid...), []byte("{}")...),
		"unknown field":      bytes.Replace(valid, []byte(`{"v":1`), []byte(`{"unknown":0,"v":1`), 1),
		"duplicate field":    bytes.Replace(valid, []byte(`{"v":1`), []byte(`{"v":1,"v":1`), 1),
		"equivalent key":     bytes.Replace(valid, []byte(`"v":1`), []byte(`"V":1`), 1),
		"equivalent escape":  bytes.Replace(valid, []byte("graph-run-test"), []byte(`graph\u002drun-test`), 1),
		"corrupt digest":     bytes.Replace(valid, []byte(`"snapshot_sha256":"`), []byte(`"snapshot_sha256":"f`), 1),
		"oversized":          bytes.Repeat([]byte(" "), MaxProgressSnapshotBytes+1),
	}
	for name, input := range cases {
		t.Run(name, func(t *testing.T) {
			if _, err := DecodeSnapshot(bytes.NewReader(input)); err == nil {
				t.Fatal("DecodeSnapshot unexpectedly succeeded")
			}
		})
	}
}

func TestDecodeSnapshotRejectsInvalidStructureWithValidOuterDigest(t *testing.T) {
	tests := structuralMutations()
	for name, mutate := range tests {
		t.Run(name, func(t *testing.T) {
			value := validUnsignedSnapshot()
			mutate(&value)
			if _, err := DecodeSnapshot(bytes.NewReader(signedSnapshotBytes(t, value))); err == nil {
				t.Fatal("DecodeSnapshot unexpectedly succeeded")
			}
		})
	}
}

func structuralMutations() map[string]func(*ProgressSnapshot) {
	tests := map[string]func(*ProgressSnapshot){
		"policy":            func(v *ProgressSnapshot) { v.ExecutionMode = "parallel" },
		"schedule identity": func(v *ProgressSnapshot) { v.ScheduleID = "schedule-other" },
		"node count":        func(v *ProgressSnapshot) { v.NodeCount = 3 },
		"ordinal gap":       func(v *ProgressSnapshot) { v.Nodes[1].ExecutionOrdinal = 2 },
		"duplicate node":    func(v *ProgressSnapshot) { v.Nodes[1].NodeID = v.Nodes[0].NodeID },
		"attempt":           func(v *ProgressSnapshot) { v.Nodes[0].Attempt = 2 },
		"candidate half": func(v *ProgressSnapshot) {
			v.Nodes[0].CandidateID = stringPointer(candidateIDPrefix + strings.Repeat("b", 64))
		},
		"candidate identity": func(v *ProgressSnapshot) {
			materialize(&v.Nodes[0], "b", "c")
			v.Nodes[0].CandidateID = stringPointer(candidateIDPrefix + strings.Repeat("e", 64))
		},
		"request without candidate": func(v *ProgressSnapshot) { materializeRequestOnly(&v.Nodes[0]) },
		"lifecycle without request": func(v *ProgressSnapshot) {
			v.Nodes[0].LifecycleStatus = stringPointer("claimed")
		},
		"terminal evidence missing": func(v *ProgressSnapshot) {
			materialize(&v.Nodes[0], "b", "c")
			v.Nodes[0].LifecycleStatus = stringPointer("terminalized")
		},
		"nonterminal outcome": func(v *ProgressSnapshot) {
			setLifecycle(&v.Nodes[0], "claimed", "")
			v.Nodes[0].TerminalOutcome = stringPointer("completed")
		},
		"unknown lifecycle": func(v *ProgressSnapshot) {
			setLifecycle(&v.Nodes[0], "unknown", "")
		},
	}
	for name, mutate := range duplicateIdentityMutations() {
		tests[name] = mutate
	}
	return tests
}

func duplicateIdentityMutations() map[string]func(*ProgressSnapshot) {
	return map[string]func(*ProgressSnapshot){
		"provider identity": func(v *ProgressSnapshot) {
			materialize(&v.Nodes[0], "b", "c")
			v.Nodes[0].PreparedRequestSHA256 = stringPointer(strings.Repeat("e", 64))
		},
		"duplicate candidate": func(v *ProgressSnapshot) {
			materialize(&v.Nodes[0], "b", "c")
			materialize(&v.Nodes[1], "b", "e")
		},
		"duplicate request": func(v *ProgressSnapshot) {
			materialize(&v.Nodes[0], "b", "c")
			materialize(&v.Nodes[1], "e", "c")
		},
		"duplicate receipt": arrangeDuplicateReceipt,
	}
}

func arrangeDuplicateReceipt(value *ProgressSnapshot) {
	setLifecycle(&value.Nodes[0], "terminalized", "completed")
	materialize(&value.Nodes[1], "e", "f")
	value.Nodes[1].LifecycleStatus = stringPointer("terminalized")
	value.Nodes[1].TerminalOutcome = stringPointer("completed")
	value.Nodes[1].TerminalReceiptSHA256 = value.Nodes[0].TerminalReceiptSHA256
}

func materializeRequestOnly(node *ProgressNode) {
	digest := strings.Repeat("c", 64)
	node.PreparedRequestSHA256 = stringPointer(digest)
	node.ProviderRequestID = stringPointer(providerIDPrefix + digest)
}

func TestDecodeAcceptsNoncontiguousEvidenceForCoreClassification(t *testing.T) {
	value := validUnsignedSnapshot()
	materialize(&value.Nodes[1], "e", "f")
	snapshot := decodeSignedSnapshot(t, value)
	decision, err := Reconcile(snapshot)
	if err != nil || decision.Disposition != "incompatible_progress" {
		t.Fatalf("Reconcile = %#v, %v", decision, err)
	}
}
