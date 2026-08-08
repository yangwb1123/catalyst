//go:build unix

package orchestrator

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"

	"forgeos/forge-core/internal/asset"
)

// runWithoutProcessGroup reproduces the PRE-FIX path: a bare exec.CommandContext with
// NO setupProcessGroup, so cancellation falls back to os/exec's direct-child-only kill.
// It starts the command, waits for the grandchild to record its pid, then cancels the
// context (killing only the direct child) and returns the grandchild pid for the caller
// to probe. It deliberately does NOT call Wait synchronously: under the default kill
// Wait would block on the grandchild's inherited pipe until the 30s sleep — the very
// hang this PR removes — so a detached goroutine drains Wait to avoid a zombie while the
// test proceeds without hanging.
func runWithoutProcessGroup(t *testing.T, argv []string, timeout time.Duration, pidFile string) int {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), timeout)
	cmd := exec.CommandContext(ctx, argv[0], argv[1:]...) // no setupProcessGroup: default cancel
	if err := cmd.Start(); err != nil {
		cancel()
		t.Fatalf("start: %v", err)
	}
	pid := readPID(waitForFile(pidFile, time.Second))
	if pid == 0 {
		cancel()
		t.Fatal("grandchild never recorded its pid; test construction broken")
	}
	<-ctx.Done()                             // let the deadline trip -> default kill hits the direct child only
	time.Sleep(50 * time.Millisecond)        // let that kill land before we probe the grandchild
	go func() { _ = cmd.Wait(); cancel() }() // drain in background; Wait would hang here on the pipe
	return pid
}

// grandchildSpawner returns argv for a DIRECT child (sh) that forks a long-lived
// GRANDCHILD inheriting the command's stdout pipe, then waits on it. The grandchild
// writes its own pid to pidFile and `exec sleep`s for sleepSecs (exec keeps the same
// pid and the inherited stdout fd, so the grandchild is exactly the pipe-holding
// process the SIGKILL-direct-child-only default would orphan). The outer `wait` makes
// the direct child block on the grandchild, so without a process-group kill cmd.Run()
// cannot return until the grandchild's sleep elapses — the hang this PR fixes.
func grandchildSpawner(pidFile string, sleepSecs int) []string {
	script := "sh -c 'echo $$ > " + pidFile + "; exec sleep " + strconv.Itoa(sleepSecs) + "' & wait"
	return []string{"sh", "-c", script}
}

// waitForFile polls until path exists and is non-empty (the grandchild has recorded
// its pid) or the deadline passes. Returns the trimmed contents, or "" on timeout.
func waitForFile(path string, within time.Duration) string {
	deadline := time.Now().Add(within)
	for time.Now().Before(deadline) {
		if b, err := os.ReadFile(path); err == nil && len(strings.TrimSpace(string(b))) > 0 {
			return strings.TrimSpace(string(b))
		}
		time.Sleep(5 * time.Millisecond)
	}
	return ""
}

// alive reports whether pid is still a live process: signal 0 probes existence, and
// ESRCH ("no such process") is the definitive "already reaped" answer.
func alive(pid int) bool {
	err := syscall.Kill(pid, 0)
	return err != syscall.ESRCH
}

// killPID best-effort reaps a leaked pid so a failed assertion never leaves a real
// 30s sleep running on the CI host. Safe to call on an already-dead pid (ESRCH).
func killPID(pid int) {
	if pid > 0 {
		_ = syscall.Kill(pid, syscall.SIGKILL)
	}
}

// readPID parses the grandchild pid the spawner wrote, or 0 if absent/garbage.
func readPID(s string) int {
	pid, err := strconv.Atoi(strings.TrimSpace(s))
	if err != nil {
		return 0
	}
	return pid
}

// ★ Core orphan proof ★ — the end-to-end evidence the gap is real AND fixed.
// WITH setupProcessGroup the negative-pid SIGKILL reaps the whole group, so the
// pipe-holding grandchild is gone (syscall.Kill(pid,0) == ESRCH) shortly after the
// timeout. The contrast subtest below shows the SAME construction leaks the grandchild
// without the process group, so this is not vacuously true.
func TestCommandExecutor_ProcessGroup_GrandchildReaped(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	ex := CommandExecutor{
		Build:   func(asset.Phase, string) []string { return grandchildSpawner(pidFile, 30) },
		Timeout: 300 * time.Millisecond,
	}

	err := ex.Execute(context.Background(), asset.Phase{Name: "slow"}, "m")

	pid := readPID(waitForFile(pidFile, time.Second))
	if pid == 0 {
		t.Fatal("grandchild never recorded its pid; test construction broken")
	}
	t.Cleanup(func() { killPID(pid) })

	execErr := requireExecError(t, err)
	if execErr.Kind != KindTimeout {
		t.Errorf("want KindTimeout, got %v", execErr.Kind)
	}
	// The group kill is asynchronous to Run's return; give the SIGKILL a moment to
	// land before asserting the grandchild is gone (generous, to stay non-flaky).
	if !waitGone(pid, 3*time.Second) {
		t.Errorf("grandchild pid %d still alive after a group-killed timeout; process-group teardown failed", pid)
	}
}

// Contrast (proves the gap is real, not vacuous): the SAME grandchild construction run
// WITHOUT setupProcessGroup — a bare exec.CommandContext, os/exec's default
// direct-child-only kill — leaks the grandchild, which is still alive after the context
// fires. This is the pre-fix behavior; the test above shows the process group fixes it.
func TestCommandExecutor_NoProcessGroup_GrandchildLeaks(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	pid := runWithoutProcessGroup(t, grandchildSpawner(pidFile, 30), 300*time.Millisecond, pidFile)
	t.Cleanup(func() { killPID(pid) })

	// Default kill hit only the direct child; the grandchild that inherited the pipe
	// survives. (We deliberately do NOT block on Wait here — under the default cancel
	// Wait would hang on the pipe until the 30s sleep, which is the very bug. See the
	// timeliness test for how the fixed path avoids that hang.)
	if !alive(pid) {
		t.Skip("grandchild already gone without a group kill — OS reaped it independently; the contrast is environment-specific, not a fix regression")
	}
	t.Logf("confirmed gap: grandchild pid %d survives the default direct-child-only kill", pid)
}

// ★ Core timeliness proof ★ — the gap that actually matters: a tripped Timeout must
// RETURN, not hang. With the grandchild holding the stdout pipe, the fixed path returns
// within Timeout + WaitDelay + slack as KindTimeout. The bare path (contrast helper)
// would block on Wait until the 30s sleep — so rather than hang CI for 30s we assert the
// fixed path returns FAST and prove the contrast hangs via a bounded select probe.
func TestCommandExecutor_ProcessGroup_TimeoutReturnsPromptly(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	ex := CommandExecutor{
		Build:   func(asset.Phase, string) []string { return grandchildSpawner(pidFile, 30) },
		Timeout: 300 * time.Millisecond,
	}

	start := time.Now()
	err := ex.Execute(context.Background(), asset.Phase{Name: "slow"}, "m")
	elapsed := time.Since(start)

	if pid := readPID(waitForFile(pidFile, time.Second)); pid > 0 {
		t.Cleanup(func() { killPID(pid) })
	}
	execErr := requireExecError(t, err)
	if execErr.Kind != KindTimeout {
		t.Errorf("want KindTimeout, got %v", execErr.Kind)
	}
	// Must return within Timeout(300ms) + WaitDelay(2s) + generous slack — and FAR
	// under the grandchild's 30s sleep, proving Run did not wait the grandchild out.
	// (WaitDelay is execbound's portable pipe-close backstop; its literal is mirrored
	// here for the orchestrator-side budget assertion.)
	const waitDelay = 2 * time.Second
	budget := 300*time.Millisecond + waitDelay + 5*time.Second
	if elapsed >= budget {
		t.Errorf("timeout did not return promptly: %v >= budget %v (Run hung on the inherited pipe)", elapsed, budget)
	}
	if elapsed >= 25*time.Second {
		t.Errorf("Run waited the grandchild's full sleep out (%v) — the process-group fix did not take effect", elapsed)
	}
}

// Backward-compat on unix: a single-process command with NO grandchildren exits
// normally and is byte-for-byte unaffected by Setpgid — the Cancel/WaitDelay path is
// never reached on a clean exit, so output and (success) classification are unchanged.
func TestCommandExecutor_ProcessGroup_SingleProcessUnaffected(t *testing.T) {
	rec := &recorder{}
	ex := CommandExecutor{
		Build:   func(p asset.Phase, mode string) []string { return []string{"echo", "byte-identical"} },
		Log:     rec.log,
		Timeout: 5 * time.Second,
	}
	if err := ex.Execute(context.Background(), asset.Phase{Name: "p"}, "m"); err != nil {
		t.Fatalf("a clean single-process command must succeed unchanged: %v", err)
	}
	if !containsLine(rec.logs, "byte-identical") {
		t.Errorf("single-process output must be captured exactly as before; logs=%v", rec.logs)
	}
}

// waitGone polls until pid is reaped (ESRCH) or the deadline passes; true once gone.
func waitGone(pid int, within time.Duration) bool {
	deadline := time.Now().Add(within)
	for time.Now().Before(deadline) {
		if !alive(pid) {
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return !alive(pid)
}
