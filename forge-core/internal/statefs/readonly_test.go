package statefs

import (
	"io"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestSideEffectFreeStateReadsPreserveBytesModeAndMtime(t *testing.T) {
	operations := []struct {
		name string
		read func(string) ([]byte, error)
	}{
		{"OpenRegularReadOnly", readThroughOpenRegularReadOnly},
		{"ReadRegularUnmodified", func(path string) ([]byte, error) {
			data, present, err := ReadRegularUnmodified(path, 1024)
			if err == nil && !present {
				return nil, os.ErrNotExist
			}
			return data, err
		}},
	}
	for _, operation := range operations {
		t.Run(operation.name, func(t *testing.T) {
			testSideEffectFreeStateRead(t, operation.read)
		})
	}
}

func readThroughOpenRegularReadOnly(path string) ([]byte, error) {
	file, err := OpenRegularReadOnly(path)
	if err != nil {
		return nil, err
	}
	defer func() { _ = file.Close() }()
	return io.ReadAll(file)
}

func testSideEffectFreeStateRead(
	t *testing.T,
	read func(string) ([]byte, error),
) {
	t.Helper()
	dir := filepath.Join(t.TempDir(), ".forge")
	if err := EnsurePrivateDir(dir); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "state.json")
	want := []byte("{\"state\":\"unchanged\"}\n")
	if err := os.WriteFile(path, want, 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatal(err)
	}
	stamp := time.Unix(1_700_000_000, 123_000_000)
	if err := os.Chtimes(path, stamp, stamp); err != nil {
		t.Fatal(err)
	}
	before, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}

	got, err := read(path)
	if err != nil {
		t.Fatalf("side-effect-free read: %v", err)
	}
	after, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != string(want) ||
		after.Mode().Perm() != before.Mode().Perm() ||
		!after.ModTime().Equal(before.ModTime()) {
		t.Fatalf("read mutated state: data=%q/%q mode=%#o/%#o mtime=%s/%s",
			got, want, after.Mode().Perm(), before.Mode().Perm(),
			after.ModTime(), before.ModTime())
	}
}
