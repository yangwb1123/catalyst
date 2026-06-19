package persist

import (
	"os"
	"path/filepath"
	"testing"
)

// sampleCheckpoint is a fully-populated checkpoint used across round-trip tests.
// Every field is non-zero so a round-trip that drops or zeroes one is caught.
func sampleCheckpoint() Checkpoint {
	return Checkpoint{
		Workflow:          "build",
		Mode:              "autonomous",
		Iteration:         7,
		RoadmapCompletion: 0.625,
		GatesGreen:        true,
		Reason:            "iteration budget reached",
		UpdatedAtUnix:     1_750_000_000,
	}
}

func TestSaveLoad_RoundTrip(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	want := sampleCheckpoint()

	if err := Save(path, want); err != nil {
		t.Fatalf("Save: %v", err)
	}
	got, found, err := Load(path)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if !found {
		t.Fatal("Load found = false, want true after Save")
	}
	if got != want {
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
	if got != (Checkpoint{}) {
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
	if got != (Checkpoint{}) {
		t.Errorf("checkpoint = %+v, want zero value on decode error", got)
	}
}

func TestSave_OverwriteIsAtomicAndClean(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")

	first := sampleCheckpoint()
	if err := Save(path, first); err != nil {
		t.Fatalf("Save first: %v", err)
	}
	second := first
	second.Iteration = 42
	second.Reason = "converged"
	if err := Save(path, second); err != nil {
		t.Fatalf("Save second: %v", err)
	}

	// The overwrite must produce a complete, parseable file holding exactly the
	// second snapshot (the first fully replaced, not appended or interleaved).
	got, found, err := Load(path)
	if err != nil {
		t.Fatalf("Load after overwrite: %v", err)
	}
	if !found || got != second {
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

	if err := Save(path, want); err != nil {
		t.Fatalf("Save into missing dirs: %v", err)
	}
	got, found, err := Load(path)
	if err != nil || !found {
		t.Fatalf("Load after nested Save: found=%v err=%v", found, err)
	}
	if got != want {
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
	if got != want {
		t.Errorf("encode/decode mismatch:\n got  %+v\n want %+v", got, want)
	}
}

func TestDecode_Malformed_IsError(t *testing.T) {
	if _, err := decode([]byte("}{garbage")); err == nil {
		t.Fatal("decode of garbage returned nil error, want error")
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
