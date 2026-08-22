package outputbinding

import (
	"fmt"
	"io"
)

// ValidateReceiptChain verifies a complete, zero-based in-memory journal view.
// It does not authenticate the file or make the observations authoritative.
func ValidateReceiptChain(receipts []AgentOutputReceipt) error {
	var prior string
	state := newReceiptChainState()
	for index, receipt := range receipts {
		if err := ValidateReceipt(receipt); err != nil {
			return fmt.Errorf("output binding: receipt chain item %d: %w", index, err)
		}
		wantSequence := int64(index + 1)
		if receipt.LedgerSequence != wantSequence {
			return fmt.Errorf("output binding: receipt chain item %d has non-contiguous sequence", index)
		}
		if index > 0 && (receipt.PriorReceiptSHA256 == nil || *receipt.PriorReceiptSHA256 != prior) {
			return fmt.Errorf("output binding: receipt chain item %d has a broken prior link", index)
		}
		if err := state.accept(receipt); err != nil {
			return fmt.Errorf("output binding: receipt chain item %d: %w", index, err)
		}
		prior = receipt.ReceiptSHA256
	}
	return nil
}

type receiptAttemptKey struct {
	runID, workflow, phase string
}

type receiptChainState struct {
	attempts   map[receiptAttemptKey]int64
	bindings   map[string]struct{}
	challenges map[string]struct{}
}

func newReceiptChainState() receiptChainState {
	return receiptChainState{
		attempts: make(map[receiptAttemptKey]int64), bindings: make(map[string]struct{}),
		challenges: make(map[string]struct{}),
	}
}

func (state receiptChainState) accept(receipt AgentOutputReceipt) error {
	if _, exists := state.challenges[receipt.Challenge]; exists {
		return fmt.Errorf("reuses a prior challenge")
	}
	if _, exists := state.bindings[receipt.BindingSHA256]; exists {
		return fmt.Errorf("reuses a prior preflight binding")
	}
	key := receiptAttemptKey{runID: receipt.RunID, workflow: receipt.Workflow, phase: receipt.Phase}
	if prior, exists := state.attempts[key]; exists && receipt.Attempt <= prior {
		return fmt.Errorf("attempt is not strictly increasing for its run/workflow/phase")
	}
	state.challenges[receipt.Challenge] = struct{}{}
	state.bindings[receipt.BindingSHA256] = struct{}{}
	state.attempts[key] = receipt.Attempt
	return nil
}

// AppendValidated writes one validated canonical JSONL record to an injected
// writer in one Write call. It provides no locking, fsync, append-mode, atomicity,
// or durability claim; the repository store must supply and verify those.
func AppendValidated(writer io.Writer, receipt AgentOutputReceipt) error {
	if writer == nil {
		return fmt.Errorf("output binding: append writer is required")
	}
	encoded, err := CanonicalReceiptJSON(receipt)
	if err != nil {
		return err
	}
	line := append(encoded, '\n')
	written, err := writer.Write(line)
	if err != nil {
		return fmt.Errorf("output binding: append receipt: %w", err)
	}
	if written != len(line) {
		return fmt.Errorf("output binding: append receipt: %w", io.ErrShortWrite)
	}
	return nil
}
