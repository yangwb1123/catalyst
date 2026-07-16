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
	for _, want := range []string{lockPath, "already active", "wait for it to finish", "stale from a crash"} {
		if !strings.Contains(msg, want) {
			t.Errorf("error message %q missing expected substring %q", msg, want)
		}
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
