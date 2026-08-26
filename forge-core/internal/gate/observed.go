package gate

import (
	"context"
	"fmt"
	"strings"

	"forgeos/forge-core/internal/execbound"
	producer "forgeos/forge-core/internal/localcommandobservationproducer"
)

// GateObservedWith explicitly opts one structural-gate spawn into ADR-0051
// local observation production. The ordinary GateWith path remains disabled.
func GateObservedWith(
	ctx context.Context,
	root, runID string,
	opts Options,
) (Result, *producer.Production, error) {
	return runObservedWith(ctx, "gate", root, runID, opts, producer.CommandGate)
}

// CheckObservedWith explicitly opts one governance-check spawn into ADR-0051
// local observation production. The ordinary CheckWith path remains disabled.
func CheckObservedWith(
	ctx context.Context,
	root, runID string,
	opts Options,
) (Result, *producer.Production, error) {
	return runObservedWith(ctx, "check", root, runID, opts, producer.CommandCheck)
}

// AcceptObservedWith explicitly opts one acceptance spawn into ADR-0051 local
// observation production. The ordinary AcceptWith path remains disabled.
func AcceptObservedWith(
	ctx context.Context,
	root, runID string,
	opts Options,
) (Result, *producer.Production, error) {
	return runObservedWith(ctx, "accept", root, runID, opts, producer.CommandAccept)
}

// ProbeAllObservedWith explicitly opts one acceptance --json spawn into
// ADR-0051 local observation production while preserving ProbeAllWith's exact
// 11-row/four-field, nonzero-exit and truncation semantics.
func ProbeAllObservedWith(
	ctx context.Context,
	root, runID string,
	opts Options,
) (
	statuses map[string]string,
	categories map[string]string,
	err error,
	production *producer.Production,
	observationError error,
) {
	if err := opts.Validate(); err != nil {
		legacyErr := fmt.Errorf("gate: invalid options: %v", err)
		return nil, nil, legacyErr, nil, err
	}
	runCtx, cancel, deadline := gateDeadlineContext(ctx, opts)
	defer cancel()
	release, ok := acquireSpawnSlot(runCtx)
	if !ok {
		if runCtx.Err() == context.DeadlineExceeded {
			err := fmt.Errorf("gate: acceptance --json timed out %s before spawn", deadline)
			return nil, nil, err, nil, context.DeadlineExceeded
		}
		err := fmt.Errorf("gate: acceptance --json canceled")
		return nil, nil, err, nil, context.Canceled
	}
	defer release()

	run := producer.Run(runCtx, RepoRoot(root), producer.CommandProbeAll, runID, opts, execbound.CaptureSplit)
	statuses, categories, err = probeFromExecution(run.Execution.Legacy, deadline)
	return statuses, categories, err, run.Production, run.ObservationError
}

func runObservedWith(
	ctx context.Context,
	name, root, runID string,
	opts Options,
	class string,
) (Result, *producer.Production, error) {
	if err := opts.Validate(); err != nil {
		legacy := newResult(name, false, fmt.Sprintf("gate: invalid options: %v", err))
		return legacy, nil, err
	}
	runCtx, cancel, deadline := gateDeadlineContext(ctx, opts)
	defer cancel()
	release, ok := acquireSpawnSlot(runCtx)
	if !ok {
		if runCtx.Err() == context.DeadlineExceeded {
			legacy := newResult(name, false,
				fmt.Sprintf("gate: timed out %s before spawn", deadline))
			return legacy, nil, context.DeadlineExceeded
		}
		return newResult(name, false, "gate: canceled"), nil, context.Canceled
	}
	defer release()

	run := producer.Run(runCtx, RepoRoot(root), class, runID, opts, execbound.CaptureCombined)
	return legacyResultFromExecution(name, deadline, run.Execution.Legacy),
		run.Production, run.ObservationError
}

func legacyResultFromExecution(name, deadline string, res execbound.Result) Result {
	switch {
	case res.TimedOut():
		return newResult(name, false,
			strings.TrimSpace(res.Rendered())+fmt.Sprintf(" …[timed out %s]", deadline))
	case res.CtxErr == context.Canceled:
		return newResult(name, false, strings.TrimSpace(res.Rendered())+" …[canceled]")
	default:
		return newResult(name, res.Err == nil, strings.TrimSpace(res.Rendered()))
	}
}

func probeFromExecution(
	res execbound.Result,
	deadline string,
) (statuses map[string]string, categories map[string]string, err error) {
	switch {
	case res.TimedOut():
		return nil, nil, fmt.Errorf("gate: acceptance --json timed out %s: %w", deadline, res.Err)
	case res.CtxErr == context.Canceled:
		return nil, nil, fmt.Errorf("gate: acceptance --json canceled")
	}
	if res.Total > int64(res.Retained) {
		return nil, nil, fmt.Errorf("gate: parsing acceptance --json: output truncated: retained %d of %d bytes",
			res.Retained, res.Total)
	}
	rejected, exitErr := validateProbeExit(res)
	if exitErr != nil {
		return nil, nil, exitErr
	}
	rows, decodeErr := decodeProbeRows(res.Stdout)
	if decodeErr != nil {
		return nil, nil, fmt.Errorf("gate: parsing acceptance --json: %w", decodeErr)
	}
	if rejected && allProbeRowsPass(rows) {
		return nil, nil, fmt.Errorf("gate: acceptance --json exited nonzero with an all-PASS envelope")
	}
	statuses = make(map[string]string, len(rows))
	categories = make(map[string]string, len(rows))
	for _, row := range rows {
		statuses[row.Criterion] = normStatus(row.Status)
		categories[row.Criterion] = row.Category
	}
	return statuses, categories, nil
}
