package persist

import (
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"forgeos/forge-core/internal/materiality"
)

// sampleCheckpoint is a fully-populated checkpoint used across round-trip tests.
// Every field is non-zero so a round-trip that drops or zeroes one is caught.
func sampleCheckpoint() Checkpoint {
	return Checkpoint{
		FormatVersion:     "forgeos.checkpoint.v1",
		Workflow:          "build",
		Mode:              "autonomous",
		Lifecycle:         "production",
		Iteration:         7,
		RoadmapCompletion: 0.625,
		GatesGreen:        true,
		Reason:            "iteration budget reached",
		UpdatedAtUnix:     1_750_000_000,
	}
}

func currentCheckpoint(workflow string, iteration int) Checkpoint {
	return Checkpoint{
		FormatVersion:        CheckpointFormatCurrent,
		Workflow:             workflow,
		WorkflowDigest:       "test-workflow-digest",
		RunID:                "run_id_not_bound",
		Mode:                 "balanced",
		Lifecycle:            "mvp",
		Materiality:          materiality.Unbound,
		Iteration:            iteration,
		RoadmapCompletion:    0.5,
		Reason:               "test checkpoint",
		UpdatedAtUnix:        1_750_000_000,
		PhaseReceipts:        map[string]string{},
		PhaseSemanticOutputs: map[string]string{},
		StageReceipts:        map[string]string{},
		ApprovalContexts:     map[string]string{},
	}
}

func TestSaveLoad_RoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	want := sampleCheckpoint()

	if err := Save(path, want, 0); err != nil {
		t.Fatalf("Save: %v", err)
	}
	got, found, err := Load(path)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if !found {
		t.Fatal("Load found = false, want true after Save")
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("round-trip mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestLoad_Missing_NotAnError(t *testing.T) {
	// A first run has no checkpoint. Absence must be reported as not-found, with
	// no error — the loop should treat it as "start fresh", not as a failure.
	path := filepath.Join(t.TempDir(), "does-not-exist.json")

	got, found, err := Load(path)
	if err != nil {
		t.Fatalf("Load of missing file returned error: %v", err)
	}
	if found {
		t.Error("found = true for a missing file, want false")
	}
	if !reflect.DeepEqual(got, Checkpoint{}) {
		t.Errorf("checkpoint = %+v, want zero value for missing file", got)
	}
}

func TestLoad_MalformedJSON_IsError(t *testing.T) {
	// A present-but-corrupt checkpoint must surface as an explicit error, never
	// be silently swallowed as "no checkpoint" — that would discard real progress.
	path := filepath.Join(t.TempDir(), "corrupt.json")
	if err := os.WriteFile(path, []byte("{not valid json"), 0o644); err != nil {
		t.Fatalf("seed corrupt file: %v", err)
	}

	got, found, err := Load(path)
	if err == nil {
		t.Fatal("Load of malformed JSON returned nil error, want error")
	}
	if found {
		t.Error("found = true for malformed JSON, want false")
	}
	if !reflect.DeepEqual(got, Checkpoint{}) {
		t.Errorf("checkpoint = %+v, want zero value on decode error", got)
	}
}

func TestSave_OverwriteIsAtomicAndClean(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")

	first := sampleCheckpoint()
	if err := Save(path, first, 0); err != nil {
		t.Fatalf("Save first: %v", err)
	}
	second := first
	second.Iteration = 42
	second.Reason = "converged"
	if err := Save(path, second, 0); err != nil {
		t.Fatalf("Save second: %v", err)
	}

	// The overwrite must produce a complete, parseable file holding exactly the
	// second snapshot (the first fully replaced, not appended or interleaved).
	got, found, err := Load(path)
	if err != nil {
		t.Fatalf("Load after overwrite: %v", err)
	}
	if !found || !reflect.DeepEqual(got, second) {
		t.Errorf("after overwrite got %+v (found=%v), want %+v", got, found, second)
	}

	// The temp file is an implementation detail of the atomic write and must not
	// survive a successful Save — a lingering ".tmp" signals a broken commit.
	if _, err := os.Stat(path + ".tmp"); !os.IsNotExist(err) {
		t.Errorf("temp file %q lingered after Save (stat err: %v)", path+".tmp", err)
	}
	// Only the checkpoint file itself should remain in the directory.
	entries, err := os.ReadDir(dir)
	if err != nil {
		t.Fatalf("ReadDir: %v", err)
	}
	if len(entries) != 1 || entries[0].Name() != "checkpoint.json" {
		t.Errorf("directory contents = %v, want exactly [checkpoint.json]", names(entries))
	}
}

func TestSave_CreatesMissingParentDirs(t *testing.T) {
	// Save must materialize the parent path; the loop should not have to pre-make
	// a .forge/state/ tree before its first checkpoint can land.
	path := filepath.Join(t.TempDir(), "nested", "deeper", "checkpoint.json")
	want := sampleCheckpoint()

	if err := Save(path, want, 0); err != nil {
		t.Fatalf("Save into missing dirs: %v", err)
	}
	got, found, err := Load(path)
	if err != nil || !found {
		t.Fatalf("Load after nested Save: found=%v err=%v", found, err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("nested round-trip mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestEncodeDecode_RoundTrip(t *testing.T) {
	// Exercise the pure serialization boundary directly, with no filesystem.
	want := sampleCheckpoint()

	data, err := encode(want)
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("encode/decode mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestDecode_Malformed_IsError(t *testing.T) {
	if _, err := decode([]byte("}{garbage")); err == nil {
		t.Fatal("decode of garbage returned nil error, want error")
	}
}

// SpentUsdMicros (the run-level spend a --resume re-seeds the budget from) must survive
// the encode/decode round-trip exactly — it is an integer, so it round-trips jitter-free
// (unlike a raw USD float). sampleCheckpoint sets it non-zero, so a round-trip that dropped
// or zeroed it would already fail TestSaveLoad_RoundTrip; this pins the field value directly.
func TestEncodeDecode_SpentUsdMicrosRoundTrips(t *testing.T) {
	want := Checkpoint{Workflow: "evolve", Iteration: 3, SpentUsdMicros: 1_234_567}
	data, err := encode(want)
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	if got.SpentUsdMicros != 1_234_567 {
		t.Errorf("SpentUsdMicros round-trip = %d, want 1234567", got.SpentUsdMicros)
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("round-trip mismatch:\n got  %+v\n want %+v", got, want)
	}
}

// PhaseIndex (the phase-granular resume position WITHIN the in-progress
// iteration) must round-trip exactly. Current checkpoints persist an explicit zero at a
// clean iteration boundary so missing recovery state cannot decode as zero.
func TestEncodeDecode_PhaseIndexRoundTripsAndV4PersistsZero(t *testing.T) {
	mid := currentCheckpoint("evolve", 4)
	mid.PhaseIndex = 3
	data, err := encode(mid)
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatalf("decode: %v", err)
	}
	if got.PhaseIndex != 3 || !reflect.DeepEqual(got, mid) {
		t.Errorf("PhaseIndex round-trip = %+v, want PhaseIndex 3 / %+v", got, mid)
	}
	zero, err := encode(currentCheckpoint("evolve", 4))
	if err != nil {
		t.Fatalf("encode zero: %v", err)
	}
	if !strings.Contains(string(zero), `"phase_index": 0`) {
		t.Errorf("a v4 PhaseIndex-0 checkpoint must persist explicit zero; got:\n%s", zero)
	}
}

func TestEncodeDecode_EvolveScanReportRequiresMidIterationPosition(t *testing.T) {
	report := `EVOLVE_SCAN_V1: {"version":"evolve_scan_v1"}`
	mid := currentCheckpoint("evolve", 4)
	mid.PhaseIndex = 1
	mid.EvolveScanReport = report
	mid.EvolveScanSemanticOutput = report
	mid.PhaseSemanticOutputs["evolve/scan"] = report
	data, err := encode(mid)
	if err != nil {
		t.Fatal(err)
	}
	got, err := decode(data)
	if err != nil {
		t.Fatal(err)
	}
	if got.EvolveScanReport != report {
		t.Fatalf("scan report = %q, want %q", got.EvolveScanReport, report)
	}
	mid.PhaseIndex = 0
	if err := validateCurrentCheckpoint(mid); err == nil ||
		!strings.Contains(err.Error(), "positive phase_index") {
		t.Fatalf("iteration-boundary scan report error = %v", err)
	}
}

func TestCheckpointV4BoundsEvolveScanReport(t *testing.T) {
	cp := currentCheckpoint("evolve", 1)
	cp.PhaseIndex, cp.AgentCalls = 1, 1
	cp.EvolveScanReport = strings.Repeat("x", checkpointScanReportMaxBytes)
	cp.EvolveScanSemanticOutput = cp.EvolveScanReport
	cp.PhaseSemanticOutputs["evolve/scan"] = cp.EvolveScanReport
	if err := validateCurrentCheckpoint(cp); err != nil {
		t.Fatalf("exact-limit report rejected: %v", err)
	}
	cp.EvolveScanReport += "x"
	cp.EvolveScanSemanticOutput = cp.EvolveScanReport
	cp.PhaseSemanticOutputs["evolve/scan"] = cp.EvolveScanReport
	if err := validateCurrentCheckpoint(cp); err == nil ||
		!strings.Contains(err.Error(), "valid UTF-8") {
		t.Fatalf("oversized report error = %v", err)
	}
	cp.EvolveScanReport = string([]byte{0xff})
	cp.EvolveScanSemanticOutput = cp.EvolveScanReport
	cp.PhaseSemanticOutputs["evolve/scan"] = cp.EvolveScanReport
	if err := validateCurrentCheckpoint(cp); err == nil ||
		!strings.Contains(err.Error(), "valid UTF-8") {
		t.Fatalf("invalid UTF-8 report error = %v", err)
	}
}

// BACK-COMPAT FOR DIAGNOSTICS: a checkpoint written before spent_usd_micros
// existed still decodes with zero spend. It remains inspectable by status/doctor,
// while autonomous resume separately rejects every pre-v4 generation because it
// lacks the complete materiality-bound recovery envelope.
func TestDecode_OldCheckpointWithoutSpent_DefaultsZero(t *testing.T) {
	// An on-disk checkpoint exactly as the pre-PR encoder produced it: every old field,
	// and crucially NO spent_usd_micros key at all.
	old := []byte(`{
  "workflow": "build",
  "mode": "autonomous",
  "iteration": 7,
  "roadmap_completion": 0.625,
  "gates_green": true,
  "reason": "iteration budget reached",
  "updated_at_unix": 1750000000
}`)
	got, err := decode(old)
	if err != nil {
		t.Fatalf("an old checkpoint without spent_usd_micros must decode cleanly; got %v", err)
	}
	if got.SpentUsdMicros != 0 {
		t.Errorf("a missing spent_usd_micros must decode to 0 (back-compat); got %d", got.SpentUsdMicros)
	}
	// The rest of the old fields must still load — the new field is purely additive.
	if got.Iteration != 7 || got.Workflow != "build" || got.RoadmapCompletion != 0.625 {
		t.Errorf("old checkpoint lost a pre-existing field: %+v", got)
	}
}

// v4's recovery envelope writes zero spend explicitly. Missing spend remains
// accepted only for the diagnostic-readable legacy generations pinned above.
func TestEncode_V4ZeroSpentIsExplicit(t *testing.T) {
	zero, err := encode(currentCheckpoint("evolve", 1))
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	if !strings.Contains(string(zero), `"spent_usd_micros": 0`) {
		t.Errorf("a v4 zero-spend checkpoint must persist explicit zero; got:\n%s", zero)
	}
	nonzero := currentCheckpoint("evolve", 1)
	nonzero.SpentUsdMicros = 500
	data, err := encode(nonzero)
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	if !strings.Contains(string(data), `"spent_usd_micros": 500`) {
		t.Errorf("a billed checkpoint must persist spent_usd_micros; got:\n%s", data)
	}
}

func TestSave_V4NormalizesMissingMaterialityToUnbound(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	cp := currentCheckpoint("evolve", 1)
	cp.Materiality = ""
	if err := Save(path, cp, 0); err != nil {
		t.Fatalf("Save: %v", err)
	}
	got, found, err := Load(path)
	if err != nil || !found {
		t.Fatalf("Load: found=%v err=%v", found, err)
	}
	if got.Materiality != materiality.Unbound {
		t.Fatalf("Materiality = %q, want %q", got.Materiality, materiality.Unbound)
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), `"materiality": "materiality_not_bound"`) {
		t.Fatalf("saved v4 checkpoint does not persist the unbound sentinel:\n%s", data)
	}
}

// names extracts entry names for readable failure messages.
func names(entries []os.DirEntry) []string {
	out := make([]string, len(entries))
	for i, e := range entries {
		out[i] = e.Name()
	}
	return out
}
