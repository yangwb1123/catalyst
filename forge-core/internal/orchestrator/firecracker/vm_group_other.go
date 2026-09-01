//go:build !unix

package firecracker

import (
	"errors"
	"os/exec"
)

func configureVMProcess(_ *exec.Cmd) {}

func (vm *vmProcess) terminate() (bool, error) {
	vm.lifecycleMu.Lock()
	defer vm.lifecycleMu.Unlock()
	if vm.reaped {
		return false, nil
	}
	err := vm.process.Kill()
	return err == nil, err
}

func (vm *vmProcess) waitForExit() error {
	state, err := vm.process.Wait()
	vm.lifecycleMu.Lock()
	vm.reaped = true
	vm.lifecycleMu.Unlock()
	if err == nil && !state.Success() {
		err = &exec.ExitError{ProcessState: state}
	}
	return err
}

func isVMTerminationError(err error) bool {
	var exitErr *exec.ExitError
	return errors.As(err, &exitErr)
}
