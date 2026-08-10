//go:build unix

package execbound

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

func TestRunObserved_UnrequestedSignalIsNotExitCode(t *testing.T) {
	result := RunObserved(context.Background(), []string{"sh", "-c", "kill -TERM $$"},
		Options{}, CaptureCombined, Spec{}, ObservationOptions{})
	if result.Legacy.Err == nil {
		t.Fatal("signal termination must retain a legacy run error")
	}
	if result.Execution.Termination != TerminationSignaled || result.Execution.ExitCode != nil ||
		result.Execution.SignalNumber == nil || *result.Execution.SignalNumber != uint32(syscall.SIGTERM) ||
		result.Execution.SignalName == "" {
		t.Errorf("signal termination = %+v", result.Execution)
	}
}

func TestRunObserved_PermissionDeniedExecutableIsSpawnFailure(t *testing.T) {
	path := filepath.Join(t.TempDir(), "not-executable")
	if err := os.WriteFile(path, []byte("#!/bin/sh\nexit 0\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	result := RunObserved(context.Background(), []string{"logical-tool"}, Options{},
		CaptureCombined, Spec{ExecutablePath: path}, ObservationOptions{})
	if result.Execution.Started || result.Execution.Termination != TerminationSpawnFailed ||
		result.Legacy.Err == nil {
		t.Errorf("permission-denied spawn result = %+v", result)
	}
}

func TestRunObserved_WaitDelayMarksDrainIncomplete(t *testing.T) {
	if testing.Short() {
		t.Skip("waits for the real WaitDelay backstop")
	}
	pidFile := filepath.Join(t.TempDir(), "inherited-pipe.pid")
	script := "sleep 30 & echo $! > " + pidFile + "; exit 0"
	start := time.Now()
	result := RunObserved(context.Background(), []string{"sh", "-c", script},
		Options{}, CaptureCombined, Spec{}, ObservationOptions{})
	elapsed := time.Since(start)

	pid := readObservedPID(t, pidFile)
	if pid > 0 {
		t.Cleanup(func() { _ = syscall.Kill(pid, syscall.SIGKILL) })
	}
	if !errors.Is(result.Legacy.Err, exec.ErrWaitDelay) {
		t.Fatalf("wait error = %v, want exec.ErrWaitDelay", result.Legacy.Err)
	}
	if result.Execution.DrainComplete || result.Execution.Termination != TerminationWaitFailed ||
		!result.Execution.Started {
		t.Errorf("WaitDelay observation = %+v", result.Execution)
	}
	if elapsed < waitDelay || elapsed > waitDelay+5*time.Second {
		t.Errorf("WaitDelay elapsed = %v, want around %v", elapsed, waitDelay)
	}
}

func TestRunObserved_AbnormalExitCannotHideIncompleteDrain(t *testing.T) {
	if testing.Short() {
		t.Skip("waits for the real drain backstop")
	}
	tests := []struct {
		name        string
		termination TerminationKind
		script      string
	}{
		{name: "nonzero", termination: TerminationExited, script: "printf before; sleep 30 & echo $! > %s; exit 7"},
		{name: "signal", termination: TerminationSignaled, script: "printf before; sleep 30 & echo $! > %s; kill -TERM $$"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			pidFile := filepath.Join(t.TempDir(), "inherited-pipe.pid")
			result := RunObserved(context.Background(),
				[]string{"sh", "-c", fmt.Sprintf(test.script, pidFile)},
				Options{}, CaptureCombined, Spec{}, ObservationOptions{})
			pid := readObservedPID(t, pidFile)
			if pid > 0 {
				t.Cleanup(func() { _ = syscall.Kill(pid, syscall.SIGKILL) })
			}
			if result.Execution.DrainComplete || result.Execution.Termination != test.termination {
				t.Fatalf("abnormal exit hid incomplete drain: %+v", result.Execution)
			}
			assertStreamObservation(t, result.Stdout, "before", "before")
			if test.termination == TerminationExited &&
				(result.Execution.ExitCode == nil || *result.Execution.ExitCode != 7) {
				t.Fatalf("nonzero exit identity lost: %+v", result.Execution)
			}
		})
	}
}

func TestRunObserved_EscapedGrandchildFailsDrainClosedWithinBound(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns an escaped real grandchild and waits for the drain backstop")
	}
	setsid, err := exec.LookPath("setsid")
	if err != nil {
		t.Skip("setsid unavailable")
	}
	pidFile := filepath.Join(t.TempDir(), "escaped.pid")
	script := fmt.Sprintf("%s sleep 30 & echo $! > %s; wait", setsid, pidFile)
	started := time.Now()
	result := RunObserved(context.Background(), []string{"sh", "-c", script},
		Options{Timeout: 200 * time.Millisecond}, CaptureCombined, Spec{}, ObservationOptions{})
	elapsed := time.Since(started)
	pid := readObservedPID(t, pidFile)
	if pid > 0 {
		t.Cleanup(func() { _ = syscall.Kill(pid, syscall.SIGKILL) })
	}
	if result.Execution.Termination != TerminationTimedOut || result.Execution.DrainComplete {
		t.Fatalf("escaped writer must be a non-producible timeout: %+v", result.Execution)
	}
	if elapsed > 200*time.Millisecond+waitDelay+3*time.Second {
		t.Fatalf("escaped writer exceeded bounded return: %v", elapsed)
	}
}

func TestRunObserved_OneSidedInheritedPipeIsIncomplete(t *testing.T) {
	if testing.Short() {
		t.Skip("waits for the real drain backstop")
	}
	for _, redirect := range []string{"2>/dev/null", ">/dev/null"} {
		t.Run(redirect, func(t *testing.T) {
			pidFile := filepath.Join(t.TempDir(), "one-sided.pid")
			script := fmt.Sprintf("sleep 30 %s & echo $! > %s; exit 0", redirect, pidFile)
			result := RunObserved(context.Background(), []string{"sh", "-c", script},
				Options{}, CaptureCombined, Spec{}, ObservationOptions{})
			pid := readObservedPID(t, pidFile)
			if pid > 0 {
				t.Cleanup(func() { _ = syscall.Kill(pid, syscall.SIGKILL) })
			}
			if result.Execution.DrainComplete || result.Execution.Termination != TerminationWaitFailed ||
				!errors.Is(result.Legacy.Err, exec.ErrWaitDelay) {
				t.Fatalf("one inherited stream must fail closed: %+v err=%v", result.Execution, result.Legacy.Err)
			}
		})
	}
}

func TestRunObserved_TimeoutReapsGrandchild(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns a real grandchild")
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

func readObservedPID(t *testing.T, path string) int {
	t.Helper()
	deadline := time.Now().Add(time.Second)
	for time.Now().Before(deadline) {
		content, err := os.ReadFile(path)
		if err == nil {
			pid, parseErr := strconv.Atoi(strings.TrimSpace(string(content)))
			if parseErr == nil {
				return pid
			}
		}
		time.Sleep(5 * time.Millisecond)
	}
	return 0
}
