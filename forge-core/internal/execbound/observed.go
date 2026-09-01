package execbound

import (
	"context"
	"crypto/sha256"
	"errors"
	"hash"
	"math"
	"os"
	"os/exec"
	"sync"
	"time"
)

// ObservationOptions controls the additive, opt-in execution observation.
// The zero value uses time.Now. The clock is read immediately before Start and
// after process wait plus stream drain, so the interval covers every observed
// byte. A failed Start never becomes a producible observation.
type ObservationOptions struct {
	Now func() time.Time
}

// StreamObservation is one raw stream as observed by this producer. SHA256
// covers every drained byte, while Retained and RetainedSHA256 cover only the
// bounded prefix kept in memory. CountOverflow means the count no longer fits
// the signed-int64 wire vocabulary used by command-observation/v1; Bytes is
// still the exact uint64 count unless uint64 itself was saturated.
type StreamObservation struct {
	Bytes          uint64
	Retained       []byte
	SHA256         [sha256.Size]byte
	RetainedSHA256 [sha256.Size]byte
	CountOverflow  bool
}

// TerminationKind is the producer-observed terminal classification. Only
// exited, timed_out, and cancelled belong to command-observation/v1; callers
// must reject not_started, spawn_failed, signaled, and wait_failed rather than
// inventing an exit-code sentinel.
type TerminationKind string

const (
	TerminationNotStarted  TerminationKind = "not_started"
	TerminationExited      TerminationKind = "exited"
	TerminationTimedOut    TerminationKind = "timed_out"
	TerminationCancelled   TerminationKind = "cancelled"
	TerminationSignaled    TerminationKind = "signaled"
	terminationSpawnFailed TerminationKind = "spawn_failed"
	terminationWaitFailed  TerminationKind = "wait_failed"
)

// ExecutionObservation describes process lifecycle facts that cannot be
// recovered honestly from Result.Err alone. StartedAt is zero when Started is
// false. DrainComplete is false when WaitDelay had to close inherited pipes.
type ExecutionObservation struct {
	StartedAt     time.Time
	EndedAt       time.Time
	Started       bool
	DrainComplete bool
	Termination   TerminationKind
	ExitCode      *int64
	SignalNumber  *uint32
	SignalName    string
}

// ObservedResult preserves the ordinary Result projection while adding exact
// stdout, stderr, and producer-serialized combined drain observations.
type ObservedResult struct {
	Legacy    Result
	Execution ExecutionObservation
	Stdout    StreamObservation
	Stderr    StreamObservation
	Combined  StreamObservation
}

// RunObserved is the opt-in evidence-producer twin of Run. Unlike Run's
// same-writer combined capture, it uses two tagged writers guarded by one
// mutex. Each Write is therefore one producer-observed drain event: its raw
// bytes update the tagged stream and then the combined stream while holding
// the lock. Combined records this producer serialization only; it does not
// attest the child or kernel's original emission order.
//
// Retention remains bounded at three prefixes of the effective output cap.
// Full digests and byte counts continue across discarded overflow bytes.
func RunObserved(
	ctx context.Context,
	argv []string,
	opts Options,
	capture Capture,
	spec Spec,
	observation ObservationOptions,
) ObservedResult {
	clock := observation.Now
	if clock == nil {
		clock = time.Now
	}
	captured := newObservedCapture(maxBytes(opts.MaxOutputBytes))
	if err := opts.Validate(); err != nil {
		return captured.result(capture, err, nil, ExecutionObservation{
			EndedAt: clock(), Termination: TerminationNotStarted,
		})
	}
	if err := validateSpec(spec); err != nil {
		return captured.result(capture, err, nil, ExecutionObservation{
			EndedAt: clock(), Termination: TerminationNotStarted,
		})
	}
	if len(argv) == 0 {
		return captured.result(capture, errors.New("empty argv"), nil, ExecutionObservation{
			EndedAt: clock(), Termination: TerminationNotStarted,
		})
	}
	return runObservedValidated(ctx, argv, opts, capture, spec, clock, captured)
}

func runObservedValidated(
	ctx context.Context,
	argv []string,
	opts Options,
	capture Capture,
	spec Spec,
	clock func() time.Time,
	captured *observedCapture,
) ObservedResult {
	runCtx, runCancel := deadlineContext(ctx, opts)
	defer runCancel()
	cmd := newObservedCommand(runCtx, argv, spec)
	stdoutPipe, stderrPipe, err := newObservedPipes()
	if err != nil {
		return captured.result(capture, err, runCtx.Err(), ExecutionObservation{
			EndedAt: clock(), Termination: TerminationNotStarted,
		})
	}
	defer stdoutPipe.close()
	defer stderrPipe.close()
	cmd.Stdout = stdoutPipe.writer
	cmd.Stderr = stderrPipe.writer

	cancelled := &cancelTracker{}
	wrapTrackedCancel(cmd, runCtx, cancelled)
	attemptStartedAt := clock()
	if err := cmd.Start(); err != nil {
		return captured.result(capture, err, runCtx.Err(), ExecutionObservation{
			EndedAt: clock(), Termination: terminationSpawnFailed,
		})
	}
	// The child owns duplicated write descriptors after Start. Closing the
	// parent copies is what lets the readers prove natural EOF after the child
	// and every inheriting descendant have closed their copies.
	// closeWriter retains any error for firstObservedPipeError after Wait; the
	// immediate calls only release the parent's duplicate descriptors.
	_ = stdoutPipe.closeWriter()
	_ = stderrPipe.closeWriter()
	drains := make(chan observedDrainResult, 2)
	stdoutPipe.drain(captured.writer(streamStdout), drains)
	stderrPipe.drain(captured.writer(streamStderr), drains)
	return finishObservedRun(
		cmd, stdoutPipe, stderrPipe, drains, cancelled, runCtx, opts, capture,
		attemptStartedAt, clock, captured,
	)
}

func newObservedCommand(ctx context.Context, argv []string, spec Spec) *boundedCommand {
	return newBoundedCommand(ctx, argv, spec)
}

func finishObservedRun(
	cmd *boundedCommand,
	stdoutPipe, stderrPipe *observedPipe,
	drains <-chan observedDrainResult,
	cancelled *cancelTracker,
	runCtx context.Context,
	opts Options,
	capture Capture,
	attemptStartedAt time.Time,
	clock func() time.Time,
	captured *observedCapture,
) ObservedResult {
	execution := ExecutionObservation{Started: true, StartedAt: attemptStartedAt}
	waitErr := cmd.Wait()
	cancelSnapshot := cancelled.snapshot()
	drainComplete, drainErr := awaitObservedDrains(stdoutPipe, stderrPipe, drains, cancelSnapshot)
	execution.EndedAt = clock()
	execution.DrainComplete = drainComplete
	// Match os/exec's legacy precedence: a process failure wins over a pipe
	// copy failure; otherwise an incomplete drain is ErrWaitDelay and a natural
	// copy error is returned directly.
	if waitErr == nil {
		waitErr = drainErr
	}
	classifyObservedTermination(&execution, cmd.Cmd, waitErr, cancelSnapshot)

	result := captured.result(capture, waitErr, runCtx.Err(), execution)
	result.Legacy.cancelApplied = cancelSnapshot.called && cancelSnapshot.err == nil
	result.Legacy.logDegradation(opts)
	return result
}

type observedStreamKind uint8

const (
	streamStdout observedStreamKind = iota
	streamStderr
)

type observedCapture struct {
	mu       sync.Mutex
	stdout   observedStream
	stderr   observedStream
	combined observedStream
}

func newObservedCapture(capacity int) *observedCapture {
	return &observedCapture{
		stdout: newObservedStream(capacity), stderr: newObservedStream(capacity),
		combined: newObservedStream(capacity),
	}
}

func (capture *observedCapture) writer(kind observedStreamKind) *taggedWriter {
	return &taggedWriter{capture: capture, kind: kind}
}

func (capture *observedCapture) write(kind observedStreamKind, payload []byte) {
	capture.mu.Lock()
	defer capture.mu.Unlock()
	if kind == streamStdout {
		capture.stdout.write(payload)
	} else {
		capture.stderr.write(payload)
	}
	capture.combined.write(payload)
}

func (capture *observedCapture) snapshots() (StreamObservation, StreamObservation, StreamObservation) {
	capture.mu.Lock()
	defer capture.mu.Unlock()
	return capture.stdout.snapshot(), capture.stderr.snapshot(), capture.combined.snapshot()
}

func (capture *observedCapture) result(
	mode Capture,
	runErr error,
	ctxErr error,
	execution ExecutionObservation,
) ObservedResult {
	stdout, stderr, combined := capture.snapshots()
	legacy := Result{
		Err: preferContextFailure(runErr, ctxErr), CtxErr: ctxErr,
		DrainIncomplete: execution.Started && !execution.DrainComplete,
	}
	if mode == CaptureSplit {
		legacy.Stdout = append([]byte(nil), stdout.Retained...)
		legacy.Stderr = append([]byte(nil), stderr.Retained...)
		legacy.Total, legacy.CountOverflow = legacyCount(stdout.Bytes, stderr.Bytes)
		legacy.Retained = legacyRetained(len(stdout.Retained), len(stderr.Retained))
		legacy.renderTotal, legacy.renderOverflow = legacyCount(stdout.Bytes)
		legacy.renderCountSet = true
	} else {
		legacy.Merged = append([]byte(nil), combined.Retained...)
		legacy.Total, legacy.CountOverflow = legacyCount(combined.Bytes)
		legacy.Retained = len(combined.Retained)
	}
	return ObservedResult{
		Legacy: legacy, Execution: execution,
		Stdout: stdout, Stderr: stderr, Combined: combined,
	}
}

type taggedWriter struct {
	capture *observedCapture
	kind    observedStreamKind
}

func (writer *taggedWriter) Write(payload []byte) (int, error) {
	writer.capture.write(writer.kind, payload)
	return len(payload), nil
}

type observedStream struct {
	capacity      int
	bytes         uint64
	retained      []byte
	fullHash      hash.Hash
	retainedHash  hash.Hash
	countOverflow bool
}

func newObservedStream(capacity int) observedStream {
	return observedStream{
		capacity: capacity, fullHash: sha256.New(), retainedHash: sha256.New(),
	}
}

func (stream *observedStream) write(payload []byte) {
	_, _ = stream.fullHash.Write(payload)
	length := uint64(len(payload))
	if stream.bytes > math.MaxUint64-length {
		stream.bytes = math.MaxUint64
		stream.countOverflow = true
	} else {
		stream.bytes += length
		if stream.bytes > math.MaxInt64 {
			stream.countOverflow = true
		}
	}
	if room := stream.capacity - len(stream.retained); room > 0 {
		kept := payload
		if len(kept) > room {
			kept = kept[:room]
		}
		stream.retained = append(stream.retained, kept...)
		_, _ = stream.retainedHash.Write(kept)
	}
}

func (stream *observedStream) snapshot() StreamObservation {
	var fullDigest, retainedDigest [sha256.Size]byte
	copy(fullDigest[:], stream.fullHash.Sum(nil))
	copy(retainedDigest[:], stream.retainedHash.Sum(nil))
	return StreamObservation{
		Bytes: stream.bytes, Retained: append([]byte(nil), stream.retained...),
		SHA256: fullDigest, RetainedSHA256: retainedDigest,
		CountOverflow: stream.countOverflow,
	}
}

func legacyCount(values ...uint64) (int64, bool) {
	var total uint64
	for _, value := range values {
		if value > math.MaxUint64-total {
			return math.MaxInt64, true
		}
		total += value
		if total > math.MaxInt64 {
			return math.MaxInt64, true
		}
	}
	return int64(total), false
}

func legacyRetained(left, right int) int {
	if left > math.MaxInt-right {
		return math.MaxInt
	}
	return left + right
}

type cancelSnapshot struct {
	called bool
	cause  error
	err    error
	at     time.Time
}

type cancelTracker struct {
	mu     sync.Mutex
	called bool
	cause  error
	err    error
	at     time.Time
}

func (tracker *cancelTracker) record(cause, err error) {
	tracker.mu.Lock()
	defer tracker.mu.Unlock()
	tracker.called, tracker.cause, tracker.err, tracker.at = true, cause, err, time.Now()
}

func (tracker *cancelTracker) snapshot() cancelSnapshot {
	tracker.mu.Lock()
	defer tracker.mu.Unlock()
	return cancelSnapshot{called: tracker.called, cause: tracker.cause, err: tracker.err, at: tracker.at}
}

func wrapTrackedCancel(cmd *boundedCommand, _ context.Context, tracker *cancelTracker) {
	cmd.tracker = tracker
}

func classifyObservedTermination(
	execution *ExecutionObservation,
	cmd *exec.Cmd,
	waitErr error,
	cancel cancelSnapshot,
) {
	if cancel.called && cancel.err == nil {
		if errors.Is(cancel.cause, context.DeadlineExceeded) {
			execution.Termination = TerminationTimedOut
		} else {
			execution.Termination = TerminationCancelled
		}
		return
	}
	if waitErr == nil && cancel.called && cancel.err != nil &&
		!errors.Is(cancel.err, os.ErrProcessDone) {
		execution.Termination = terminationWaitFailed
		return
	}
	var exitError *exec.ExitError
	if waitErr != nil && !errors.As(waitErr, &exitError) {
		execution.Termination = terminationWaitFailed
		return
	}
	if code, signalNumber, signalName, signaled, ok := observedProcessState(cmd.ProcessState); ok {
		if signaled {
			execution.Termination = TerminationSignaled
			execution.SignalNumber = &signalNumber
			execution.SignalName = signalName
			return
		}
		execution.Termination = TerminationExited
		execution.ExitCode = &code
		return
	}
	// A cancellation that raced an already-finished process returns
	// os.ErrProcessDone. Its actual ProcessState wins; reaching here means no
	// reliable terminal state was available.
	if cancel.called && errors.Is(cancel.err, os.ErrProcessDone) {
		execution.Termination = terminationWaitFailed
		return
	}
	execution.Termination = terminationWaitFailed
}
