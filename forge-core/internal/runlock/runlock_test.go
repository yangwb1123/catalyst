package runlock

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestAcquire_Succeeds(t *testing.T) {
	root := t.TempDir()
	lock, err := Acquire(root)
	if err != nil {
		t.Fatalf("Acquire: %v", err)
	}
	if lock == nil {
		t.Fatal("Acquire returned nil lock with nil error")
	}
	want := filepath.Join(root, ".forge", "run.lock")
	if lock.Path != want {
		t.Errorf("lock.Path = %q, want %q", lock.Path, want)
	}
	if _, err := os.Stat(want); err != nil {
		t.Fatalf("lock file not created: %v", err)
	}
	if err := lock.Release(); err != nil {
		t.Errorf("Release: %v", err)
	}
}

func TestAcquire_CreatesForgeDirIfMissing(t *testing.T) {
	root := t.TempDir() // deliberately no .forge/ yet — mirrors openTracer's own MkdirAll
	lock, err := Acquire(root)
	if err != nil {
		t.Fatalf("Acquire: %v", err)
	}
	defer lock.Release()

	dir := filepath.Join(root, ".forge")
	st, err := os.Stat(dir)
	if err != nil {
		t.Fatalf("stat %s: %v", dir, err)
	}
	if !st.IsDir() {
		t.Fatalf("%s exists but is not a directory", dir)
	}
}

func TestBusyDoesNotCreateStateAndTracksHeldLock(t *testing.T) {
	root := t.TempDir()
	busy, err := Busy(root)
	if err != nil || busy {
		t.Fatalf("Busy on fresh root = %v, %v", busy, err)
	}
	if _, err := os.Stat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("Busy created .forge on a read-only probe: %v", err)
	}
	lock, err := Acquire(root)
	if err != nil {
		t.Fatal(err)
	}
	busy, err = Busy(root)
	if Supported() && (err != nil || !busy) {
		t.Fatalf("Busy while held = %v, %v", busy, err)
	}
	if err := lock.Release(); err != nil {
		t.Fatal(err)
	}
	busy, err = Busy(root)
	if err != nil || busy {
		t.Fatalf("Busy after release = %v, %v", busy, err)
	}
}

func TestAcquire_RejectsForgeDirectorySymlink(t *testing.T) {
	root := t.TempDir()
	outside := t.TempDir()
	if err := os.Symlink(outside, filepath.Join(root, ".forge")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if lock, err := Acquire(root); err == nil {
		lock.Release()
		t.Fatal(".forge directory symlink was accepted")
	}
}

func TestAcquire_RejectsRunLockSymlinkWithoutClobber(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, ".forge")
	if err := os.Mkdir(dir, 0o700); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(t.TempDir(), "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(sentinel, filepath.Join(dir, "run.lock")); err != nil {
		t.Skipf("symlink unavailable: %v", err)
	}
	if lock, err := Acquire(root); err == nil {
		lock.Release()
		t.Fatal("run.lock symlink was accepted")
	}
	data, err := os.ReadFile(sentinel)
	if err != nil || string(data) != "keep" {
		t.Fatalf("outside sentinel changed: data=%q err=%v", data, err)
	}
}

// TestAcquire_SecondAttemptFailsFast is the required concurrent-contention
// test: it proves a second Acquire on an already-held root fails
// IMMEDIATELY (never blocks/retries/waits) with an actionable error naming
// the lock path and both operator remedies. flock is tied to the open file
// description, so two separate os.OpenFile-backed opens of the same path
// within one process (exactly what the two Acquire calls below do)
// faithfully reproduce cross-process contention per flock(2) semantics.
func TestAcquire_SecondAttemptFailsFast(t *testing.T) {
	root := t.TempDir()

	first, err := Acquire(root)
	if err != nil {
		t.Fatalf("first Acquire: %v", err)
	}
	defer first.Release()

	start := time.Now()
	second, err := Acquire(root)
	elapsed := time.Since(start)

	if err == nil {
		second.Release()
		t.Fatal("second Acquire succeeded while the first still holds the lock")
	}
	if elapsed > 200*time.Millisecond {
		t.Errorf("second Acquire took %v to fail — it must fail FAST, never block/retry/wait", elapsed)
	}

	msg := err.Error()
	lockPath := filepath.Join(root, ".forge", "run.lock")
	for _, want := range []string{
		lockPath,
		"already active",
		"wait for that command to finish",
		"do not unlink a contended lock file",
	} {
		if !strings.Contains(msg, want) {
			t.Errorf("error message %q missing expected substring %q", msg, want)
		}
	}
	if strings.Contains(msg, "remove") || strings.Contains(msg, "stale") {
		t.Errorf("contention message gives unsafe stale-file advice: %q", msg)
	}
}

func TestRelease_AllowsReacquire(t *testing.T) {
	root := t.TempDir()

	first, err := Acquire(root)
	if err != nil {
		t.Fatalf("first Acquire: %v", err)
	}
	if err := first.Release(); err != nil {
		t.Fatalf("Release: %v", err)
	}

	second, err := Acquire(root)
	if err != nil {
		t.Fatalf("second Acquire after Release: %v", err)
	}
	if err := second.Release(); err != nil {
		t.Errorf("second Release: %v", err)
	}
}

func TestRelease_NilSafe(t *testing.T) {
	var lock *Lock
	if err := lock.Release(); err != nil {
		t.Errorf("(*Lock)(nil).Release() = %v, want nil", err)
	}
}

func TestRelease_Idempotent(t *testing.T) {
	root := t.TempDir()
	lock, err := Acquire(root)
	if err != nil {
		t.Fatalf("Acquire: %v", err)
	}
	if err := lock.Release(); err != nil {
		t.Fatalf("first Release: %v", err)
	}
	if err := lock.Release(); err != nil {
		t.Errorf("second Release: %v, want nil (idempotent)", err)
	}
}

func TestAcquire_DifferentRootsNoContention(t *testing.T) {
	rootA, rootB := t.TempDir(), t.TempDir()

	lockA, err := Acquire(rootA)
	if err != nil {
		t.Fatalf("Acquire rootA: %v", err)
	}
	defer lockA.Release()

	lockB, err := Acquire(rootB)
	if err != nil {
		t.Fatalf("Acquire rootB (should be independent of rootA's held lock): %v", err)
	}
	defer lockB.Release()
}
