//go:build unix

package execbound

import (
	"errors"
	"os"
	"syscall"
)

func observedCancelProcessDone(err error) bool {
	return errors.Is(err, os.ErrProcessDone) || errors.Is(err, syscall.ESRCH)
}

func observedProcessState(state *os.ProcessState) (
	exitCode int64,
	signalNumber uint32,
	signalName string,
	signaled bool,
	ok bool,
) {
	if state == nil {
		return 0, 0, "", false, false
	}
	status, statusOK := state.Sys().(syscall.WaitStatus)
	if statusOK && status.Signaled() {
		signal := status.Signal()
		return 0, uint32(signal), signal.String(), true, true
	}
	code := state.ExitCode()
	if code < 0 {
		return 0, 0, "", false, false
	}
	return int64(code), 0, "", false, true
}
