//go:build unix

package runlock

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

// TestMain intercepts a re-exec of this test binary when
// GO_WANT_RUNLOCK_HELPER=1 is set, running the acquire-and-block helper
// instead of the normal test suite — the standard Go "helper subprocess"
// pattern (re-exec os.Args[0] with a gating env var; cf. os/exec's own
// tests). TestAcquire_ProcessDeathReleasesLock below spawns exactly this.
func TestMain(m *testing.M) {
	if os.Getenv("GO_WANT_RUNLOCK_HELPER") == "1" {
		runAcquireAndBlockHelper()
		return
	}
	os.Exit(m.Run())
}

// runAcquireAndBlockHelper acquires the lock on RUNLOCK_TEST_ROOT, signals
// readiness by writing a marker file, then blocks (via a long real sleep,
// not select{}, so the Go runtime's deadlock detector never fires) until
// the parent test SIGKILLs this process.
func runAcquireAndBlockHelper() {
	root := os.Getenv("RUNLOCK_TEST_ROOT")
	if _, err := Acquire(root); err != nil {
		fmt.Fprintln(os.Stderr, "runlock helper: Acquire failed:", err)
		os.Exit(1)
	}
	readyPath := filepath.Join(root, ".forge", "helper.ready")
	if err := os.WriteFile(readyPath, []byte("ready\n"), 0o644); err != nil {
		fmt.Fprintln(os.Stderr, "runlock helper: signal readiness failed:", err)
		os.Exit(1)
	}
	time.Sleep(time.Hour) // outlives any sane test timeout; the parent kills us first
}

// waitForFile polls for path to exist, up to timeout. Returns false on timeout.
func waitForFile(path string, timeout time.Duration) bool {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		if _, err := os.Stat(path); err == nil {
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return false
}

// TestAcquire_ProcessDeathReleasesLock spawns a REAL child process that
// acquires run.lock and is then SIGKILLed (not gracefully exited), then
// verifies a fresh Acquire on the same root succeeds promptly afterward.
// This directly demonstrates — rather than leaving as an unverified
// comment — that the "if stale from a crash" escape hatch in Acquire's
// error message is (mostly) unnecessary in practice: flock is tied to the
// OPEN FILE DESCRIPTION, so the kernel releases it automatically on ANY
// holder process death, including SIGKILL.
func TestAcquire_ProcessDeathReleasesLock(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns a real subprocess; skipped under -short")
	}
	root := t.TempDir()

	cmd := exec.Command(os.Args[0])
	cmd.Env = append(os.Environ(),
		"GO_WANT_RUNLOCK_HELPER=1",
		"RUNLOCK_TEST_ROOT="+root,
	)
	if err := cmd.Start(); err != nil {
		t.Fatalf("start helper subprocess: %v", err)
	}
	defer func() {
		if cmd.Process != nil {
			cmd.Process.Kill()
			cmd.Wait()
		}
	}()

	readyPath := filepath.Join(root, ".forge", "helper.ready")
	if !waitForFile(readyPath, 3*time.Second) {
		t.Fatal("helper never signaled that it acquired the lock")
	}

	// Sanity: confirm real contention while the helper is alive and holding
	// the lock — otherwise the death-releases-it assertion below would be
	// vacuously true (lock never actually contended in the first place).
	if held, err := Acquire(root); err == nil {
		held.Release()
		t.Fatal("Acquire succeeded while the helper still holds the lock — test construction broken")
	}

	if err := cmd.Process.Kill(); err != nil { // SIGKILL: not a graceful exit
		t.Fatalf("SIGKILL helper: %v", err)
	}
	cmd.Wait()

	lock, err := acquireWithRetry(root, 2*time.Second)
	if err != nil {
		t.Fatalf("Acquire after helper SIGKILL: %v (kernel should release the flock on any process death)", err)
	}
	lock.Release()
}

// acquireWithRetry polls Acquire until it succeeds or timeout elapses. The
// kernel releases a crashed holder's flock immediately, but scheduling a
// SIGKILLed process's exit isn't instantaneous, so a short poll (rather than
// a single immediate Acquire) avoids a flaky race in the test above.
func acquireWithRetry(root string, timeout time.Duration) (*Lock, error) {
	deadline := time.Now().Add(timeout)
	var lock *Lock
	var err error
	for {
		lock, err = Acquire(root)
		if err == nil || time.Now().After(deadline) {
			return lock, err
		}
		time.Sleep(20 * time.Millisecond)
	}
}
