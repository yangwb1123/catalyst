//go:build unix

package execbound

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
)

// grandchildSpawner returns argv for a DIRECT child (sh) that forks a
// long-lived GRANDCHILD inheriting the command's stdout pipe, then waits on
// it. The grandchild writes its own pid to pidFile and `exec sleep`s for
// sleepSecs (exec keeps the same pid and the inherited stdout fd, so the
// grandchild is exactly the pipe-holding process the SIGKILL-direct-child-only
// default would orphan). The outer `wait` makes the direct child block on the
// grandchild, so without a process-group kill cmd.Run() cannot return until
// the grandchild's sleep elapses — the hang this package fixes.
func grandchildSpawner(pidFile string, sleepSecs int) []string {
	script := "sh -c 'echo $$ > " + pidFile + "; exec sleep " + strconv.Itoa(sleepSecs) + "' & wait"
	return []string{"sh", "-c", script}
}

func readPIDFile(t *testing.T, path string, within time.Duration) int {
	t.Helper()
	deadline := time.Now().Add(within)
	for time.Now().Before(deadline) {
		if b, err := os.ReadFile(path); err == nil && len(strings.TrimSpace(string(b))) > 0 {
			pid, err := strconv.Atoi(strings.TrimSpace(string(b)))
			if err == nil {
				return pid
			}
		}
		time.Sleep(5 * time.Millisecond)
	}
	return 0
}

func processAlive(pid int) bool {
	return syscall.Kill(pid, 0) != syscall.ESRCH
}

func killPID(pid int) {
	if pid > 0 {
		_ = syscall.Kill(pid, syscall.SIGKILL)
	}
}

func waitGone(pid int, within time.Duration) bool {
	deadline := time.Now().Add(within)
	for time.Now().Before(deadline) {
		if !processAlive(pid) {
			return true
		}
		time.Sleep(10 * time.Millisecond)
	}
	return !processAlive(pid)
}

// setupProcessGroup unit: on unix it must wire all three mechanisms — a new
// process group (Setpgid), a non-default group-kill Cancel, and a positive
// WaitDelay backstop (moved here from the orchestrator's test suite with the
// extracted machinery).
func TestSetupProcessGroup_WiresGroupKillAndWaitDelay(t *testing.T) {
	cmd := exec.Command("sh", "-c", "true")
	setupProcessGroup(cmd)

	if cmd.SysProcAttr == nil || !cmd.SysProcAttr.Setpgid {
		t.Error("must set SysProcAttr.Setpgid=true so the child leads a new process group")
	}
	if cmd.Cancel == nil {
		t.Error("must install a Cancel that group-kills (overriding os/exec's direct-child-only default)")
	}
	if cmd.WaitDelay <= 0 {
		t.Errorf("must set a positive WaitDelay backstop; got %v", cmd.WaitDelay)
	}
	if cmd.WaitDelay != waitDelay {
		t.Errorf("WaitDelay = %v, want the documented grace %v", cmd.WaitDelay, waitDelay)
	}
	if !GroupKillAvailable() {
		t.Error("GroupKillAvailable must be true on unix")
	}
}

// ★ Core orphan proof ★ — the negative-pid SIGKILL reaps the whole group, so
// the pipe-holding grandchild is gone (syscall.Kill(pid,0) == ESRCH) shortly
// after the deadline.
func TestExecbound_ProcessGroup_GrandchildReaped(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	res := Run(context.Background(), grandchildSpawner(pidFile, 30),
		Options{Timeout: 300 * time.Millisecond}, CaptureCombined, Spec{})

	pid := readPIDFile(t, pidFile, time.Second)
	if pid == 0 {
		t.Fatal("grandchild never recorded its pid; test construction broken")
	}
	t.Cleanup(func() { killPID(pid) })

	if !res.TimedOut() {
		t.Fatalf("must report TimedOut; CtxErr=%v", res.CtxErr)
	}
	if !waitGone(pid, 3*time.Second) {
		t.Errorf("grandchild pid %d still alive after a group-killed timeout; process-group teardown failed", pid)
	}
}

// ★ Core timeliness proof (T11) ★ — a tripped deadline must RETURN, not hang:
// with the grandchild holding the stdout pipe, Run returns within deadline +
// WaitDelay + slack. The WaitDelay backstop is what guarantees this even if
// the group kill races a just-forked grandchild.
func TestExecbound_WaitDelay_Backstop_Unix(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	pidFile := filepath.Join(t.TempDir(), "grandchild.pid")
	start := time.Now()
	res := Run(context.Background(), grandchildSpawner(pidFile, 30),
		Options{Timeout: 300 * time.Millisecond}, CaptureCombined, Spec{})
	elapsed := time.Since(start)

	if pid := readPIDFile(t, pidFile, time.Second); pid > 0 {
		t.Cleanup(func() { killPID(pid) })
	}
	if !res.TimedOut() {
		t.Fatalf("must report TimedOut; CtxErr=%v Err=%v", res.CtxErr, res.Err)
	}
	// Must return within Timeout(300ms) + WaitDelay(2s) + generous slack — and
	// FAR under the grandchild's 30s sleep, proving Run did not wait it out.
	budget := 300*time.Millisecond + waitDelay + 5*time.Second
	if elapsed >= budget {
		t.Errorf("timeout did not return promptly: %v >= budget %v (Run hung on the inherited pipe)", elapsed, budget)
	}
	if elapsed >= 25*time.Second {
		t.Errorf("Run waited the grandchild's full sleep out (%v) — the process-group fix did not take effect", elapsed)
	}
}

// On unix no degradation Log line is emitted on the kill path (group teardown
// IS available): the Log sink stays silent even when the deadline fires.
func TestExecbound_KillPath_NoDegradationLogOnUnix(t *testing.T) {
	var logs []string
	res := Run(context.Background(), []string{"sleep", "30"},
		Options{Timeout: 300 * time.Millisecond, Log: func(s string) { logs = append(logs, s) }},
		CaptureCombined, Spec{})
	if !res.TimedOut() {
		t.Fatalf("must report TimedOut; CtxErr=%v", res.CtxErr)
	}
	if len(logs) != 0 {
		t.Errorf("unix kill path must not emit a degradation log; got %v", logs)
	}
}
