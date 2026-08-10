package execbound

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"sync"
	"time"
)

type observedPipe struct {
	reader          *os.File
	writer          *os.File
	readerCloseOnce sync.Once
	writerCloseOnce sync.Once
	writerCloseErr  error
}

type observedDrainResult struct {
	err error
}

func newObservedPipes() (*observedPipe, *observedPipe, error) {
	stdoutReader, stdoutWriter, err := os.Pipe()
	if err != nil {
		return nil, nil, fmt.Errorf("create observed stdout pipe: %w", err)
	}
	stdout := &observedPipe{reader: stdoutReader, writer: stdoutWriter}
	stderrReader, stderrWriter, err := os.Pipe()
	if err != nil {
		stdout.close()
		return nil, nil, fmt.Errorf("create observed stderr pipe: %w", err)
	}
	return stdout, &observedPipe{reader: stderrReader, writer: stderrWriter}, nil
}

func (pipe *observedPipe) closeWriter() error {
	if pipe == nil {
		return nil
	}
	pipe.writerCloseOnce.Do(func() {
		if pipe.writer != nil {
			pipe.writerCloseErr = pipe.writer.Close()
		}
	})
	return pipe.writerCloseErr
}

func (pipe *observedPipe) closeReader() {
	if pipe == nil {
		return
	}
	pipe.readerCloseOnce.Do(func() {
		if pipe.reader != nil {
			_ = pipe.reader.Close()
		}
	})
}

func (pipe *observedPipe) close() {
	_ = pipe.closeWriter()
	pipe.closeReader()
}

func (pipe *observedPipe) drain(target io.Writer, results chan<- observedDrainResult) {
	go func() {
		_, err := io.Copy(target, pipe.reader)
		results <- observedDrainResult{err: err}
	}()
}

// awaitObservedDrains proves that both raw stream readers reached natural EOF.
// A process exit is not enough: a descendant may still hold either inherited
// writer. The timer starts from the earlier of process-wait completion and the
// controller cancel callback, preserving the existing deadline+WaitDelay
// upper bound. If timer and completion race, the timer is conservatively an
// incomplete drain.
func awaitObservedDrains(
	stdout, stderr *observedPipe,
	results <-chan observedDrainResult,
	cancel cancelSnapshot,
) (bool, error) {
	deadline := time.Now().Add(waitDelay)
	if !cancel.at.IsZero() {
		cancelDeadline := cancel.at.Add(waitDelay)
		if cancelDeadline.Before(deadline) {
			deadline = cancelDeadline
		}
	}
	duration := time.Until(deadline)
	if duration < 0 {
		duration = 0
	}
	timer := time.NewTimer(duration)
	defer stopObservedTimer(timer)

	firstErr := firstObservedPipeError(stdout, stderr)
	forced, firstErr := collectObservedDrains(
		stdout, stderr, results, timer, deadline, firstErr,
	)
	stdout.closeReader()
	stderr.closeReader()
	if forced {
		return false, exec.ErrWaitDelay
	}
	if firstErr != nil {
		return false, firstErr
	}
	return true, nil
}

func collectObservedDrains(
	stdout, stderr *observedPipe,
	results <-chan observedDrainResult,
	timer *time.Timer,
	deadline time.Time,
	firstErr error,
) (bool, error) {
	completed := 0
	forced := false
	for completed < 2 {
		if !forced && !time.Now().Before(deadline) {
			forced = true
			stdout.closeReader()
			stderr.closeReader()
		}
		if forced {
			result := <-results
			completed++
			if result.err != nil && firstErr == nil {
				firstErr = result.err
			}
			continue
		}
		select {
		case result := <-results:
			completed++
			if result.err != nil && firstErr == nil {
				firstErr = result.err
			}
			if completed == 2 && !time.Now().Before(deadline) {
				forced = true
			}
		case <-timer.C:
			forced = true
			stdout.closeReader()
			stderr.closeReader()
		}
	}
	return forced, firstErr
}

func firstObservedPipeError(stdout, stderr *observedPipe) error {
	if stdout.writerCloseErr != nil {
		return stdout.writerCloseErr
	}
	return stderr.writerCloseErr
}

func stopObservedTimer(timer *time.Timer) {
	if timer.Stop() {
		return
	}
	select {
	case <-timer.C:
	default:
	}
}
