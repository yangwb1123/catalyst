package localcommandobservationproducer

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"math"
	"regexp"
	"time"

	commandcontract "forgeos/forge-core/internal/commandobservationevidencecontract"
	"forgeos/forge-core/internal/execbound"
)

var runIDPattern = regexp.MustCompile(`^[a-z0-9][a-z0-9._:/-]*$`)

const postCancellationProfileGrace = time.Second

// RunResult keeps the actual process result separate from observation
// production. A command that started retains its ordinary legacy result even
// when post-execution source/tool stability prevents production.
type RunResult struct {
	Execution        execbound.ObservedResult
	Production       *Production
	ObservationError error
}

// Run explicitly opts one closed-profile local gate command into observation
// production. It performs all fallible profile work before the single spawn,
// executes the resolved executable path in the exact scrubbed environment,
// then fails closed if lifecycle, stream, tool, or source facts are incomplete.
// The pre/post byte snapshots detect ordinary drift but, as ADR-0051 records,
// do not turn the local read-to-exec path into executable pinning. It does not
// interpret command output or exit status as a gate verdict.
func Run(
	ctx context.Context,
	root, class, runID string,
	opts execbound.Options,
	capture execbound.Capture,
) RunResult {
	timeoutMS, err := validateRunInputs(class, runID, opts, capture)
	if err != nil {
		return preflightFailure(err)
	}
	prepared, err := prepareProfiles(ctx, root, class, timeoutMS)
	if err != nil {
		return preflightFailure(fmt.Errorf("prepare command observation profiles: %w", err))
	}
	execution := execbound.RunObserved(
		ctx,
		prepared.Command.Argv,
		opts,
		capture,
		execbound.Spec{
			Dir: prepared.Root, Env: append([]string(nil), prepared.ChildEnvironment...),
			ExecutablePath: prepared.Tool.FinalPath,
		},
		execbound.ObservationOptions{},
	)
	wireCapture, err := captureFromExecution(execution)
	if err != nil {
		return RunResult{Execution: execution, ObservationError: err}
	}
	sealContext, stopSealing := postExecutionProfileContext(ctx, execution.Execution.Termination)
	defer stopSealing()
	production, err := buildProduction(sealContext, prepared, runID, wireCapture)
	if err != nil {
		return RunResult{
			Execution:        execution,
			ObservationError: fmt.Errorf("seal local command observation production: %w", err),
		}
	}
	return RunResult{Execution: execution, Production: production}
}

func validateRunInputs(
	class, runID string,
	opts execbound.Options,
	capture execbound.Capture,
) (*int64, error) {
	if err := ensureSupportedPlatform(); err != nil {
		return nil, err
	}
	if err := opts.Validate(); err != nil {
		return nil, fmt.Errorf("invalid execution options: %w", err)
	}
	if len(runID) > 160 || !runIDPattern.MatchString(runID) {
		return nil, fmt.Errorf("run_id is not a valid command-observation identifier")
	}
	if _, err := commandForClass(class); err != nil {
		return nil, err
	}
	wantCapture := execbound.CaptureCombined
	if class == CommandProbeAll {
		wantCapture = execbound.CaptureSplit
	}
	if capture != wantCapture {
		return nil, fmt.Errorf("command class %q requires capture mode %d", class, wantCapture)
	}
	if opts.Unbounded {
		return nil, nil
	}
	timeout := opts.Timeout
	if timeout == 0 {
		timeout = execbound.DefaultTimeout
	}
	if timeout%time.Millisecond != 0 {
		return nil, fmt.Errorf("timeout must be an exact whole number of milliseconds")
	}
	milliseconds := timeout / time.Millisecond
	if milliseconds < 1 || milliseconds > 86_400_000 {
		return nil, fmt.Errorf("timeout_ms must be integer 1..86400000")
	}
	result := int64(milliseconds)
	return &result, nil
}

func postExecutionProfileContext(
	parent context.Context,
	termination execbound.TerminationKind,
) (context.Context, context.CancelFunc) {
	if termination != execbound.TerminationCancelled && termination != execbound.TerminationTimedOut {
		return context.WithCancel(parent)
	}
	// A controller cancellation may itself be the valid captured termination.
	// Detach only when the observation proves cancellation/timeout, and only for
	// one short fixed window. A caller cancellation racing with natural exit
	// remains fail-closed instead of being silently ignored during sealing.
	if parent.Err() != nil {
		return context.WithTimeout(context.WithoutCancel(parent), postCancellationProfileGrace)
	}
	return context.WithTimeout(parent, postCancellationProfileGrace)
}

func preflightFailure(err error) RunResult {
	wrapped := fmt.Errorf("local command observation preflight: %w", err)
	return RunResult{
		Execution: execbound.ObservedResult{
			Legacy: execbound.Result{Err: wrapped},
			Execution: execbound.ExecutionObservation{
				Termination: execbound.TerminationNotStarted,
			},
		},
		ObservationError: wrapped,
	}
}

func captureFromExecution(execution execbound.ObservedResult) (capture, error) {
	if !execution.Execution.Started {
		return capture{}, fmt.Errorf("local command did not start; no observation produced")
	}
	if !execution.Execution.DrainComplete {
		return capture{}, fmt.Errorf("local command stream drain was incomplete; no observation produced")
	}
	started, ended, err := observationTimes(execution.Execution.StartedAt, execution.Execution.EndedAt)
	if err != nil {
		return capture{}, err
	}
	stdout, err := wireStream("stdout", execution.Stdout)
	if err != nil {
		return capture{}, err
	}
	stderr, err := wireStream("stderr", execution.Stderr)
	if err != nil {
		return capture{}, err
	}
	combined, err := wireStream("combined", execution.Combined)
	if err != nil {
		return capture{}, err
	}
	if stdout.Bytes > math.MaxInt64-stderr.Bytes || combined.Bytes != stdout.Bytes+stderr.Bytes {
		return capture{}, fmt.Errorf("observed stream byte counts are inconsistent")
	}
	termination, err := wireTermination(execution.Execution)
	if err != nil {
		return capture{}, err
	}
	return capture{
		StartedAtUnixMS: started, EndedAtUnixMS: ended,
		Streams:     commandcontract.Streams{Combined: combined, Stderr: stderr, Stdout: stdout},
		Termination: termination,
	}, nil
}

func observationTimes(started, ended time.Time) (int64, int64, error) {
	if started.IsZero() || ended.IsZero() {
		return 0, 0, fmt.Errorf("local command observation timestamps are incomplete")
	}
	startedMS, endedMS := started.UnixMilli(), ended.UnixMilli()
	if startedMS < 0 || endedMS < startedMS {
		return 0, 0, fmt.Errorf("local command observation timestamps are noncanonical")
	}
	return startedMS, endedMS, nil
}

func wireStream(name string, observed execbound.StreamObservation) (commandcontract.Stream, error) {
	if observed.CountOverflow || observed.Bytes > math.MaxInt64 {
		return commandcontract.Stream{}, fmt.Errorf("%s stream byte count exceeds signed int64", name)
	}
	if uint64(len(observed.Retained)) > observed.Bytes {
		return commandcontract.Stream{}, fmt.Errorf("%s retained prefix exceeds full stream", name)
	}
	retainedDigest := sha256.Sum256(observed.Retained)
	if retainedDigest != observed.RetainedSHA256 {
		return commandcontract.Stream{}, fmt.Errorf("%s retained prefix digest is inconsistent", name)
	}
	return commandcontract.Stream{
		Bytes: int64(observed.Bytes), RetainedBytes: int64(len(observed.Retained)),
		RetainedSHA256: hex.EncodeToString(observed.RetainedSHA256[:]),
		SHA256:         hex.EncodeToString(observed.SHA256[:]),
	}, nil
}

func wireTermination(execution execbound.ExecutionObservation) (commandcontract.Termination, error) {
	switch execution.Termination {
	case execbound.TerminationExited:
		if execution.ExitCode == nil || *execution.ExitCode < 0 || *execution.ExitCode > math.MaxInt32 {
			return commandcontract.Termination{}, fmt.Errorf("exited command lacks canonical exit code")
		}
		exitCode := *execution.ExitCode
		return commandcontract.Termination{ExitCode: &exitCode, Kind: "exited"}, nil
	case execbound.TerminationTimedOut:
		return commandcontract.Termination{Kind: "timed_out"}, nil
	case execbound.TerminationCancelled:
		return commandcontract.Termination{Kind: "cancelled"}, nil
	default:
		return commandcontract.Termination{}, fmt.Errorf(
			"unsupported local command termination %q; no observation produced",
			execution.Termination,
		)
	}
}
