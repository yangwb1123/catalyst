package main

import (
	"encoding/json"
	"io"
	"os"
	"strings"
	"testing"

	"forgeos/forge-core/internal/asset"
	"forgeos/forge-core/internal/converge"
	"forgeos/forge-core/internal/materiality"
	"forgeos/forge-core/internal/persist"
	"forgeos/forge-core/internal/trace"
)

func TestChainStateV4RequiresDurableMateriality(t *testing.T) {
	root := t.TempDir()
	if err := saveChainState(root, chainState{
		Status: "running", CurrentStage: "build", Materiality: "L3",
	}); err != nil {
		t.Fatal(err)
	}
	state, found, err := loadChainState(root)
	if err != nil || !found || state.Format != chainStateFormat || state.Materiality != "L3" {
		t.Fatalf("v4 state = %+v found=%v err=%v", state, found, err)
	}

	data, err := os.ReadFile(chainStatePath(root))
	if err != nil {
		t.Fatal(err)
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		t.Fatal(err)
	}
	assertChainMaterialityFieldRejections(t, root, fields)
}

func assertChainMaterialityFieldRejections(t *testing.T, root string, fields map[string]json.RawMessage) {
	t.Helper()
	for _, tc := range []struct {
		name  string
		value *string
		want  string
	}{
		{name: "missing", want: "missing required field"},
		{name: "null", value: stringPointer("null"), want: "is null"},
		{name: "invalid", value: stringPointer(`"L5"`), want: "invalid materiality"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			copyFields := make(map[string]json.RawMessage, len(fields))
			for key, value := range fields {
				copyFields[key] = value
			}
			if tc.value == nil {
				delete(copyFields, "materiality")
			} else {
				copyFields["materiality"] = json.RawMessage(*tc.value)
			}
			mutated, err := json.Marshal(copyFields)
			if err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(chainStatePath(root), mutated, 0o600); err != nil {
				t.Fatal(err)
			}
			if _, found, err := loadChainState(root); !found || err == nil ||
				!strings.Contains(err.Error(), tc.want) {
				t.Fatalf("load found=%v err=%v, want %q", found, err, tc.want)
			}
		})
	}
}

func TestChainStateNormalizesOmissionAndRejectsInvalidSave(t *testing.T) {
	root := t.TempDir()
	if err := saveChainState(root, chainState{Status: "running"}); err != nil {
		t.Fatal(err)
	}
	state, found, err := loadChainState(root)
	if err != nil || !found || state.Materiality != materiality.Unbound {
		t.Fatalf("normalized state = %+v found=%v err=%v", state, found, err)
	}
	if err := saveChainState(t.TempDir(), chainState{
		Status: "running", Materiality: "L5",
	}); err == nil || !strings.Contains(err.Error(), "materiality") {
		t.Fatalf("invalid materiality save error = %v", err)
	}
}

func TestChainStateV2IsDiagnosticReadableButNotResumable(t *testing.T) {
	root := t.TempDir()
	if err := os.MkdirAll(forgeDir(root), 0o700); err != nil {
		t.Fatal(err)
	}
	legacy := `{"_format":"forgeos.chain-state.v2","status":"waiting_approval","current_stage":"review"}`
	if err := os.WriteFile(chainStatePath(root), []byte(legacy), 0o600); err != nil {
		t.Fatal(err)
	}
	state, found, err := loadChainState(root)
	if err != nil || !found || state.Format != chainStateFormatV2 {
		t.Fatalf("diagnostic load = %+v found=%v err=%v", state, found, err)
	}
	if status := chainStatusForDisplay(root); status == nil || status.Error != "" ||
		status.Status != "waiting_approval" {
		t.Fatalf("legacy status = %#v", status)
	}
	if err := validateResumableChainState(state); err == nil ||
		!strings.Contains(err.Error(), "diagnostic-only") {
		t.Fatalf("legacy resume error = %v", err)
	}
}

func TestChainResumeRestoresMaterialityAndRejectsEveryExplicitMismatch(t *testing.T) {
	state := resumableTestState("design", "design", nil)
	state.Format, state.Materiality = chainStateFormat, "L3"
	base := runOpts{
		mode: "balanced", lifecycle: "idea", materiality: materiality.Unbound,
		maxChainStages: defaultMaxChainStages, runFlagsCaptured: true,
	}
	omitted := base
	if _, err := restoreChainRunOptions(&omitted, &runBudget{}, &state, "idea"); err != nil {
		t.Fatal(err)
	}
	if omitted.materiality != "L3" {
		t.Fatalf("omitted resume materiality = %q, want L3", omitted.materiality)
	}

	for _, requested := range []string{"L0", "L2", "L4"} {
		t.Run(requested, func(t *testing.T) {
			o := base
			o.materiality, o.materialityExplicit = requested, true
			if _, err := restoreChainRunOptions(&o, &runBudget{}, &state, "idea"); err == nil ||
				!strings.Contains(err.Error(), "persisted --materiality=L3") {
				t.Fatalf("explicit mismatch error = %v", err)
			}
		})
	}
	exact := base
	exact.materiality, exact.materialityExplicit = "L3", true
	if _, err := restoreChainRunOptions(&exact, &runBudget{}, &state, "idea"); err != nil {
		t.Fatalf("exact materiality rejected: %v", err)
	}
}

func TestChainStatusDisplaysMateriality(t *testing.T) {
	root := t.TempDir()
	if err := saveChainState(root, chainState{
		Status: "waiting_approval", CurrentStage: "review", Materiality: "L4",
	}); err != nil {
		t.Fatal(err)
	}
	status := chainStatusForDisplay(root)
	if status == nil || status.Materiality != "L4" {
		t.Fatalf("status = %#v", status)
	}
	text := captureStdout(t, func() { printChainStatusText(root) })
	if !strings.Contains(text, "materiality=L4") {
		t.Fatalf("text status omits materiality: %s", text)
	}
}

func TestEvolveResumeMaterialityBindingRestoresOmissionAndRejectsMismatch(t *testing.T) {
	cp := persist.Checkpoint{
		FormatVersion: persist.CheckpointFormatCurrent,
		Workflow:      "evolve", WorkflowDigest: "digest",
		Mode: "balanced", Lifecycle: "mvp", Materiality: "L3",
		Iteration: 1, RoadmapCompletion: 0.5, Reason: "iteration complete",
		UpdatedAtUnix: 1_750_000_000,
	}
	omitted := checkpointBinding{
		Workflow: "evolve", WorkflowDigest: "digest", Mode: "balanced", Lifecycle: "mvp",
		Materiality: materiality.Unbound, PhaseLimit: 1,
	}
	if err := validateResumeCheckpoint(cp, omitted); err != nil {
		t.Fatalf("omitted binding rejected: %v", err)
	}
	exact := omitted
	exact.Materiality, exact.MaterialityExplicit = "L3", true
	if err := validateResumeCheckpoint(cp, exact); err != nil {
		t.Fatalf("exact binding rejected: %v", err)
	}
	for _, requested := range []string{"L2", "L4"} {
		mismatch := omitted
		mismatch.Materiality, mismatch.MaterialityExplicit = requested, true
		if err := validateResumeCheckpoint(cp, mismatch); err == nil ||
			!strings.Contains(err.Error(), "materiality mismatch") {
			t.Fatalf("%s mismatch error = %v", requested, err)
		}
	}

	state := loopResumeState{found: true, materiality: "L3", maxLoopBacks: maxLoopBack}
	o := runOpts{materiality: materiality.Unbound, runFlagsCaptured: true}
	if err := restoreResumeRunOptions(&o, state); err != nil || o.materiality != "L3" {
		t.Fatalf("restore omission materiality=%q err=%v", o.materiality, err)
	}
	o = runOpts{materiality: "L4", materialityExplicit: true, runFlagsCaptured: true}
	if err := restoreResumeRunOptions(&o, state); err == nil ||
		!strings.Contains(err.Error(), "persisted --materiality=L3") {
		t.Fatalf("restore mismatch error = %v", err)
	}
}

func TestCheckpointHooksPersistExactMateriality(t *testing.T) {
	wf := asset.Workflow{Stage: "evolve"}
	phaseRoot := t.TempDir()
	phase := phaseCheckpointHook(runOpts{
		root: phaseRoot, mode: "balanced", lifecycle: "mvp", materiality: "L4",
	}, wf, &runBudget{}, nil, func(string) {})
	if err := phase(1, 0, 1, 0); err != nil {
		t.Fatal(err)
	}
	assertCheckpointMateriality(t, phaseRoot, "L4")

	iterationRoot := t.TempDir()
	iterationOpts := runOpts{
		root: iterationRoot, mode: "balanced", lifecycle: "mvp", materiality: "L3",
	}
	iteration := checkpointHook(iterationOpts, wf, trace.NewTracer(io.Discard),
		&runBudget{}, func(string) {}, nil, nil)
	if err := iteration(1, converge.Signals{GatesGreen: true}, 0); err != nil {
		t.Fatal(err)
	}
	assertCheckpointMateriality(t, iterationRoot, "L3")

	invalidRoot := t.TempDir()
	invalid := phaseCheckpointHook(runOpts{
		root: invalidRoot, mode: "balanced", lifecycle: "mvp", materiality: "L5",
	}, wf, &runBudget{}, nil, func(string) {})
	if err := invalid(1, 0, 1, 0); err == nil || !strings.Contains(err.Error(), "materiality") {
		t.Fatalf("invalid hook materiality error = %v", err)
	}
}

func assertCheckpointMateriality(t *testing.T, root, want string) {
	t.Helper()
	cp, found, err := persist.Load(checkpointPath(root))
	if err != nil || !found || cp.Materiality != want ||
		cp.FormatVersion != persist.CheckpointFormatCurrent {
		t.Fatalf("checkpoint = %+v found=%v err=%v, want materiality %s", cp, found, err, want)
	}
}

func stringPointer(value string) *string { return &value }
