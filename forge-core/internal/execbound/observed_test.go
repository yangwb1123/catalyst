package execbound

import (
	"context"
	"crypto/sha256"
	"errors"
	"math"
	"os"
	"os/exec"
	"reflect"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestObservedCapture_TaggedDrainOrderAndDigests(t *testing.T) {
	capture := newObservedCapture(3)
	stdout := capture.writer(streamStdout)
	stderr := capture.writer(streamStderr)
	_, _ = stdout.Write([]byte("ab"))
	_, _ = stderr.Write([]byte("XYZ"))
	_, _ = stdout.Write([]byte("cd"))

	out, errOut, combined := capture.snapshots()
	assertStreamObservation(t, out, "abcd", "abc")
	assertStreamObservation(t, errOut, "XYZ", "XYZ")
	assertStreamObservation(t, combined, "abXYZcd", "abX")
	if combined.Bytes != out.Bytes+errOut.Bytes {
		t.Errorf("combined bytes = %d, stdout+stderr = %d", combined.Bytes, out.Bytes+errOut.Bytes)
	}
}

func TestObservedCapture_EmptyAndCountOverflow(t *testing.T) {
	emptyStream := newObservedStream(8)
	empty := emptyStream.snapshot()
	emptyDigest := sha256.Sum256(nil)
	if empty.Bytes != 0 || len(empty.Retained) != 0 || empty.SHA256 != emptyDigest ||
		empty.RetainedSHA256 != emptyDigest || empty.CountOverflow {
		t.Errorf("empty observation is not canonical: %+v", empty)
	}

	stream := newObservedStream(8)
	stream.bytes = math.MaxInt64
	stream.write([]byte("x"))
	overflow := stream.snapshot()
	if overflow.Bytes != uint64(math.MaxInt64)+1 || !overflow.CountOverflow {
		t.Errorf("signed wire overflow not marked: bytes=%d overflow=%v", overflow.Bytes, overflow.CountOverflow)
	}
}

func TestObservedCapture_ConcurrentWritersAreSerialized(t *testing.T) {
	const writes = 100
	const chunk = "abcd"
	capture := newObservedCapture(writes * len(chunk) * 2)
	writers := []*taggedWriter{capture.writer(streamStdout), capture.writer(streamStderr)}
	var wait sync.WaitGroup
	for _, writer := range writers {
		wait.Add(1)
		go func(writer *taggedWriter) {
			defer wait.Done()
			for range writes {
				_, _ = writer.Write([]byte(chunk))
			}
		}(writer)
	}
	wait.Wait()
	stdout, stderr, combined := capture.snapshots()
	wantPerStream := strings.Repeat(chunk, writes)
	assertStreamObservation(t, stdout, wantPerStream, wantPerStream)
	assertStreamObservation(t, stderr, wantPerStream, wantPerStream)
	if combined.Bytes != uint64(len(wantPerStream)*2) || len(combined.Retained) != len(wantPerStream)*2 {
		t.Errorf("combined count/retention = %d/%d", combined.Bytes, len(combined.Retained))
	}
	if combined.SHA256 != sha256.Sum256(combined.Retained) {
		t.Error("combined digest must cover the exact mutex-serialized retained bytes")
	}
}

func TestRunObserved_SplitCapturesFullStreamsAndLegacyProjection(t *testing.T) {
	result := RunObserved(context.Background(),
		[]string{"sh", "-c", "printf stdout; printf stderr >&2; exit 3"},
		Options{MaxOutputBytes: 4}, CaptureSplit, Spec{}, ObservationOptions{})

	if result.Legacy.Err == nil {
		t.Fatal("nonzero exit must retain its legacy error")
	}
	if result.Execution.Termination != TerminationExited || result.Execution.ExitCode == nil ||
		*result.Execution.ExitCode != 3 {
		t.Errorf("termination = %+v", result.Execution)
	}
	if !result.Execution.Started || !result.Execution.DrainComplete {
		t.Errorf("started/drain complete = %v/%v", result.Execution.Started, result.Execution.DrainComplete)
	}
	assertStreamObservation(t, result.Stdout, "stdout", "stdo")
	assertStreamObservation(t, result.Stderr, "stderr", "stde")
	if result.Combined.Bytes != 12 || result.Legacy.Total != 12 || result.Legacy.Retained != 8 {
		t.Errorf("counts combined/legacy = %d/%d/%d", result.Combined.Bytes, result.Legacy.Total, result.Legacy.Retained)
	}
	if string(result.Legacy.Stdout) != "stdo" || string(result.Legacy.Stderr) != "stde" || result.Legacy.Merged != nil {
		t.Errorf("split legacy projection drifted: %+v", result.Legacy)
	}
}

func TestRunObserved_CombinedProjectionAndInjectedClock(t *testing.T) {
	start := time.Unix(1_700_000_000, 123_000_000)
	end := start.Add(250 * time.Millisecond)
	times := []time.Time{start, end}
	clockReads := 0
	result := RunObserved(context.Background(), []string{"printf", "hello"},
		Options{MaxOutputBytes: 3}, CaptureCombined, Spec{}, ObservationOptions{
			Now: func() time.Time {
				value := times[clockReads]
				clockReads++
				return value
			},
		})
	if result.Legacy.Err != nil {
		t.Fatalf("printf: %v", result.Legacy.Err)
	}
	if clockReads != 2 || !result.Execution.StartedAt.Equal(start) || !result.Execution.EndedAt.Equal(end) {
		t.Errorf("clock reads/times = %d/%v/%v", clockReads, result.Execution.StartedAt, result.Execution.EndedAt)
	}
	if result.Execution.Termination != TerminationExited || result.Execution.ExitCode == nil || *result.Execution.ExitCode != 0 {
		t.Errorf("termination = %+v", result.Execution)
	}
	assertStreamObservation(t, result.Stdout, "hello", "hel")
	assertStreamObservation(t, result.Stderr, "", "")
	assertStreamObservation(t, result.Combined, "hello", "hel")
	if string(result.Legacy.Merged) != "hel" || result.Legacy.Total != 5 || result.Legacy.Retained != 3 {
		t.Errorf("combined legacy projection = %+v", result.Legacy)
	}
}

func TestRunObserved_NotStartedAndSpawnFailed(t *testing.T) {
	now := time.Unix(1_700_000_000, 0)
	notStarted := RunObserved(context.Background(), []string{"true"}, Options{Timeout: -1},
		CaptureCombined, Spec{}, ObservationOptions{Now: func() time.Time { return now }})
	if notStarted.Execution.Started || notStarted.Execution.Termination != TerminationNotStarted ||
		notStarted.Legacy.Err == nil || !notStarted.Execution.EndedAt.Equal(now) {
		t.Errorf("invalid options result = %+v", notStarted)
	}

	spawnFailed := RunObserved(context.Background(), []string{"forge-no-such-observed-binary"}, Options{},
		CaptureCombined, Spec{}, ObservationOptions{Now: func() time.Time { return now }})
	if spawnFailed.Execution.Started || spawnFailed.Execution.Termination != TerminationSpawnFailed ||
		spawnFailed.Legacy.Err == nil || !errors.Is(spawnFailed.Legacy.Err, exec.ErrNotFound) {
		t.Errorf("spawn failure result = %+v", spawnFailed)
	}
	if !spawnFailed.Execution.StartedAt.IsZero() || spawnFailed.Execution.DrainComplete {
		t.Errorf("spawn failure must have zero start/incomplete drain: %+v", spawnFailed.Execution)
	}
}

func TestRunObserved_PreCancelledAndInvalidCWDDoNotStart(t *testing.T) {
	ctx, cancel := context.WithCancel(context.Background())
	cancel()
	preCancelled := RunObserved(ctx, []string{"true"}, Options{Unbounded: true},
		CaptureCombined, Spec{}, ObservationOptions{})
	if preCancelled.Execution.Started || preCancelled.Execution.Termination != TerminationSpawnFailed ||
		!errors.Is(preCancelled.Legacy.Err, context.Canceled) ||
		!errors.Is(preCancelled.Legacy.CtxErr, context.Canceled) {
		t.Errorf("pre-cancelled spawn result = %+v", preCancelled)
	}

	invalidCWD := RunObserved(context.Background(), []string{"true"}, Options{},
		CaptureCombined, Spec{Dir: "/forgeos/execbound/no-such-directory"}, ObservationOptions{})
	if invalidCWD.Execution.Started || invalidCWD.Execution.Termination != TerminationSpawnFailed ||
		invalidCWD.Legacy.Err == nil {
		t.Errorf("invalid cwd spawn result = %+v", invalidCWD)
	}
}

func TestRun_ExecutablePathOverridesFailedLookup(t *testing.T) {
	sh, err := exec.LookPath("sh")
	if err != nil {
		t.Skip("sh unavailable")
	}
	result := Run(context.Background(), []string{"forge-logical-shell-name", "-c", "printf override"},
		Options{}, CaptureCombined, Spec{ExecutablePath: sh})
	if result.Err != nil || result.Observed() != "override" {
		t.Errorf("resolved executable override failed: err=%v output=%q", result.Err, result.Observed())
	}
}

func TestRunObserved_ExecutablePathOverridesFailedLookup(t *testing.T) {
	sh, err := exec.LookPath("sh")
	if err != nil {
		t.Skip("sh unavailable")
	}
	result := RunObserved(context.Background(),
		[]string{"forge-logical-observed-shell", "-c", "printf observed-override"},
		Options{}, CaptureCombined, Spec{ExecutablePath: sh}, ObservationOptions{})
	if result.Legacy.Err != nil || result.Legacy.Observed() != "observed-override" ||
		result.Execution.Termination != TerminationExited || !result.Execution.DrainComplete {
		t.Errorf("resolved observed executable override failed: err=%v output=%q execution=%+v",
			result.Legacy.Err, result.Legacy.Observed(), result.Execution)
	}
}

func TestRunObserved_TimeoutAndCancellationAreDistinct(t *testing.T) {
	timedOut := RunObserved(context.Background(), []string{"sleep", "30"},
		Options{Timeout: 50 * time.Millisecond}, CaptureCombined, Spec{}, ObservationOptions{})
	if timedOut.Execution.Termination != TerminationTimedOut || timedOut.Execution.ExitCode != nil ||
		!timedOut.Execution.Started || timedOut.Legacy.Err == nil {
		t.Errorf("timeout result = %+v", timedOut.Execution)
	}

	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan ObservedResult, 1)
	go func() {
		done <- RunObserved(ctx, []string{"sleep", "30"}, Options{Unbounded: true},
			CaptureCombined, Spec{}, ObservationOptions{})
	}()
	time.Sleep(50 * time.Millisecond)
	cancel()
	select {
	case cancelled := <-done:
		if cancelled.Execution.Termination != TerminationCancelled || cancelled.Execution.ExitCode != nil ||
			!cancelled.Execution.Started || cancelled.Legacy.Err == nil {
			t.Errorf("cancel result = %+v", cancelled.Execution)
		}
	case <-time.After(5 * time.Second):
		t.Fatal("cancelled observed run did not return")
	}
}

func TestObservedTermination_ProcessDoneCancellationDoesNotHideExit(t *testing.T) {
	cmd := exec.Command("sh", "-c", "exit 7")
	waitErr := cmd.Run()
	var execution ExecutionObservation
	classifyObservedTermination(&execution, cmd, waitErr, cancelSnapshot{
		called: true, cause: context.Canceled, err: os.ErrProcessDone,
	})
	if execution.Termination != TerminationExited || execution.ExitCode == nil || *execution.ExitCode != 7 {
		t.Errorf("racing process-done cancellation hid actual exit: %+v", execution)
	}
}

func TestLegacyCountSaturatesWithoutWrapping(t *testing.T) {
	if got := legacyCount(math.MaxInt64, 1); got != math.MaxInt64 {
		t.Errorf("legacyCount overflow = %d", got)
	}
}

func assertStreamObservation(t *testing.T, got StreamObservation, full, retained string) {
	t.Helper()
	if got.Bytes != uint64(len(full)) || string(got.Retained) != retained {
		t.Errorf("stream count/prefix = %d/%q, want %d/%q", got.Bytes, got.Retained, len(full), retained)
	}
	if got.SHA256 != sha256.Sum256([]byte(full)) || got.RetainedSHA256 != sha256.Sum256([]byte(retained)) {
		t.Errorf("stream digest mismatch for full=%q retained=%q", full, retained)
	}
	if got.CountOverflow {
		t.Error("small stream unexpectedly marked CountOverflow")
	}
}

func TestStreamObservationSnapshotIsDefensive(t *testing.T) {
	stream := newObservedStream(4)
	stream.write([]byte("data"))
	first := stream.snapshot()
	first.Retained[0] = 'X'
	second := stream.snapshot()
	if reflect.DeepEqual(first.Retained, second.Retained) || string(second.Retained) != "data" {
		t.Errorf("snapshot retained aliases mutable state: first=%q second=%q", first.Retained, second.Retained)
	}
}

func TestObservedResultLegacyAndObservationBytesDoNotAlias(t *testing.T) {
	result := RunObserved(context.Background(),
		[]string{"sh", "-c", "printf stdout; printf stderr >&2"},
		Options{}, CaptureSplit, Spec{}, ObservationOptions{})
	if result.Legacy.Err != nil {
		t.Fatal(result.Legacy.Err)
	}
	wantStdout := append([]byte(nil), result.Stdout.Retained...)
	wantStderr := append([]byte(nil), result.Stderr.Retained...)
	result.Legacy.Stdout[0] = 'X'
	result.Legacy.Stderr[0] = 'Y'
	if !reflect.DeepEqual(result.Stdout.Retained, wantStdout) ||
		!reflect.DeepEqual(result.Stderr.Retained, wantStderr) {
		t.Fatalf("legacy mutation changed observation bytes: stdout=%q stderr=%q",
			result.Stdout.Retained, result.Stderr.Retained)
	}
	result.Stdout.Retained[0] = 'Z'
	if result.Legacy.Stdout[0] != 'X' {
		t.Fatalf("observation mutation changed legacy bytes: %q", result.Legacy.Stdout)
	}
}
