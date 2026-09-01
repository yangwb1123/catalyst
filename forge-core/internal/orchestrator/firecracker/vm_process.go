package firecracker

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"sync"
	"time"
)

const vmProcessWaitTimeout = 2 * time.Second

var errVMDiagnosticDrainIncomplete = errors.New("firecracker diagnostic drain did not complete")
var errVMProcessExitIncomplete = errors.New("firecracker process did not exit after termination")

// vmProcess gives exactly one goroutine ownership of process reaping. On Unix,
// reap and process-group termination share lifecycleMu so a numeric PGID can
// never be signalled after its leader has been reaped and made reusable.
type vmProcess struct {
	process          *os.Process
	pid              int
	diagnosticReader *os.File
	exited           chan struct{}
	drained          chan struct{}
	lifecycleMu      sync.Mutex
	reaped           bool
	mu               sync.Mutex
	waitErr          error
	drainCopyErr     error
	readerCloseErr   error
	stopOnce         sync.Once
	stopErr          error
}

func launchVMProcess(cmd *exec.Cmd, diagnostics io.Writer) (*vmProcess, error) {
	reader, writer, err := os.Pipe()
	if err != nil {
		return nil, err
	}
	cmd.Stdout, cmd.Stderr = writer, writer
	configureVMProcess(cmd)
	if err := cmd.Start(); err != nil {
		_ = reader.Close()
		_ = writer.Close()
		return nil, err
	}
	vm := startVMProcess(cmd, reader, diagnostics)
	if err := writer.Close(); err != nil {
		return nil, errors.Join(
			fmt.Errorf("close parent diagnostic writer: %w", err), vm.stop(),
		)
	}
	return vm, nil
}

func startVMProcess(cmd *exec.Cmd, reader *os.File, diagnostics io.Writer) *vmProcess {
	vm := &vmProcess{
		process: cmd.Process, pid: cmd.Process.Pid, diagnosticReader: reader,
		exited: make(chan struct{}), drained: make(chan struct{}),
	}
	go func() {
		err := vm.waitForExit()
		vm.mu.Lock()
		vm.waitErr = err
		vm.mu.Unlock()
		close(vm.exited)
	}()
	go func() {
		_, copyErr := io.Copy(diagnostics, reader)
		closeErr := reader.Close()
		vm.mu.Lock()
		vm.drainCopyErr = copyErr
		vm.readerCloseErr = closeErr
		vm.mu.Unlock()
		close(vm.drained)
	}()
	return vm
}

func (vm *vmProcess) stop() error {
	vm.stopOnce.Do(func() {
		var failures []error
		terminated, err := vm.terminate()
		if err != nil && !errors.Is(err, os.ErrProcessDone) {
			failures = append(failures, fmt.Errorf("terminate firecracker process: %w", err))
		}
		closedReader := false
		if !channelClosed(vm.drained) {
			closedReader = true
			if err := vm.diagnosticReader.Close(); err != nil && !errors.Is(err, os.ErrClosed) {
				failures = append(failures, fmt.Errorf("close firecracker diagnostic reader: %w", err))
			}
		}
		deadline := time.Now().Add(vmProcessWaitTimeout)
		exited := awaitVMChannel(vm.exited, deadline)
		drained := awaitVMChannel(vm.drained, deadline)
		failures = append(failures, vm.stopCompletionErrors(
			exited, drained, terminated, closedReader,
		)...)
		vm.stopErr = errors.Join(failures...)
	})
	return vm.stopErr
}

func (vm *vmProcess) stopCompletionErrors(
	exited, drained, terminated, closedReader bool,
) []error {
	var failures []error
	if !exited {
		failures = append(failures, errVMProcessExitIncomplete)
	} else if err := vm.waitError(); err != nil &&
		!(terminated && isVMTerminationError(err)) {
		failures = append(failures, fmt.Errorf("wait for firecracker process: %w", err))
	}
	if !drained {
		failures = append(failures, errVMDiagnosticDrainIncomplete)
	} else if err := vm.diagnosticStopError(closedReader); err != nil {
		failures = append(failures, fmt.Errorf("firecracker diagnostic drain: %w", err))
	}
	return failures
}

func (vm *vmProcess) result() error {
	<-vm.exited
	if !awaitVMChannel(vm.drained, time.Now().Add(vmProcessWaitTimeout)) {
		return errors.Join(
			vm.waitError(), errVMDiagnosticDrainIncomplete, vm.stop(),
		)
	}
	if err := vm.diagnosticError(); err != nil {
		return errors.Join(vm.waitError(), fmt.Errorf("firecracker diagnostic drain: %w", err))
	}
	return vm.waitError()
}

func (vm *vmProcess) waitError() error {
	vm.mu.Lock()
	defer vm.mu.Unlock()
	return vm.waitErr
}

func (vm *vmProcess) diagnosticError() error {
	vm.mu.Lock()
	defer vm.mu.Unlock()
	return errors.Join(vm.drainCopyErr, vm.readerCloseErr)
}

func (vm *vmProcess) diagnosticStopError(closedReader bool) error {
	vm.mu.Lock()
	defer vm.mu.Unlock()
	copyErr, closeErr := vm.drainCopyErr, vm.readerCloseErr
	if closedReader && errors.Is(copyErr, os.ErrClosed) {
		copyErr = nil
	}
	if closedReader && errors.Is(closeErr, os.ErrClosed) {
		closeErr = nil
	}
	return errors.Join(copyErr, closeErr)
}

func channelClosed(channel <-chan struct{}) bool {
	select {
	case <-channel:
		return true
	default:
		return false
	}
}

func awaitVMChannel(channel <-chan struct{}, deadline time.Time) bool {
	if channelClosed(channel) {
		return true
	}
	remaining := time.Until(deadline)
	if remaining <= 0 {
		return false
	}
	timer := time.NewTimer(remaining)
	defer timer.Stop()
	select {
	case <-channel:
		return true
	case <-timer.C:
		return false
	}
}

func (vm *vmProcess) earlyExitError() error {
	if err := vm.result(); err != nil {
		return fmt.Errorf("firecracker exited before its API socket was ready: %w", err)
	}
	return errors.New("firecracker exited before its API socket was ready")
}

// waitForVM treats the VMM process exit as the only completion event. Serial
// bytes are diagnostics and can stop the VM on overflow, but cannot report a
// guest exit status.
func (r *FirecrackerRunner) waitForVM(
	ctx context.Context,
	vm *vmProcess,
	serial *serialCapture,
	timeout time.Duration,
) error {
	select {
	case <-ctx.Done():
		return firecrackerContextError(ctx.Err(), timeout)
	case <-serial.overflow:
		return serial.limitError()
	case <-vm.exited:
	}
	if err := ctx.Err(); err != nil {
		return firecrackerContextError(err, timeout)
	}
	if err := serial.limitError(); err != nil {
		return err
	}
	if err := vm.result(); err != nil {
		return configFault(fmt.Errorf("firecracker VMM exited abnormally: %w", err))
	}
	return nil
}
