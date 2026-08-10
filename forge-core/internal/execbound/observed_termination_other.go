//go:build !unix

package execbound

import (
	"errors"
	"os"
)

func observedCancelProcessDone(err error) bool {
	return errors.Is(err, os.ErrProcessDone)
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
	code := state.ExitCode()
	if code < 0 {
		return 0, 0, "", false, false
	}
	return int64(code), 0, "", false, true
}
