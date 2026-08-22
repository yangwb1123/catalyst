package main

import (
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func resumableTestState(entry, current string, completed []string) chainState {
	state := chainState{
		RunID: "resume-test", Status: "waiting_approval",
		EntryStage: entry, CurrentStage: current,
		CompletedStages: append([]string(nil), completed...),
		Mode:            "balanced", Lifecycle: "idea", MaxChainStages: defaultMaxChainStages,
	}
	state.WorkflowDigests = make(map[string]string, len(completed)+1)
	for _, stage := range completed {
		state.WorkflowDigests[stage] = strings.Repeat("0", workflowDigestHexLength)
	}
	state.WorkflowDigests[current] = strings.Repeat("0", workflowDigestHexLength)
	return state
}

func bindResumableTestState(t *testing.T, root string, state *chainState) {
	t.Helper()
	state.WorkflowDigests = make(map[string]string)
	for _, stage := range append(append([]string(nil), state.CompletedStages...), state.CurrentStage) {
		wf := loadChainWorkflowForTest(t, root, stage)
		if err := state.bindWorkflow(wf.Stage, checkpointWorkflowDigest(wf)); err != nil {
			t.Fatal(err)
		}
	}
}

func bindAvailableResumableTestState(t *testing.T, root string, state *chainState) {
	t.Helper()
	for _, stage := range append(append([]string(nil), state.CompletedStages...), state.CurrentStage) {
		wf, err := loadWorkflowNativeOnly(root, stage)
		if err == nil {
			state.WorkflowDigests[stage] = checkpointWorkflowDigest(wf)
		}
	}
}

func loadChainWorkflowForTest(t *testing.T, root, stage string) asset.Workflow {
	t.Helper()
	wf, err := loadWorkflowNativeOnly(root, stage)
	if err != nil {
		t.Fatalf("load chain workflow %q: %v", stage, err)
	}
	return wf
}
