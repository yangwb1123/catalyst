//go:build linux

package execbound

import (
	"context"
	"errors"
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

// setupProcessGroup unit: on Linux with waitid(WNOWAIT), it must wire a new
// process group and a positive WaitDelay. Cancellation is owned by
// boundedCommand so its lifecycle lock can serialize group kill and reap.
func TestSetupProcessGroup_WiresGroupKillAndWaitDelay(t *testing.T) {
	cmd := exec.Command("sh", "-c", "true")
	lifecycle := setupProcessGroup(cmd)
	if !groupKillSupported() {
		t.Skip("waitid(WNOWAIT) unavailable; direct-child fallback is active")
	}

	if cmd.SysProcAttr == nil || !cmd.SysProcAttr.Setpgid {
		t.Error("must set SysProcAttr.Setpgid=true so the child leads a new process group")
	}
	if cmd.Cancel != nil {
		t.Error("os/exec Cancel must remain nil; boundedCommand owns safe cancellation")
	}
	if cmd.WaitDelay <= 0 {
		t.Errorf("must set a positive WaitDelay backstop; got %v", cmd.WaitDelay)
	}
	if cmd.WaitDelay != waitDelay {
		t.Errorf("WaitDelay = %v, want the documented grace %v", cmd.WaitDelay, waitDelay)
	}
	if !groupKillSupported() {
		t.Error("race-free Linux group kill must be available")
	}
	if lifecycle == nil || !lifecycle.groupKill {
		t.Error("setup must retain a group-kill lifecycle")
	}
}

func TestSetupProcessGroupNormalizesMissingProcess(t *testing.T) {
	command := exec.Command("sh", "-c", "true")
	lifecycle := setupProcessGroup(command)
	command.Process = &os.Process{Pid: 1 << 30}
	if err := lifecycle.cancel(command); !errors.Is(err, os.ErrProcessDone) {
		t.Fatalf("missing process cancellation = %v", err)
	}
}

func TestExecboundDoesNotGroupKillAfterReap(t *testing.T) {
	if !groupKillSupported() {
		t.Skip("waitid(WNOWAIT) unavailable; direct-child fallback is active")
	}
	originalWaitHook := afterLinuxWaitidLocked
	originalCancelHook := beforeLinuxGroupCancel
	originalSignal := signalLinuxProcessGroup
	reapLocked := make(chan struct{})
	releaseReap := make(chan struct{})
	cancelEntered := make(chan struct{})
	afterLinuxWaitidLocked = func() { close(reapLocked); <-releaseReap }
	beforeLinuxGroupCancel = func() { close(cancelEntered) }
	signalCalls := 0
	signalLinuxProcessGroup = func(pid int) error {
		signalCalls++
		return originalSignal(pid)
	}
	t.Cleanup(func() {
		afterLinuxWaitidLocked = originalWaitHook
		beforeLinuxGroupCancel = originalCancelHook
		signalLinuxProcessGroup = originalSignal
	})

	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan Result, 1)
	go func() {
		done <- Run(ctx, []string{"true"}, Options{Unbounded: true}, CaptureCombined, Spec{})
	}()
	<-reapLocked
	cancel()
	<-cancelEntered
	close(releaseReap)
	result := <-done
	if signalCalls != 0 {
		t.Fatalf("group signal ran after reap began: calls=%d", signalCalls)
	}
	if !errors.Is(result.CtxErr, context.Canceled) {
		t.Fatalf("raced cancellation result = %+v", result)
	}
}

// ★ Core orphan proof ★ — the negative-pid SIGKILL reaps the whole group, so
// the pipe-holding grandchild is gone (syscall.Kill(pid,0) == ESRCH) shortly
// after the deadline.
func TestExecbound_ProcessGroup_GrandchildReaped(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	if !groupKillSupported() {
		t.Skip("waitid(WNOWAIT) unavailable; direct-child fallback is active")
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

func TestRunObserved_TimeoutReapsGrandchild(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns a real grandchild")
	}
	if !groupKillSupported() {
		t.Skip("waitid(WNOWAIT) unavailable; direct-child fallback is active")
	}
	pidFile := filepath.Join(t.TempDir(), "observed-grandchild.pid")
	result := RunObserved(context.Background(), grandchildSpawner(pidFile, 30),
		Options{Timeout: 300 * time.Millisecond}, CaptureCombined, Spec{}, ObservationOptions{})
	pid := readPIDFile(t, pidFile, time.Second)
	if pid == 0 {
		t.Fatal("grandchild did not record its pid")
	}
	t.Cleanup(func() { killPID(pid) })
	if result.Execution.Termination != TerminationTimedOut || !result.Execution.DrainComplete {
		t.Errorf("timeout observation = %+v", result.Execution)
	}
	if !waitGone(pid, 3*time.Second) {
		t.Errorf("grandchild pid %d survived observed timeout", pid)
	}
}

// ★ Core timeliness proof (T11) ★ — a tripped deadline must RETURN, not hang:
// with the grandchild holding the stdout pipe, Run returns within deadline +
// WaitDelay + slack. The WaitDelay backstop is what guarantees this even if
// the group kill races a just-forked grandchild.
func TestExecbound_WaitDelay_Backstop_Linux(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns real grandchild processes; skipped under -short")
	}
	if !groupKillSupported() {
		t.Skip("waitid(WNOWAIT) unavailable; direct-child fallback is active")
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

// On supported Linux hosts no degradation Log line is emitted on the kill
// path: the Log sink stays silent even when the deadline fires.
func TestExecbound_KillPath_NoDegradationLogOnLinux(t *testing.T) {
	if !groupKillSupported() {
		t.Skip("waitid(WNOWAIT) unavailable; direct-child fallback is active")
	}
	var logs []string
	res := Run(context.Background(), []string{"sleep", "30"},
		Options{Timeout: 300 * time.Millisecond, Log: func(s string) { logs = append(logs, s) }},
		CaptureCombined, Spec{})
	if !res.TimedOut() {
		t.Fatalf("must report TimedOut; CtxErr=%v", res.CtxErr)
	}
	if len(logs) != 0 {
		t.Errorf("Linux group-kill path must not emit a degradation log; got %v", logs)
	}
}

func TestExecbound_IncompleteDrainWarnsEvenOnLinux(t *testing.T) {
	if testing.Short() {
		t.Skip("waits for the real WaitDelay backstop")
	}
	setsid, err := exec.LookPath("setsid")
	if err != nil {
		t.Skip("setsid unavailable")
	}
	pidFile := filepath.Join(t.TempDir(), "escaped.pid")
	logs := []string{}
	result := Run(context.Background(), []string{"sh", "-c",
		setsid + " sleep 30 & echo $! > " + pidFile + "; exit 0"},
		Options{Log: func(value string) { logs = append(logs, value) }},
		CaptureCombined, Spec{})
	if pid := readPIDFile(t, pidFile, time.Second); pid > 0 {
		t.Cleanup(func() { killPID(pid) })
	}
	if !errors.Is(result.Err, exec.ErrWaitDelay) || !result.DrainIncomplete {
		t.Fatalf("incomplete drain result = %+v", result)
	}
	if len(logs) != 1 || !strings.Contains(logs[0], "output drain incomplete") {
		t.Fatalf("incomplete drain warning = %v", logs)
	}
}

func TestExecbound_AbnormalExitCannotHideIncompleteDrain(t *testing.T) {
	if testing.Short() {
		t.Skip("waits for the real WaitDelay backstop")
	}
	pidFile := filepath.Join(t.TempDir(), "survivor.pid")
	logs := []string{}
	result := Run(context.Background(), []string{"sh", "-c",
		"sleep 30 & echo $! > " + pidFile + "; exit 7"},
		Options{Log: func(value string) { logs = append(logs, value) }},
		CaptureCombined, Spec{})
	if pid := readPIDFile(t, pidFile, time.Second); pid > 0 {
		t.Cleanup(func() { killPID(pid) })
	}
	var exitError *exec.ExitError
	if !errors.As(result.Err, &exitError) || !result.DrainIncomplete {
		t.Fatalf("abnormal incomplete drain result = %+v", result)
	}
	if len(logs) != 1 || !strings.Contains(logs[0], "output drain incomplete") {
		t.Fatalf("abnormal incomplete drain warning = %v", logs)
	}
}
