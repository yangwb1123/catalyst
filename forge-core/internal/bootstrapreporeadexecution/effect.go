package bootstrapreporeadexecution

import (
	"context"
	"fmt"
	"os"
	"time"

	"forgeos/forge-core/internal/bootstraprepoexecutionauthority"
	"forgeos/forge-core/internal/grantstate"
	"forgeos/forge-core/internal/pinnedreporead"
)

func executeTransitions(deps dependencies, repositoryRoot string, session stateSession,
	snapshot grantstate.Snapshot, ledger *bootstraprepoexecutionauthority.Ledger,
	authorities authorityInputs, inputs executionInputs,
	signer *bootstraprepoexecutionauthority.Signer, startedAt int64) ([]byte, error) {
	snapshot, ledger, _, err := appendTransition(session, snapshot, ledger,
		authorities.ledger, inputs, nil, "reserved_no_repo_io", startedAt, "", signer,
		authorities.execution)
	if err != nil {
		return nil, err
	}
	repository, err := session.BindRepository(repositoryRoot)
	if err != nil {
		return nil, abandonReservation(deps.clock, session, snapshot, ledger,
			authorities, inputs, signer, err)
	}
	defer func() { _ = repository.Close() }()
	if err = session.VerifyRepository(); err != nil {
		return nil, abandonReservation(deps.clock, session, snapshot, ledger,
			authorities, inputs, signer, err)
	}
	if err = deps.preflightRepository(repository); err != nil {
		return nil, abandonReservation(deps.clock, session, snapshot, ledger,
			authorities, inputs, signer, err)
	}
	intentAt, err := currentTime(deps.clock, ledger)
	if err != nil {
		return nil, err
	}
	snapshot, ledger, _, err = appendTransition(session, snapshot, ledger,
		authorities.ledger, inputs, nil, "effect_intent", intentAt, "", signer,
		authorities.execution)
	if err != nil {
		return nil, err
	}
	monotonicStart := deps.monotonicNow()
	return readAndComplete(deps, session, repository, snapshot, ledger,
		authorities, inputs, signer, monotonicStart)
}

func abandonReservation(runtimeClock clock, session stateSession, snapshot grantstate.Snapshot,
	ledger *bootstraprepoexecutionauthority.Ledger, authorities authorityInputs,
	inputs executionInputs, signer *bootstraprepoexecutionauthority.Signer, cause error) error {
	recordedAt, err := currentTime(runtimeClock, ledger)
	if err != nil {
		return err
	}
	_, _, _, err = appendTransition(session, snapshot, ledger, authorities.ledger,
		inputs, nil, "quarantined", recordedAt, "orphaned_reserved_no_repo_io",
		signer, authorities.execution)
	if err != nil {
		return err
	}
	return coded(CodeRepositoryRejected, cause)
}

func readAndComplete(deps dependencies, session stateSession, repository *os.File,
	snapshot grantstate.Snapshot, ledger *bootstraprepoexecutionauthority.Ledger,
	authorities authorityInputs, inputs executionInputs,
	signer *bootstraprepoexecutionauthority.Signer, monotonicStart time.Time) ([]byte, error) {
	entries, limits := readPlan(inputs)
	ctx, cancel := context.WithTimeout(context.Background(),
		time.Duration(limitsTimeout(inputs))*time.Millisecond)
	defer cancel()
	if err := verifyRepositoryWithinDeadline(ctx, session); err != nil {
		return nil, finishReadFailure(deps.clock, session, snapshot, ledger,
			authorities, inputs, signer, readFailureCode(ctx,
				pinnedreporead.CodeIdentityChanged), err)
	}
	files, err := deps.readFiles(ctx, repository, entries, limits)
	if err != nil {
		clearFiles(files)
		return nil, finishReadFailure(deps.clock, session, snapshot, ledger,
			authorities, inputs, signer, readFailureCode(ctx, normalizeReadCode(err)), err)
	}
	defer clearFiles(files)
	if err = verifyRepositoryWithinDeadline(ctx, session); err != nil {
		return nil, finishReadFailure(deps.clock, session, snapshot, ledger,
			authorities, inputs, signer, readFailureCode(ctx,
				pinnedreporead.CodeIdentityChanged), err)
	}
	elapsed := deps.monotonicNow().Sub(monotonicStart).Milliseconds()
	if elapsed < 0 {
		return nil, coded(CodePersistenceUncertain,
			fmt.Errorf("monotonic execution clock regressed"))
	}
	return completeRead(deps.clock, session, snapshot, ledger, authorities,
		inputs, signer, files, elapsed)
}

func verifyRepositoryWithinDeadline(ctx context.Context, session stateSession) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if err := session.VerifyRepository(); err != nil {
		if timeout := ctx.Err(); timeout != nil {
			return timeout
		}
		return err
	}
	return ctx.Err()
}

func readFailureCode(ctx context.Context, fallback pinnedreporead.Code) pinnedreporead.Code {
	if ctx.Err() != nil {
		return pinnedreporead.CodeTimeoutExceeded
	}
	return fallback
}

func readPlan(inputs executionInputs) ([]pinnedreporead.ExpectedEntry, pinnedreporead.Limits) {
	manifestEntries := inputs.manifest.Entries()
	entries := make([]pinnedreporead.ExpectedEntry, 0, len(manifestEntries))
	for _, entry := range manifestEntries {
		entries = append(entries, pinnedreporead.ExpectedEntry{Bytes: entry.ContentBytes,
			ContentSHA256: entry.ContentSHA256, Kind: "regular", Path: entry.Path})
	}
	output, _ := inputs.invocation.ExecutionLimits()
	return entries, pinnedreporead.Limits{MaxFiles: pinnedreporead.MaxFiles,
		MaxFileBytes: pinnedreporead.MaxFileBytes, MaxTotalBytes: output}
}

func limitsTimeout(inputs executionInputs) int64 {
	_, timeout := inputs.invocation.ExecutionLimits()
	return timeout
}

func normalizeReadCode(err error) pinnedreporead.Code {
	code := pinnedreporead.ErrorCode(err)
	switch code {
	case pinnedreporead.CodeContentMismatch, pinnedreporead.CodeIdentityChanged,
		pinnedreporead.CodeReadFailed, pinnedreporead.CodeTimeoutExceeded:
		return code
	default:
		return pinnedreporead.CodeReadFailed
	}
}

func finishReadFailure(runtimeClock clock, session stateSession,
	snapshot grantstate.Snapshot, ledger *bootstraprepoexecutionauthority.Ledger,
	authorities authorityInputs, inputs executionInputs,
	signer *bootstraprepoexecutionauthority.Signer, reason pinnedreporead.Code,
	cause error) error {
	now, err := currentTime(runtimeClock, ledger)
	if err != nil {
		return coded(CodePersistenceUncertain, err)
	}
	return finishReadFailureAt(session, snapshot, ledger, authorities, inputs,
		signer, reason, cause, now)
}

func finishReadFailureAt(session stateSession, snapshot grantstate.Snapshot,
	ledger *bootstraprepoexecutionauthority.Ledger, authorities authorityInputs,
	inputs executionInputs, signer *bootstraprepoexecutionauthority.Signer,
	reason pinnedreporead.Code, cause error, recordedAt int64) error {
	var err error
	_, _, _, err = appendTransition(session, snapshot, ledger, authorities.ledger,
		inputs, nil, "failed_consumed", recordedAt, string(reason), signer,
		authorities.execution)
	if err != nil {
		return err
	}
	return coded(CodeRepositoryRejected, cause)
}

func completeRead(runtimeClock clock, session stateSession, snapshot grantstate.Snapshot,
	ledger *bootstraprepoexecutionauthority.Ledger, authorities authorityInputs,
	inputs executionInputs, signer *bootstraprepoexecutionauthority.Signer,
	files []pinnedreporead.File, elapsed int64) ([]byte, error) {
	completedAt, err := currentTime(runtimeClock, ledger)
	if err != nil {
		return nil, coded(CodePersistenceUncertain, fmt.Errorf("completion clock is unavailable"))
	}
	_, timeout := inputs.invocation.ExecutionLimits()
	if elapsed > timeout {
		return nil, finishReadFailureAt(session, snapshot, ledger, authorities, inputs,
			signer, pinnedreporead.CodeTimeoutExceeded,
			fmt.Errorf("cooperative read budget elapsed"), completedAt)
	}
	contents := make([][]byte, len(files))
	for index := range files {
		contents[index] = files[index].Content
	}
	result, err := bootstraprepoexecutionauthority.BuildResult(inputs.policy,
		inputs.invocation, inputs.manifest, contents, completedAt, elapsed)
	if err != nil {
		return nil, finishReadFailureAt(session, snapshot, ledger, authorities, inputs,
			signer, pinnedreporead.CodeReadFailed, err, completedAt)
	}
	return persistCompleted(session, snapshot, ledger, authorities, inputs,
		signer, result, completedAt)
}

func persistCompleted(session stateSession, snapshot grantstate.Snapshot,
	ledger *bootstraprepoexecutionauthority.Ledger, authorities authorityInputs,
	inputs executionInputs, signer *bootstraprepoexecutionauthority.Signer,
	result *bootstraprepoexecutionauthority.Result, completedAt int64) ([]byte, error) {
	metadata, err := bootstraprepoexecutionauthority.BuildMetadata(result)
	if err != nil {
		return nil, coded(CodePersistenceUncertain, err)
	}
	_, _, receipt, err := appendTransition(session, snapshot, ledger, authorities.ledger,
		inputs, metadata, "completed", completedAt, "", signer, authorities.execution)
	if err != nil {
		return nil, err
	}
	delivery, err := bootstraprepoexecutionauthority.BuildDelivery(
		"first_delivery", result, receipt, metadata)
	if err != nil {
		return nil, coded(CodePersistenceUncertain, err)
	}
	encoded, err := bootstraprepoexecutionauthority.CanonicalJSON(delivery)
	if err != nil || len(encoded) == 0 {
		return nil, coded(CodePersistenceUncertain, fmt.Errorf("first delivery encoding failed"))
	}
	return encoded, nil
}

func clearFiles(files []pinnedreporead.File) {
	for index := range files {
		clear(files[index].Content)
		files[index].Content = nil
	}
}
