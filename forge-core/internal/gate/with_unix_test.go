//go:build unix

package gate

import (
	"context"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

// ── T7 (R1/R2) — process-tree kill through the gate bridge ─────────────────
// A stub harness that forks a pipe-inheriting grandchild and waits on it must
// have BOTH the direct child and the grandchild reaped when the run's ctx is
// cancelled — the process-group teardown this direction adds to the gate
// bridge (mirrors the orchestrator's grandchild proof).
func TestGateWith_Deadline_CtxCancelKillsGrandchild(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	t.Setenv(EnvTimeout, "")
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	stubBinary(t, "node", "/bin/sleep 60 & echo $! > "+pidFile+"; wait")
	root := t.TempDir()

	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan struct{})
	var res Result
	go func() {
		defer close(done)
		res = GateWith(ctx, root, Options{})
	}()

	pid := readGatePID(t, pidFile, 3*time.Second)
	if pid == 0 {
		cancel()
		t.Fatal("grandchild never recorded its pid; test construction broken")
	}
	t.Cleanup(func() { killGatePID(pid) })
	cancel()
	select {
	case <-done:
	case <-time.After(10 * time.Second):
		t.Fatal("GateWith did not return after ctx cancel")
	}
	if res.Status != StatusFail || !strings.Contains(res.Output, "canceled") {
		t.Errorf("cancelled gate must FAIL with the cancel clause; got %q %q", res.Status, res.Output)
	}
	if !gateWaitGone(pid, 3*time.Second) {
		t.Errorf("grandchild pid %d still alive after ctx cancel — process-group teardown failed", pid)
	}
}

// readGatePID polls path until the grandchild records its pid (or the deadline
// passes); 0 on timeout/absence.
func readGatePID(t *testing.T, path string, within time.Duration) int {
	t.Helper()
	deadline := time.Now().Add(within)
	for time.Now().Before(deadline) {
		if b, err := os.ReadFile(path); err == nil && len(strings.TrimSpace(string(b))) > 0 {
			pid, err := strconv.Atoi(strings.TrimSpace(string(b)))
			if err == nil {
				return pid
			}
		}
		time.Sleep(10 * time.Millisecond)
	}
	return 0
}

func gateAlive(pid int) bool {
	return syscall.Kill(pid, 0) != syscall.ESRCH
}

func gateWaitGone(pid int, within time.Duration) bool {
	deadline := time.Now().Add(within)
	for time.Now().Before(deadline) {
		if !gateAlive(pid) {
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return !gateAlive(pid)
}

func killGatePID(pid int) {
	if pid > 0 {
		_ = syscall.Kill(pid, syscall.SIGKILL)
	}
}
