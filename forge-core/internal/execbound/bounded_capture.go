package execbound

import (
	"context"
)

type boundedCapture struct {
	mode           Capture
	stdout, stderr *cappedBuffer
	merged         *cappedBuffer
	pipes          []*observedPipe
	drains         chan observedDrainResult
}

func newBoundedCapture(mode Capture, capacity int) (*boundedCapture, error) {
	capture := &boundedCapture{
		mode: mode, drains: make(chan observedDrainResult, 2),
	}
	if mode == CaptureSplit {
		stdout, stderr, err := newObservedPipes()
		if err != nil {
			return nil, err
		}
		capture.pipes = []*observedPipe{stdout, stderr}
		capture.stdout = &cappedBuffer{cap: capacity}
		capture.stderr = &cappedBuffer{cap: capacity}
	} else {
		combined, err := newObservedPipe("combined")
		if err != nil {
			return nil, err
		}
		capture.pipes = []*observedPipe{combined}
		capture.merged = &cappedBuffer{cap: capacity}
	}
	return capture, nil
}

func (capture *boundedCapture) close() {
	for _, pipe := range capture.pipes {
		pipe.close()
	}
}

func (capture *boundedCapture) start(command *boundedCommand) error {
	command.Stdout = capture.pipes[0].writer
	command.Stderr = capture.pipes[len(capture.pipes)-1].writer
	if err := command.Start(); err != nil {
		return err
	}
	for _, pipe := range capture.pipes {
		_ = pipe.closeWriter()
	}
	if capture.mode == CaptureSplit {
		capture.pipes[0].drain(capture.stdout, capture.drains)
		capture.pipes[1].drain(capture.stderr, capture.drains)
	} else {
		capture.pipes[0].drain(capture.merged, capture.drains)
	}
	return nil
}

func runCapturedCommand(
	command *boundedCommand,
	runCtx context.Context,
	cancelled *cancelTracker,
	opts Options,
	mode Capture,
) Result {
	capture, err := newBoundedCapture(mode, maxBytes(opts.MaxOutputBytes))
	if err != nil {
		return Result{Err: err, CtxErr: runCtx.Err()}
	}
	defer capture.close()
	if err := capture.start(command); err != nil {
		return capturedResult(err, runCtx.Err(), cancelled.snapshot())
	}
	waitErr := command.Wait()
	cancelSnapshot := cancelled.snapshot()
	drainComplete, drainErr := awaitPipeDrains(capture.pipes, capture.drains, cancelSnapshot)
	if waitErr == nil {
		waitErr = drainErr
	}
	result := capturedResult(waitErr, runCtx.Err(), cancelSnapshot)
	result.DrainIncomplete = !drainComplete
	capture.populate(&result)
	result.logDegradation(opts)
	return result
}

func (capture *boundedCapture) populate(result *Result) {
	if capture.mode == CaptureSplit {
		result.Stdout, result.Stderr = capture.stdout.buf, capture.stderr.buf
		result.Total, result.CountOverflow = combinedByteCount(capture.stdout, capture.stderr)
		result.Retained = legacyRetained(len(capture.stdout.buf), len(capture.stderr.buf))
		result.renderTotal = capture.stdout.total
		result.renderOverflow = capture.stdout.countOverflow
		result.renderCountSet = true
		return
	}
	result.Merged, result.Total = capture.merged.buf, capture.merged.total
	result.Retained = len(capture.merged.buf)
	result.CountOverflow = capture.merged.countOverflow
}
