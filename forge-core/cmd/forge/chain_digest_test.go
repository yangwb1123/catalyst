package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
)

func TestChainResumeRejectsCompletedHighBuildWorkflowMutationBeforeWork(t *testing.T) {
	root, entry, _ := highBuildWaitingFixture(t)
	before, err := os.ReadFile(chainStatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	writeChainAsset(t, root, "build", humanChainWorkflow("build", "design", "mutated-phase"))
	if _, _, err := prepareChainResume(entry, chainOpts(root)); err == nil ||
		!strings.Contains(err.Error(), `workflow digest mismatch for stage "build"`) {
		t.Fatalf("mutated Build resume error=%v", err)
	}
	after, err := os.ReadFile(chainStatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(after, before) {
		t.Fatal("stale resume rewrote its durable cursor before rejection")
	}
	if _, err := os.Stat(filepath.Join(forgeDir(root), "trace.jsonl")); !os.IsNotExist(err) {
		t.Fatalf("stale resume created trace before rejection: %v", err)
	}
}

func TestChainResumeAcceptsExactWorkflowDigestSet(t *testing.T) {
	root, entry, want := highBuildWaitingFixture(t)
	current, got, err := prepareChainResume(entry, chainOpts(root))
	if err != nil {
		t.Fatalf("exact resume rejected: %v", err)
	}
	if current.Stage != "design" || got == nil ||
		got.WorkflowDigests["build"] != want.WorkflowDigests["build"] ||
		got.WorkflowDigests["design"] != want.WorkflowDigests["design"] {
		t.Fatalf("exact resume current=%+v state=%+v", current, got)
	}
}

func TestChainResumeRejectsMissingStageWorkflowDigest(t *testing.T) {
	root, entry, state := highBuildWaitingFixture(t)
	delete(state.WorkflowDigests, "design")
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	if _, _, err := prepareChainResume(entry, chainOpts(root)); err == nil ||
		!strings.Contains(err.Error(), `lacks workflow digest for stage "design"`) {
		t.Fatalf("missing stage digest error = %v", err)
	}
}

func TestChainStateV4RejectsMissingNullAndInvalidDigestField(t *testing.T) {
	root, _, _ := highBuildWaitingFixture(t)
	data, err := os.ReadFile(chainStatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		t.Fatal(err)
	}
	assertChainDigestFieldRejections(t, root, fields)
}

func TestChainStateV4RejectsDuplicateCoreFields(t *testing.T) {
	root, _, _ := highBuildWaitingFixture(t)
	data, err := os.ReadFile(chainStatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	for _, tc := range []struct{ name, prefix string }{
		{"materiality", `{"materiality":"L4",`},
		{"workflow_digests", `{"workflow_digests":{},`},
		{"mode", `{"mode":"engineering",`},
		{"cursor", `{"current_stage":"review",`},
	} {
		t.Run(tc.name, func(t *testing.T) {
			assertChainStateLoadFails(t, root, append([]byte(tc.prefix), data[1:]...), "duplicate field")
		})
	}
}

func TestChainStateV4RejectsDuplicateWorkflowDigestStage(t *testing.T) {
	root, _, state := highBuildWaitingFixture(t)
	data, err := os.ReadFile(chainStatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		t.Fatal(err)
	}
	digest := state.WorkflowDigests["build"]
	fields["workflow_digests"] = json.RawMessage(`{"build":"` + digest + `","build":"` + digest + `"}`)
	mutated, err := json.Marshal(fields)
	if err != nil {
		t.Fatal(err)
	}
	assertChainStateLoadFails(t, root, mutated, "duplicate field")
}

func TestChainStateV5RejectsUnsortedNestedRecoveryMap(t *testing.T) {
	root, _, state := highBuildWaitingFixture(t)
	data, err := os.ReadFile(chainStatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		t.Fatal(err)
	}
	fields["workflow_digests"] = json.RawMessage(`{"design":"` +
		state.WorkflowDigests["design"] + `","build":"` + state.WorkflowDigests["build"] + `"}`)
	mutated, err := json.Marshal(fields)
	if err != nil {
		t.Fatal(err)
	}
	assertChainStateLoadFails(t, root, mutated, "must be sorted")
}

func assertChainDigestFieldRejections(t *testing.T, root string, fields map[string]json.RawMessage) {
	t.Helper()
	for _, tc := range []struct {
		name, raw, want string
	}{
		{"missing", "", "missing required field"},
		{"null", "null", "is null"},
		{"invalid", `{"build":"not-a-digest"}`, "lowercase SHA-256"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			mutated := cloneChainRawFields(fields)
			if tc.raw == "" {
				delete(mutated, "workflow_digests")
			} else {
				mutated["workflow_digests"] = json.RawMessage(tc.raw)
			}
			data, err := json.Marshal(mutated)
			if err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(chainStatePath(root), data, 0o600); err != nil {
				t.Fatal(err)
			}
			if _, found, err := loadChainState(root); !found || err == nil ||
				!strings.Contains(err.Error(), tc.want) {
				t.Fatalf("load found=%v err=%v, want %q", found, err, tc.want)
			}
		})
	}
}

func TestChainStateV3IsDiagnosticOnlyWithoutWorkflowDigests(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(forgeDir(root), 0o700); err != nil {
		t.Fatal(err)
	}
	raw := `{"_format":"forgeos.chain-state.v3","run_id":"old","status":"waiting_approval",` +
		`"entry_stage":"build","current_stage":"design","completed_stages":["build"],` +
		`"mode":"balanced","lifecycle":"idea","materiality":"L3","max_chain_stages":8}`
	if err := os.WriteFile(chainStatePath(root), []byte(raw), 0o600); err != nil {
		t.Fatal(err)
	}
	state, found, err := loadChainState(root)
	if err != nil || !found || state.Format != chainStateFormatV3 {
		t.Fatalf("v3 diagnostic load=%+v found=%v err=%v", state, found, err)
	}
	if status := chainStatusForDisplay(root); status == nil || status.Error != "" {
		t.Fatalf("v3 diagnostic status=%+v", status)
	}
	if err := validateResumableChainState(state); err == nil ||
		!strings.Contains(err.Error(), "diagnostic-only") {
		t.Fatalf("v3 resume error=%v", err)
	}
}

func highBuildWaitingFixture(t *testing.T) (string, asset.Workflow, chainState) {
	t.Helper()
	root := t.TempDir()
	writeChainAsset(t, root, "build", humanChainWorkflow("build", "design"))
	writeChainAsset(t, root, "design", humanChainWorkflow("design", ""))
	entry := loadChainWorkflowForTest(t, root, "build")
	state := chainState{
		RunID: "high-build-wait", Status: "waiting_approval",
		EntryStage: "build", CurrentStage: "design", CompletedStages: []string{"build"},
		Mode: "balanced", Lifecycle: "idea", Materiality: "L3",
		MaxChainStages: defaultMaxChainStages, WorkflowDigests: make(map[string]string),
	}
	for _, stage := range []string{"build", "design"} {
		wf := loadChainWorkflowForTest(t, root, stage)
		if err := state.bindWorkflow(wf.Stage, checkpointWorkflowDigest(wf)); err != nil {
			t.Fatal(err)
		}
	}
	if err := saveChainState(root, state); err != nil {
		t.Fatal(err)
	}
	return root, entry, state
}

func cloneChainRawFields(fields map[string]json.RawMessage) map[string]json.RawMessage {
	cloned := make(map[string]json.RawMessage, len(fields))
	for name, raw := range fields {
		cloned[name] = append(json.RawMessage(nil), raw...)
	}
	return cloned
}

func assertChainStateLoadFails(t *testing.T, root string, data []byte, want string) {
	t.Helper()
	if err := os.WriteFile(chainStatePath(root), data, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, found, err := loadChainState(root); !found || err == nil ||
		!strings.Contains(err.Error(), want) {
		t.Fatalf("load found=%v err=%v, want %q", found, err, want)
	}
}
