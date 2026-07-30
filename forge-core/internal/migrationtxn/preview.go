package migrationtxn

import (
	"bytes"
	"fmt"
	"path/filepath"

	"forgeos/forge-core/internal/runlock"
)

const previewSnapshotAttempts = 3

type preparedSnapshot struct {
	result Result
	intent *promotionIntent
}

func previewOperation(root string, prepare prepareOperation) (Result, error) {
	var previous preparedSnapshot
	for attempt := 0; attempt < previewSnapshotAttempts; attempt++ {
		current, err := takePreparedSnapshot(root, prepare)
		if err != nil {
			return Result{}, err
		}
		if attempt > 0 && samePreparedSnapshot(previous, current) {
			return current.result, nil
		}
		previous = current
	}
	return Result{}, fmt.Errorf(
		"migrationtxn: repository changed while planning migration; retry",
	)
}

func takePreparedSnapshot(
	root string,
	prepare prepareOperation,
) (preparedSnapshot, error) {
	if err := requirePreviewIdle(root); err != nil {
		return preparedSnapshot{}, err
	}
	if err := ValidateExecutionState(root); err != nil {
		return preparedSnapshot{}, err
	}
	result, intent, err := prepare(root, realFileOps{})
	if err != nil {
		return preparedSnapshot{}, err
	}
	if err := requirePreviewIdle(root); err != nil {
		return preparedSnapshot{}, err
	}
	return preparedSnapshot{result: result, intent: intent}, nil
}

func requirePreviewIdle(root string) error {
	busy, err := runlock.Busy(root)
	if err != nil {
		return fmt.Errorf("migrationtxn: probe repository mutation lock: %w", err)
	}
	if busy {
		return fmt.Errorf(
			"migrationtxn: repository mutation lock %s is busy; "+
				"wait for the verified holder to finish; "+
				"do not unlink a contended lock file",
			filepath.Join(root, ".forge", "run.lock"),
		)
	}
	pending, err := pendingWithOps(root, realFileOps{})
	if err != nil {
		return err
	}
	if pending {
		return fmt.Errorf(
			"migrationtxn: pending migration requires its matching recovery: " +
				"`forge migrate --to-lifecycle production --apply` or " +
				"`forge migrate --to engineering --apply`",
		)
	}
	return nil
}

func samePreparedSnapshot(left, right preparedSnapshot) bool {
	leftResult, leftResultErr := marshalCanonical(left.result)
	rightResult, rightResultErr := marshalCanonical(right.result)
	if leftResultErr != nil || rightResultErr != nil ||
		!bytes.Equal(leftResult, rightResult) {
		return false
	}
	if left.intent == nil || right.intent == nil {
		return left.intent == nil && right.intent == nil
	}
	leftIntent, leftIntentErr := encodeIntent(*left.intent)
	rightIntent, rightIntentErr := encodeIntent(*right.intent)
	return leftIntentErr == nil &&
		rightIntentErr == nil &&
		bytes.Equal(leftIntent, rightIntent)
}
