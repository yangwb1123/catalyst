//go:build !linux

package execbound

import (
	"context"
	"errors"
	"os"
	"os/exec"
	"strings"
	"testing"
	"time"
)

const nonLinuxHelperMode = "FORGE_EXECBOUND_NONLINUX_HELPER"

func TestExecbound_NonLinuxHelper(t *testing.T) {
	switch os.Getenv(nonLinuxHelperMode) {
	case "":
		return
	case "clean":
		return
	case "sleep":
		time.Sleep(30 * time.Second)
	case "hold":
		time.Sleep(6 * time.Second)
	case "spawn":
		command := exec.Command(os.Args[0], "-test.run=^TestExecbound_NonLinuxHelper$")
		command.Env = helperEnvironment("hold")
		command.Stdout, command.Stderr = os.Stdout, os.Stderr
		if err := command.Start(); err != nil {
			os.Exit(3)
		}
		time.Sleep(30 * time.Second)
	default:
		os.Exit(2)
	}
}

func TestSetupProcessGroupSetsWaitDelayOnNonLinux(t *testing.T) {
	command := exec.Command("placeholder")
	setupProcessGroup(command)
	if command.WaitDelay != waitDelay || command.WaitDelay <= 0 {
		t.Fatalf("WaitDelay = %v, want %v", command.WaitDelay, waitDelay)
	}
}

// On non-Linux targets, group teardown is unavailable by construction: the capability
// query reports false, and the kill path emits the honest degradation Log line
// (only when a kill event actually fired — never on the happy path).
func TestExecbound_GroupKillAvailable_NonLinux(t *testing.T) {
	if groupKillSupported() {
		t.Error("GroupKillAvailable must be false on non-Linux targets")
	}
	var logs []string
	opts := Options{Timeout: 300 * time.Millisecond, Log: func(s string) { logs = append(logs, s) }}
	res := Run(context.Background(), helperArgv(), opts, CaptureCombined, helperSpec("sleep"))
	if !res.TimedOut() {
		t.Fatalf("must report TimedOut; CtxErr=%v", res.CtxErr)
	}
	if len(logs) != 1 || !strings.Contains(logs[0], "process-group teardown unavailable") {
		t.Errorf("kill path must emit exactly one degradation log line; got %v", logs)
	}
	// Happy path: a clean exit must not log.
	logs = nil
	ok := Run(context.Background(), helperArgv(), Options{
		Log: func(s string) { logs = append(logs, s) },
	}, CaptureCombined, helperSpec("clean"))
	if ok.Err != nil {
		t.Fatalf("clean exit: %v", ok.Err)
	}
	if len(logs) != 0 {
		t.Errorf("clean exit must not log degradation; got %v", logs)
	}
}

func TestExecbound_ParentCancellationLogDoesNotClaimTimeout(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	timer := time.AfterFunc(100*time.Millisecond, cancel)
	defer timer.Stop()
	logs := []string{}
	result := Run(ctx, helperArgv(), Options{
		Unbounded: true, Log: func(value string) { logs = append(logs, value) },
	}, CaptureCombined, helperSpec("sleep"))
	if result.Err == nil || !errors.Is(result.CtxErr, context.Canceled) {
		t.Fatalf("parent cancellation result = %+v", result)
	}
	if len(logs) != 1 || !strings.Contains(logs[0], "cancelled command") ||
		strings.Contains(logs[0], "timed-out") {
		t.Fatalf("parent cancellation warning = %v", logs)
	}
}

func TestExecbound_WaitDelayBoundsInheritedPipes_NonLinux(t *testing.T) {
	if testing.Short() {
		t.Skip("spawns a real descendant that retains output descriptors")
	}
	started := time.Now()
	result := Run(
		context.Background(), helperArgv(), Options{Timeout: 300 * time.Millisecond},
		CaptureCombined, helperSpec("spawn"),
	)
	if !result.TimedOut() {
		t.Fatalf("inherited-pipe run did not time out: %+v", result)
	}
	if elapsed := time.Since(started); elapsed > 4*time.Second {
		t.Fatalf("WaitDelay did not bound inherited pipes: %v", elapsed)
	}
}

func helperArgv() []string {
	return []string{"execbound-nonlinux-helper", "-test.run=^TestExecbound_NonLinuxHelper$"}
}

func helperSpec(mode string) Spec {
	return Spec{Env: helperEnvironment(mode), ExecutablePath: os.Args[0]}
}

func helperEnvironment(mode string) []string {
	prefix := nonLinuxHelperMode + "="
	result := make([]string, 0, len(os.Environ())+1)
	for _, declaration := range os.Environ() {
		if !strings.HasPrefix(declaration, prefix) {
			result = append(result, declaration)
		}
	}
	return append(result, nonLinuxHelperMode+"="+mode)
}
