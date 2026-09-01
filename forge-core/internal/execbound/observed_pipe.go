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
	stdout, err := newObservedPipe("stdout")
	if err != nil {
		return nil, nil, err
	}
	stderr, err := newObservedPipe("stderr")
	if err != nil {
		stdout.close()
		return nil, nil, err
	}
	return stdout, stderr, nil
}

func newObservedPipe(label string) (*observedPipe, error) {
	reader, writer, err := os.Pipe()
	if err != nil {
		return nil, fmt.Errorf("create observed %s pipe: %w", label, err)
	}
	return &observedPipe{reader: reader, writer: writer}, nil
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
	return awaitPipeDrains([]*observedPipe{stdout, stderr}, results, cancel)
}

func awaitPipeDrains(
	pipes []*observedPipe,
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

	firstErr := firstObservedPipeError(pipes...)
	forced, firstErr := collectObservedDrains(
		pipes, results, timer, deadline, firstErr,
	)
	closeObservedReaders(pipes)
	if forced {
		return false, exec.ErrWaitDelay
	}
	if firstErr != nil {
		return false, firstErr
	}
	return true, nil
}

func collectObservedDrains(
	pipes []*observedPipe,
	results <-chan observedDrainResult,
	timer *time.Timer,
	deadline time.Time,
	firstErr error,
) (bool, error) {
	completed := 0
	forced := false
	for completed < len(pipes) {
		if !forced && !time.Now().Before(deadline) {
			forced = true
			closeObservedReaders(pipes)
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
			if completed == len(pipes) && !time.Now().Before(deadline) {
				forced = true
			}
		case <-timer.C:
			forced = true
			closeObservedReaders(pipes)
		}
	}
	return forced, firstErr
}

func firstObservedPipeError(pipes ...*observedPipe) error {
	for _, pipe := range pipes {
		if pipe.writerCloseErr != nil {
			return pipe.writerCloseErr
		}
	}
	return nil
}

func closeObservedReaders(pipes []*observedPipe) {
	for _, pipe := range pipes {
		pipe.closeReader()
	}
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
