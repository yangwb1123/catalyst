//go:build linux

package firecracker

import (
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"testing"
	"time"
)

func TestVMProcessStopBoundsEscapedInheritedDiagnosticWriter(t *testing.T) {
	vm, escaped := launchEscapedWriterFixture(t, "block")
	started := time.Now()
	if err := vm.stop(); err != nil {
		t.Fatalf("VM stop failed: %v", err)
	}
	if elapsed := time.Since(started); elapsed > 3*time.Second {
		t.Fatalf("VM stop exceeded bound: %s", elapsed)
	}
	if err := escaped.Signal(syscall.Signal(0)); err != nil {
		t.Fatalf("fixture did not escape direct process group: %v", err)
	}
}

func TestVMProcessResultDoesNotSignalStaleGroupAfterDirectExit(t *testing.T) {
	vm, escaped := launchEscapedWriterFixture(t, "exit")
	awaitVMProcessExit(t, vm)
	originalSignal := signalVMProcessGroup
	signalCalls := 0
	signalVMProcessGroup = func(pid int) error {
		signalCalls++
		return originalSignal(pid)
	}
	t.Cleanup(func() { signalVMProcessGroup = originalSignal })
	started := time.Now()
	err := vm.result()
	if !errors.Is(err, errVMDiagnosticDrainIncomplete) {
		t.Fatalf("escaped diagnostic result = %v", err)
	}
	if elapsed := time.Since(started); elapsed > 3*time.Second {
		t.Fatalf("VM result exceeded bound: %s", elapsed)
	}
	if err := escaped.Signal(syscall.Signal(0)); err != nil {
		t.Fatalf("direct-exit cleanup signaled escaped process: %v", err)
	}
	if signalCalls != 0 {
		t.Fatalf("direct-exit cleanup attempted %d stale group signals", signalCalls)
	}
}

func TestVMProcessStopSignalsLiveProcessGroup(t *testing.T) {
	vm, descendant := launchEscapedWriterFixture(t, "block-group")
	if err := vm.stop(); err != nil {
		t.Fatalf("VM stop failed: %v", err)
	}
	awaitVMProcessGone(t, descendant)
}

func launchEscapedWriterFixture(t *testing.T, mode string) (*vmProcess, *os.Process) {
	t.Helper()
	pidFile := filepath.Join(t.TempDir(), "escaped.pid")
	command := exec.Command(os.Args[0], "-test.run=^TestVMProcessEscapedWriterHelper$")
	command.Env = append(os.Environ(),
		"FORGE_VM_PROCESS_HELPER="+mode, "FORGE_VM_PROCESS_PID_FILE="+pidFile,
	)
	vm, err := launchVMProcess(command, io.Discard)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = vm.stop() })
	escapedPID := waitForEscapedPID(t, pidFile)
	escaped, err := os.FindProcess(escapedPID)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = escaped.Kill()
		_ = escaped.Release()
	})
	return vm, escaped
}

func TestVMProcessEscapedWriterHelper(t *testing.T) {
	mode := os.Getenv("FORGE_VM_PROCESS_HELPER")
	if mode != "block" && mode != "exit" && mode != "block-group" {
		return
	}
	child := exec.Command(os.Args[0], "-test.run=^TestVMProcessEscapedLeafHelper$")
	child.Env = append(os.Environ(), "FORGE_VM_PROCESS_LEAF=1")
	if mode != "block-group" {
		child.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	}
	child.Stdout, child.Stderr = os.Stdout, os.Stderr
	if err := child.Start(); err != nil {
		t.Fatal(err)
	}
	pidFile := os.Getenv("FORGE_VM_PROCESS_PID_FILE")
	if err := os.WriteFile(pidFile, []byte(strconv.Itoa(child.Process.Pid)), 0o600); err != nil {
		t.Fatal(err)
	}
	if mode != "exit" {
		time.Sleep(30 * time.Second)
	}
}

func TestVMProcessEscapedLeafHelper(t *testing.T) {
	if os.Getenv("FORGE_VM_PROCESS_LEAF") == "1" {
		time.Sleep(30 * time.Second)
	}
}

func awaitVMProcessExit(t *testing.T, vm *vmProcess) {
	t.Helper()
	select {
	case <-vm.exited:
	case <-time.After(5 * time.Second):
		t.Fatal("direct VM process did not exit")
	}
}

func awaitVMProcessGone(t *testing.T, process *os.Process) {
	t.Helper()
	deadline := time.Now().Add(5 * time.Second)
	for time.Now().Before(deadline) {
		if err := process.Signal(syscall.Signal(0)); errors.Is(err, os.ErrProcessDone) {
			return
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatalf("process %d survived group termination", process.Pid)
}

func waitForEscapedPID(t *testing.T, path string) int {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		payload, err := os.ReadFile(path)
		if err == nil {
			pid, parseErr := strconv.Atoi(strings.TrimSpace(string(payload)))
			if parseErr == nil && pid > 0 {
				return pid
			}
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal(fmt.Errorf("escaped helper PID was not published"))
	return 0
}
