package runlock

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

type lockFileSnapshot struct {
	data  []byte
	mode  os.FileMode
	mtime time.Time
}

func TestBusyIdleProbePreservesLockBytesModeAndMtime(t *testing.T) {
	if !Supported() {
		t.Skip("host has no real contention probe")
	}
	root := t.TempDir()
	dir := filepath.Join(root, ".forge")
	if err := os.Mkdir(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, "run.lock")
	if err := os.WriteFile(path, []byte("operator-owned metadata\n"), 0o640); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatal(err)
	}
	stamp := time.Unix(1_690_000_000, 456_000_000)
	if err := os.Chtimes(path, stamp, stamp); err != nil {
		t.Fatal(err)
	}
	before := snapshotLockFile(t, path)

	busy, err := Busy(root)
	if err != nil || busy {
		t.Fatalf("Busy idle lock = %v, %v; want false, nil", busy, err)
	}
	assertLockFileSnapshot(t, path, before)
}

func TestBusyContendedProbePreservesLockBytesModeAndMtime(t *testing.T) {
	if !Supported() {
		t.Skip("host has no real contention probe")
	}
	root := t.TempDir()
	lock, err := Acquire(root)
	if err != nil {
		t.Fatalf("Acquire: %v", err)
	}
	defer lock.Release()
	before := snapshotLockFile(t, lock.Path)

	busy, err := Busy(root)
	if err != nil || !busy {
		t.Fatalf("Busy held lock = %v, %v; want true, nil", busy, err)
	}
	assertLockFileSnapshot(t, lock.Path, before)
}

func snapshotLockFile(t *testing.T, path string) lockFileSnapshot {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	return lockFileSnapshot{
		data: append([]byte(nil), data...),
		mode: info.Mode().Perm(), mtime: info.ModTime(),
	}
}

func assertLockFileSnapshot(t *testing.T, path string, want lockFileSnapshot) {
	t.Helper()
	got := snapshotLockFile(t, path)
	if string(got.data) != string(want.data) ||
		got.mode != want.mode ||
		!got.mtime.Equal(want.mtime) {
		t.Fatalf("Busy mutated lock: data=%q/%q mode=%#o/%#o mtime=%s/%s",
			got.data, want.data, got.mode, want.mode, got.mtime, want.mtime)
	}
}
