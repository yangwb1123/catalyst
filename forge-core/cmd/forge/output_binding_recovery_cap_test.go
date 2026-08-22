package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/outputbindingstore"
	"forgeos/forge-core/internal/persist"
)

func TestOutputBindingRecoverySemanticCapPrecedesReceiptAppend(t *testing.T) {
	for _, test := range []struct {
		name    string
		durable bool
		size    int
		wantErr bool
	}{
		{"durable at cap", true, persist.RecoverySemanticOutputMaxBytes, false},
		{"durable over cap", true, persist.RecoverySemanticOutputMaxBytes + 1, true},
		{"non-durable over recovery cap", false, persist.RecoverySemanticOutputMaxBytes + 1, false},
	} {
		t.Run(test.name, func(t *testing.T) {
			runtime, phase := recoveryCapAttempt(t, test.durable)
			semantic := strings.Repeat("x", test.size)
			err := runtime.commit(phase.Name, semantic, semantic, 0)
			if test.wantErr != (err != nil) {
				t.Fatalf("commit error = %v, wantErr=%v", err, test.wantErr)
			}
			receipts, loadErr := outputbindingstore.New(runtime.root).Load()
			if loadErr != nil {
				t.Fatal(loadErr)
			}
			wantReceipts := 1
			if test.wantErr {
				wantReceipts = 0
			}
			if len(receipts) != wantReceipts {
				t.Fatalf("receipt count = %d, want %d", len(receipts), wantReceipts)
			}
		})
	}
}

func recoveryCapAttempt(t *testing.T, durable bool) (*outputBindingRuntime, asset.Phase) {
	t.Helper()
	runtime, _ := outputBindingFixture(t)
	phase := asset.Phase{Name: "planner", Agent: "planner", Readonly: true, FeedsForward: durable}
	runtime.wf.Phases = append([]asset.Phase{phase}, runtime.wf.Phases...)
	runtime.workflowSHA = checkpointWorkflowDigest(runtime.wf)
	runtime.priorEmits = priorEmitsOf(runtime.wf)
	writeBindingWorkflow(t, runtime)
	if err := runtime.prepare(phase, "engineering"); err != nil {
		t.Fatal(err)
	}
	runtime.recordBuild(phase, "opus", "prompt", "", nil)
	if _, err := runtime.finalize(phase, "engineering", []string{"agent", "-p", "prompt"}); err != nil {
		t.Fatal(err)
	}
	return runtime, phase
}
