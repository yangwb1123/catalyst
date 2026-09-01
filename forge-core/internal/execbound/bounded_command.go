package execbound

import (
	"context"
	"errors"
	"fmt"
	"os"
	"os/exec"
)

// boundedCommand owns the cancellation watcher instead of os/exec. That lets
// Linux serialize numeric process-group signalling with the actual reap while
// retaining Cmd.Wait's ProcessState and stdin-copy semantics.
type boundedCommand struct {
	*exec.Cmd
	runCtx      context.Context
	lifecycle   *processLifecycle
	tracker     *cancelTracker
	cancel      func(*exec.Cmd) error
	processDone chan struct{}
	cancelDone  chan struct{}
}

func (command *boundedCommand) Start() error {
	if err := command.runCtx.Err(); err != nil {
		return err
	}
	if err := command.Cmd.Start(); err != nil {
		return err
	}
	go command.watchCancellation()
	return nil
}

func (command *boundedCommand) Wait() error {
	err := command.lifecycle.wait(command.Cmd)
	close(command.processDone)
	<-command.cancelDone
	if err != nil || command.tracker == nil {
		return err
	}
	cancelled := command.tracker.snapshot()
	if !cancelled.called || cancelled.err == nil || errors.Is(cancelled.err, os.ErrProcessDone) {
		return nil
	}
	// Match os/exec's precedence: a process/copy wait error wins; otherwise a
	// cancellation error that is not the benign already-finished race must be
	// observable instead of allowing a later zero exit to look successful.
	return fmt.Errorf("exec: canceling Cmd: %w", cancelled.err)
}

func (command *boundedCommand) watchCancellation() {
	defer close(command.cancelDone)
	select {
	case <-command.processDone:
		return
	case <-command.runCtx.Done():
	}
	err := command.cancel(command.Cmd)
	if command.tracker != nil {
		command.tracker.record(command.runCtx.Err(), err)
	}
}
