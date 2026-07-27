// Package persist — fault injection tests for the persistence layer
// (seventh-wave-data-realism.md §方向4: 故障注入与持久层健壮性测试).
//
// These tests simulate data-layer failures that a production run could encounter:
// corrupted files, concurrent writes, malformed data, and resource exhaustion.
// They verify that the persist layer fails closed, never silently corrupts state,
// and surfaces errors honestly rather than swallowing them.
package persist

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/statefs"
)

// TestSave_CorruptedLastLine verifies that a checkpoint with a trailing
// incomplete JSON line (e.g. a crash during write) is correctly detected as
// malformed — the atomic write contract guarantees this never happens via
// Save's temp+rename, but a manually-edited or external-corruption file must
// be surfaced as an error, never a silent fallback.
func TestSave_CorruptedLastLine(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	// Write a valid checkpoint first.
	cp := currentCheckpoint("evolve", 1)
	cp.RoadmapCompletion = 0.5
	if err := Save(path, cp, 0); err != nil {
		t.Fatalf("Save: %v", err)
	}
	// Now corrupt the file by appending a truncated JSON fragment (what a crash
	// during the old no-atomic write would leave behind).
	f, err := os.OpenFile(path, os.O_APPEND|os.O_WRONLY, 0o644)
	if err != nil {
		t.Fatalf("open for append: %v", err)
	}
	if _, err := f.WriteString(`{"broken`); err != nil {
		f.Close()
		t.Fatalf("append corrupt: %v", err)
	}
	f.Close()

	// Load must see a malformed file and report as an error.
	_, found, err := Load(path)
	if err == nil {
		t.Fatal("Load of corrupted file returned nil error, want error")
	}
	if found {
		t.Error("found = true for corrupted checkpoint, want false")
	}
}

// TestCheckpoint_WrongType verifies that a checkpoint with a wrong JSON type
// (e.g. iteration field set to a string "abc" instead of an int) is detected
// as a decode error, not silently zeroed or truncated.
func TestCheckpoint_WrongType(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	bad := map[string]any{
		"workflow":           "evolve",
		"iteration":          "abc", // wrong type: should be int
		"roadmap_completion": 0.5,
		"gates_green":        true,
		"reason":             "test",
		"updated_at_unix":    1_750_000_000,
	}
	data, err := json.MarshalIndent(bad, "", "  ")
	if err != nil {
		t.Fatalf("marshal bad data: %v", err)
	}
	if err := os.WriteFile(path, append(data, '\n'), 0o644); err != nil {
		t.Fatalf("write bad file: %v", err)
	}

	_, _, err = Load(path)
	if err == nil {
		t.Fatal("Load of type-mismatched checkpoint returned nil error, want error")
	}
}

// TestDecode_UnknownFieldsDoNotFail verifies that an extra, unknown field in
// the JSON is ignored (golang json.Unmarshal by default ignores unknown keys).
// This is important for forward-compatibility when a future forge version adds
// a field to Checkpoint and the old binary needs to load the new-format
// checkpoint — it should succeed, not error.
func TestDecode_UnknownFieldsDoNotFail(t *testing.T) {
	data := []byte(`{
		"workflow": "evolve",
		"iteration": 3,
		"roadmap_completion": 0.75,
		"gates_green": false,
		"reason": "converged",
		"updated_at_unix": 1750000000,
		"future_field": "should be ignored"
	}`)
	cp, err := decode(data)
	if err != nil {
		t.Fatalf("decode with extra field returned error: %v", err)
	}
	if cp.Iteration != 3 || cp.Workflow != "evolve" || cp.RoadmapCompletion != 0.75 {
		t.Errorf("fields corrupted by extra unknown field: %+v", cp)
	}
}

// loadCheckpointIteration loads the checkpoint at path and asserts its
// Iteration equals want, failing the test otherwise. Shared by the rotation
// tests below, which each need to verify several rotated files after a
// chain of Save calls.
func loadCheckpointIteration(t *testing.T, path string, want int) {
	t.Helper()
	got, found, err := Load(path)
	if err != nil || !found {
		t.Fatalf("load %s: found=%v err=%v", path, found, err)
	}
	if got.Iteration != want {
		t.Errorf("%s iteration = %d, want %d", path, got.Iteration, want)
	}
}

// TestSave_RetainHistory verifies that when retain > 0, the old checkpoint
// file is rotated to .1, .2, ... before being overwritten — creating a
// checkpoint history for trend analysis.
func TestSave_RetainHistory(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")

	// First save (retain=3): creates checkpoint.json, no rotation.
	first := currentCheckpoint("evolve", 1)
	first.Reason = "first"
	if err := Save(path, first, 3); err != nil {
		t.Fatalf("Save first: %v", err)
	}

	// Second save: rotates checkpoint.json → checkpoint.json.1, writes new.
	second := currentCheckpoint("evolve", 2)
	second.Reason = "second"
	if err := Save(path, second, 3); err != nil {
		t.Fatalf("Save second: %v", err)
	}

	// Third save: .0→.1, .1→.2, writes new.
	third := currentCheckpoint("evolve", 3)
	third.Reason = "third"
	if err := Save(path, third, 3); err != nil {
		t.Fatalf("Save third: %v", err)
	}

	// Current must be third, .1 must be second, .2 must be first.
	loadCheckpointIteration(t, path, 3)
	loadCheckpointIteration(t, path+".1", 2)
	loadCheckpointIteration(t, path+".2", 1)

	// Should be exactly 3 files (no .3).
	entries, _ := os.ReadDir(dir)
	if len(entries) != 3 {
		t.Errorf("directory has %d entries, want exactly 3", len(entries))
	}
}

func TestSave_RetainFinalCommitFailureKeepsOldCurrent(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	first := currentCheckpoint("evolve", 1)
	first.Reason = "first"
	if err := Save(path, first, 2); err != nil {
		t.Fatalf("Save first: %v", err)
	}

	second := currentCheckpoint("evolve", 2)
	second.Reason = "second"
	writeThenFail := func(target string, data []byte, perm os.FileMode) error {
		if err := statefs.AtomicWrite(target, data, perm); err != nil {
			return err
		}
		if target == path {
			return errors.New("injected final commit failure")
		}
		return nil
	}
	err := saveWithWriter(path, second, 2, writeThenFail)
	if err == nil || !strings.Contains(err.Error(), "injected final commit failure") {
		t.Fatalf("saveWithWriter error = %v, want injected final commit failure", err)
	}
	loadCheckpointIteration(t, path, 1)
	if _, statErr := os.Stat(path + ".1"); !os.IsNotExist(statErr) {
		t.Fatalf("history changed after failed final commit: stat err=%v", statErr)
	}

	if err := Save(path, second, 2); err != nil {
		t.Fatalf("retry Save: %v", err)
	}
	loadCheckpointIteration(t, path, 2)
	loadCheckpointIteration(t, path+".1", 1)
}

func TestSave_RetainHistoryFailureKeepsOldCurrent(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	for iteration := 1; iteration <= 2; iteration++ {
		cp := currentCheckpoint("evolve", iteration)
		cp.Reason = "seed"
		if err := Save(path, cp, 2); err != nil {
			t.Fatalf("seed Save %d: %v", iteration, err)
		}
	}

	third := currentCheckpoint("evolve", 3)
	third.Reason = "third"
	failHistory := func(target string, data []byte, perm os.FileMode) error {
		if target == path+".1" {
			return errors.New("injected history commit failure")
		}
		return statefs.AtomicWrite(target, data, perm)
	}
	err := saveWithWriter(path, third, 2, failHistory)
	if err == nil || !strings.Contains(err.Error(), "injected history commit failure") {
		t.Fatalf("saveWithWriter error = %v, want injected history failure", err)
	}
	loadCheckpointIteration(t, path, 2)
	loadCheckpointIteration(t, path+".1", 1)
	if _, statErr := os.Stat(path + ".2"); !os.IsNotExist(statErr) {
		t.Fatalf("history rollback left unexpected .2 file: stat err=%v", statErr)
	}
}

// TestSave_RetainOverwrite verifies that a repeated Save with retain > 0
// correctly evicts the oldest history entry when the rotation chain fills up.
func TestSave_RetainEviction(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")

	for i := 1; i <= 5; i++ {
		cp := currentCheckpoint("evolve", i)
		cp.Reason = "iteration"
		if err := Save(path, cp, 3); err != nil {
			t.Fatalf("Save %d: %v", i, err)
		}
	}

	// retain=3 should leave 4 files: current (iteration 5), .1 (iteration 4),
	// .2 (iteration 3), .3 (iteration 2) — current file + up to retain backups.
	// Iterations 1 is evicted.
	entries, _ := os.ReadDir(dir)
	if len(entries) != 4 {
		t.Fatalf("directory has %d entries, want 4 (current + retain=3 backups)", len(entries))
	}

	got, _, err := Load(path)
	if err != nil {
		t.Fatalf("load current: %v", err)
	}
	if got.Iteration != 5 {
		t.Errorf("current iteration = %d, want 5 (most recent)", got.Iteration)
	}
}

// TestSave_RotateBackCompat verifies that rotateRetain is a no-op when no prior
// checkpoint exists — the first Save with retain > 0 must succeed cleanly even
// when there is nothing to rotate.
func TestSave_RetainFirstSave(t *testing.T) {
	path := filepath.Join(t.TempDir(), "fresh", "checkpoint.json") // nested dir to test mkdirAll too

	cp := currentCheckpoint("evolve", 1)
	cp.Reason = "fresh"
	if err := Save(path, cp, 5); err != nil {
		t.Fatalf("first Save with retain=5 on clean directory: %v", err)
	}
	got, found, err := Load(path)
	if err != nil || !found {
		t.Fatalf("load after first retain-save: found=%v err=%v", found, err)
	}
	if got.Workflow != "evolve" {
		t.Errorf("first retain-save corrupted: got %+v", got)
	}
}

func TestSave_RetainMissingCurrentPreservesExistingHistory(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	history := currentCheckpoint("evolve", 7)
	data, err := encode(history)
	if err != nil {
		t.Fatal(err)
	}
	if err := statefs.AtomicWrite(path+".1", data, 0o600); err != nil {
		t.Fatal(err)
	}
	current := currentCheckpoint("evolve", 8)
	if err := Save(path, current, 2); err != nil {
		t.Fatal(err)
	}
	loadCheckpointIteration(t, path, 8)
	loadCheckpointIteration(t, path+".1", 7)
	if _, err := os.Stat(path + ".2"); !os.IsNotExist(err) {
		t.Fatalf("missing current spuriously aged history: %v", err)
	}
}

// TestEncode_FormatVersionIsSetOnSave verifies that every Save sets the
// FormatVersion field, so new checkpoints carry the version marker even when
// the caller passes a zero-value Checkpoint.
func TestEncode_FormatVersionIsSetOnSave(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	cp := currentCheckpoint("evolve", 1)
	cp.FormatVersion = ""
	if err := Save(path, cp, 0); err != nil {
		t.Fatalf("Save: %v", err)
	}
	got, _, err := Load(path)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if got.FormatVersion != CheckpointFormatCurrent {
		t.Errorf("FormatVersion = %q, want %q", got.FormatVersion, CheckpointFormatCurrent)
	}
}

// TestDecode_OldFormatWithoutVersionStillLoads verifies that a checkpoint file
// written before FormatVersion was introduced decodes cleanly, with the
// legacy marker left empty so callers can distinguish diagnostic-only state.
func TestDecode_OldFormatWithoutVersionStillLoads(t *testing.T) {
	// A minimal checkpoint without _format field, as an old forge version wrote.
	old := []byte(`{
		"workflow": "build",
		"iteration": 42,
		"roadmap_completion": 1.0,
		"gates_green": true,
		"reason": "converged",
		"updated_at_unix": 1750000000
	}`)
	cp, err := decode(old)
	if err != nil {
		t.Fatalf("decode old pre-format checkpoint: %v", err)
	}
	if cp.FormatVersion != "" {
		t.Errorf("FormatVersion = %q, want empty (back-compat: old files have no _format)", cp.FormatVersion)
	}
	if cp.Iteration != 42 || cp.RoadmapCompletion != 1.0 {
		t.Errorf("old checkpoint fields corrupted: %+v", cp)
	}
}

// TestEncode_FormatVersionKeyInJSON verifies that the FormatVersion field
// serialises as "_format" in the JSON, matching the JSON tag.
func TestEncode_FormatVersionKeyInJSON(t *testing.T) {
	data, err := encode(Checkpoint{Workflow: "test", Iteration: 1, FormatVersion: "forgeos.checkpoint.v1"})
	if err != nil {
		t.Fatalf("encode: %v", err)
	}
	if !strings.Contains(string(data), `"_format": "forgeos.checkpoint.v1"`) {
		t.Errorf("encoded JSON missing _format key:\\n%s", data)
	}
}

// TestSave_MidFileCorruptionResilience verifies that a checkpoint whose
// sibling temp file was left behind from a previous crash is cleaned up —
// the temp file does not block the next Save.
func TestSave_TempFileCleanup(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")
	tmpPath := path + ".tmp"

	// Seed a stale temp file as if a previous save crashed before rename.
	if err := os.WriteFile(tmpPath, []byte(`{"stale": true}`), 0o644); err != nil {
		t.Fatalf("write stale tmp: %v", err)
	}

	cp := currentCheckpoint("evolve", 1)
	cp.Reason = "clean"
	if err := Save(path, cp, 0); err != nil {
		t.Fatalf("Save after stale tmp: %v", err)
	}

	// The stale temp file should be gone (Save's rename step cleans it).
	if _, err := os.Stat(tmpPath); !os.IsNotExist(err) {
		t.Errorf("stale temp file %q not removed after Save (stat: %v)", tmpPath, err)
	}
}
