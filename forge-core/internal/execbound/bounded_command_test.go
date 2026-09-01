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

const cancellationFailureHelper = "FORGE_EXECBOUND_CANCEL_FAILURE_HELPER"

var errTestCancellationFailure = errors.New("injected cancellation failure")

func TestBoundedCommandWaitReportsCancellationFailure(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	command := newBoundedCommand(ctx,
		[]string{os.Args[0], "-test.run=^TestBoundedCommandCancellationFailureHelper$"},
		Spec{Env: append(os.Environ(), cancellationFailureHelper+"=1")},
	)
	tracker := &cancelTracker{}
	wrapTrackedCancel(command, ctx, tracker)
	cancelCalled := make(chan struct{})
	command.cancel = func(*exec.Cmd) error {
		close(cancelCalled)
		return errTestCancellationFailure
	}
	if err := command.Start(); err != nil {
		t.Fatal(err)
	}
	cancel()
	<-cancelCalled
	err := command.Wait()
	if !errors.Is(err, errTestCancellationFailure) ||
		!strings.Contains(err.Error(), "exec: canceling Cmd") {
		t.Fatalf("cancellation failure wait error = %v", err)
	}
	if command.ProcessState == nil || !command.ProcessState.Success() {
		t.Fatalf("helper process state = %v", command.ProcessState)
	}
	execution := ExecutionObservation{}
	classifyObservedTermination(&execution, command.Cmd, err, tracker.snapshot())
	if execution.Termination != terminationWaitFailed || execution.ExitCode != nil {
		t.Fatalf("cancellation failure classification = %+v", execution)
	}
	execution = ExecutionObservation{}
	classifyObservedTermination(&execution, command.Cmd, nil, tracker.snapshot())
	if execution.Termination != terminationWaitFailed || execution.ExitCode != nil {
		t.Fatalf("defensive cancellation classification = %+v", execution)
	}
}

func TestBoundedCommandCancellationFailureHelper(t *testing.T) {
	if os.Getenv(cancellationFailureHelper) == "1" {
		time.Sleep(100 * time.Millisecond)
	}
}
