package persist

import (
	"os"
	"path/filepath"
	"testing"
)

func TestSaveCreatesPrivateCheckpoint(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	if err := Save(path, currentCheckpoint("build", 0), 0); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("checkpoint permissions = %04o, want 0600", got)
	}
}

func TestSaveTightensLegacyCheckpointPermissions(t *testing.T) {
	path := filepath.Join(t.TempDir(), "checkpoint.json")
	if err := os.WriteFile(path, []byte("{}\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := Save(path, currentCheckpoint("build", 0), 0); err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("rewritten checkpoint permissions = %04o, want 0600", got)
	}
}

func TestSaveTightensAllRotatedCheckpointPermissions(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")
	for _, candidate := range []string{path, path + ".1", path + ".2"} {
		if err := os.WriteFile(candidate, []byte("{}\n"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	if err := Save(path, currentCheckpoint("build", 4), 3); err != nil {
		t.Fatal(err)
	}
	for _, candidate := range []string{path, path + ".1", path + ".2", path + ".3"} {
		info, err := os.Stat(candidate)
		if err != nil {
			t.Fatal(err)
		}
		if got := info.Mode().Perm(); got != 0o600 {
			t.Errorf("%s permissions = %04o, want 0600", candidate, got)
		}
	}
}

func TestSaveFailsClosedOnUnsafeCheckpointHistory(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "checkpoint.json")
	first := currentCheckpoint("evolve", 1)
	if err := Save(path, first, 0); err != nil {
		t.Fatal(err)
	}
	if err := os.Mkdir(path+".1", 0o755); err != nil {
		t.Fatal(err)
	}
	err := Save(path, currentCheckpoint("evolve", 2), 2)
	if err == nil {
		t.Fatal("unsafe history entry must fail checkpoint rotation")
	}
	got, found, loadErr := Load(path)
	if loadErr != nil || !found || got.Iteration != first.Iteration {
		t.Fatalf("failed rotation changed current checkpoint: found=%v err=%v got=%+v", found, loadErr, got)
	}
}
