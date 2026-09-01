//go:build unix

package firecracker

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"syscall"
	"time"
)

const vmProcessPollInterval = 5 * time.Millisecond

var signalVMProcessGroup = func(pid int) error {
	return syscall.Kill(-pid, syscall.SIGKILL)
}

type vmProcessExitError struct {
	status syscall.WaitStatus
}

func (err *vmProcessExitError) Error() string {
	if err.status.Signaled() {
		return fmt.Sprintf("signal: %s", err.status.Signal())
	}
	return fmt.Sprintf("exit status %d", err.status.ExitStatus())
}

func (err *vmProcessExitError) ExitCode() int {
	if !err.status.Exited() {
		return -1
	}
	return err.status.ExitStatus()
}

func configureVMProcess(command *exec.Cmd) {
	command.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
}

func (vm *vmProcess) terminate() (bool, error) {
	vm.lifecycleMu.Lock()
	defer vm.lifecycleMu.Unlock()
	if vm.reaped {
		return false, nil
	}
	err := signalVMProcessGroup(vm.pid)
	if errors.Is(err, syscall.ESRCH) {
		return false, os.ErrProcessDone
	}
	return err == nil, err
}

func (vm *vmProcess) waitForExit() error {
	var status syscall.WaitStatus
	var usage syscall.Rusage
	for {
		vm.lifecycleMu.Lock()
		waited, err := syscall.Wait4(vm.pid, &status, vmWaitNoHang, &usage)
		if errors.Is(err, syscall.EINTR) {
			vm.lifecycleMu.Unlock()
			continue
		}
		if waited == vm.pid || err != nil {
			vm.reaped = true
			releaseErr := vm.process.Release()
			vm.lifecycleMu.Unlock()
			return vmProcessWaitResult(status, err, releaseErr)
		}
		vm.lifecycleMu.Unlock()
		time.Sleep(vmProcessPollInterval)
	}
}

func vmProcessWaitResult(status syscall.WaitStatus, waitErr, releaseErr error) error {
	if waitErr != nil {
		return errors.Join(os.NewSyscallError("wait", waitErr), releaseErr)
	}
	if status.Exited() && status.ExitStatus() == 0 {
		return releaseErr
	}
	return errors.Join(&vmProcessExitError{status: status}, releaseErr)
}

func isVMTerminationError(err error) bool {
	var exitErr *vmProcessExitError
	return errors.As(err, &exitErr) && exitErr.status.Signaled() &&
		exitErr.status.Signal() == syscall.SIGKILL
}
