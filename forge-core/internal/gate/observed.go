package gate

import (
	"context"
	"encoding/json"
	"fmt"
	"os/exec"
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
// non-zero-exit, JSON parsing and truncation semantics.
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
	release, ok := acquireSpawnSlot(ctx)
	if !ok {
		err := fmt.Errorf("gate: acceptance --json canceled")
		return nil, nil, err, nil, context.Canceled
	}
	defer release()

	run := producer.Run(ctx, RepoRoot(root), producer.CommandProbeAll, runID, opts, execbound.CaptureSplit)
	statuses, categories, err = probeFromExecution(run.Execution.Legacy, opts)
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
	release, ok := acquireSpawnSlot(ctx)
	if !ok {
		return newResult(name, false, "gate: canceled"), nil, context.Canceled
	}
	defer release()

	run := producer.Run(ctx, RepoRoot(root), class, runID, opts, execbound.CaptureCombined)
	return legacyResultFromExecution(name, opts, run.Execution.Legacy),
		run.Production, run.ObservationError
}

func legacyResultFromExecution(name string, opts Options, res execbound.Result) Result {
	switch {
	case res.TimedOut():
		return newResult(name, false,
			strings.TrimSpace(res.Rendered())+fmt.Sprintf(" …[timed out after %s%s]", effectiveDeadline(opts), knobClause(opts)))
	case res.CtxErr == context.Canceled:
		return newResult(name, false, strings.TrimSpace(res.Rendered())+" …[canceled]")
	default:
		return newResult(name, res.Err == nil, strings.TrimSpace(res.Rendered()))
	}
}

func probeFromExecution(
	res execbound.Result,
	opts Options,
) (statuses map[string]string, categories map[string]string, err error) {
	switch {
	case res.TimedOut():
		return nil, nil, fmt.Errorf("gate: acceptance --json timed out after %s%s: %w",
			effectiveDeadline(opts), knobClause(opts), res.Err)
	case res.CtxErr == context.Canceled:
		return nil, nil, fmt.Errorf("gate: acceptance --json canceled")
	}
	if res.Err != nil {
		if _, ok := res.Err.(*exec.ExitError); !ok || len(res.Stdout) == 0 {
			return nil, nil, fmt.Errorf("gate: acceptance --json failed: %w (%s)", res.Err, exitStderr(res.Stderr))
		}
	}
	var rows []probeRow
	if err := json.Unmarshal(res.Stdout, &rows); err != nil {
		if res.Total > int64(res.Retained) {
			return nil, nil, fmt.Errorf("gate: parsing acceptance --json: %w (output truncated: retained %d of %d bytes)",
				err, res.Retained, res.Total)
		}
		return nil, nil, fmt.Errorf("gate: parsing acceptance --json: %w", err)
	}
	statuses = make(map[string]string, len(rows))
	categories = make(map[string]string, len(rows))
	for _, row := range rows {
		statuses[row.Criterion] = normStatus(row.Status)
		categories[row.Criterion] = row.Category
	}
	return statuses, categories, nil
}
