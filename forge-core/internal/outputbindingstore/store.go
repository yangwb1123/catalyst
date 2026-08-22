// Package outputbindingstore durably appends authority-neutral local output
// receipts. Callers must also hold Forge's repository run lock; this store adds
// in-process serialization and strict whole-chain verification.
package outputbindingstore

import (
	"bytes"
	"fmt"
	"path/filepath"
	"sync"

	"forgeos/forge-core/internal/outputbinding"
	"forgeos/forge-core/internal/statefs"
)

const (
	ledgerName            = "agent-output-receipts.jsonl"
	receiptAnchorName     = "agent-output-receipts.head.json"
	receiptAnchorFormat   = "forgeos.agent-output-receipt-head.v1"
	defaultMaxLedgerBytes = int64(128 << 20)
	defaultMaxReceipts    = 65_536
)

type Store struct {
	root       string
	mu         sync.Mutex
	maxBytes   int64
	maxEntries int
}

type ledgerLimits struct {
	bytes   int64
	entries int
}

type ledgerSnapshot struct {
	receipts []outputbinding.AgentOutputReceipt
	data     []byte
	bytes    int64
}

func New(root string) *Store {
	return &Store{root: root, maxBytes: defaultMaxLedgerBytes, maxEntries: defaultMaxReceipts}
}

func Path(root string) string {
	return filepath.Join(root, ".forge", ledgerName)
}

// Append fills the chain fields, seals draft and appends exactly one canonical
// line. A malformed existing ledger blocks the append. The outer Forge run lock
// is the cross-process serialization boundary.
func (store *Store) Append(draft outputbinding.AgentOutputReceipt) (outputbinding.AgentOutputReceipt, error) {
	if store == nil || store.root == "" {
		return outputbinding.AgentOutputReceipt{}, fmt.Errorf("output binding store: repository root is required")
	}
	store.mu.Lock()
	defer store.mu.Unlock()
	if err := validateDraftChainFields(draft); err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	limits := store.limits()
	snapshot, err := store.loadAnchoredLedger(limits)
	if err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	sealed, line, err := prepareAppend(draft, snapshot)
	if err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	if err := validateAppendCapacity(snapshot, int64(len(line)), limits); err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	if err := statefs.EnsurePrivateDir(filepath.Join(store.root, ".forge")); err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	next := append(bytes.Clone(snapshot.data), line...)
	// Publish the detached head first. If the process stops before replacing
	// the ledger, the ahead anchor makes every future read fail closed rather
	// than silently accepting a rolled-back prefix.
	if err := writeReceiptAnchor(store.root, next, len(snapshot.receipts)+1); err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	if err := commitLedger(Path(store.root), snapshot.data, line); err != nil {
		return outputbinding.AgentOutputReceipt{}, err
	}
	return sealed, nil
}

func (store *Store) limits() ledgerLimits {
	limits := ledgerLimits{bytes: store.maxBytes, entries: store.maxEntries}
	if limits.bytes <= 0 {
		limits.bytes = defaultMaxLedgerBytes
	}
	if limits.entries <= 0 {
		limits.entries = defaultMaxReceipts
	}
	return limits
}

func validateDraftChainFields(draft outputbinding.AgentOutputReceipt) error {
	if draft.LedgerSequence != 0 || draft.PriorReceiptSHA256 != nil || draft.ReceiptSHA256 != "" {
		return fmt.Errorf("output binding store: draft must not set ledger chain fields")
	}
	return nil
}

// Load verifies and returns the complete detached local receipt chain.
func (store *Store) Load() ([]outputbinding.AgentOutputReceipt, error) {
	if store == nil || store.root == "" {
		return nil, fmt.Errorf("output binding store: repository root is required")
	}
	store.mu.Lock()
	defer store.mu.Unlock()
	snapshot, err := store.loadAnchoredLedger(store.limits())
	return snapshot.receipts, err
}

func loadLedger(path string, limits ledgerLimits) (ledgerSnapshot, error) {
	data, present, err := statefs.ReadRegularUnmodified(path, limits.bytes)
	if err != nil {
		return ledgerSnapshot{}, fmt.Errorf("output binding store: read ledger: %w", err)
	}
	if !present || len(data) == 0 {
		return ledgerSnapshot{receipts: []outputbinding.AgentOutputReceipt{}, data: []byte{}}, nil
	}
	if data[len(data)-1] != '\n' {
		return ledgerSnapshot{}, fmt.Errorf("output binding store: ledger has a truncated final line")
	}
	if bytes.Count(data, []byte{'\n'}) > limits.entries {
		return ledgerSnapshot{}, fmt.Errorf("output binding store: ledger exceeds %d receipts", limits.entries)
	}
	lines := bytes.Split(data[:len(data)-1], []byte{'\n'})
	receipts := make([]outputbinding.AgentOutputReceipt, len(lines))
	for index, line := range lines {
		if len(line) == 0 {
			return ledgerSnapshot{}, fmt.Errorf("output binding store: ledger line %d is empty", index+1)
		}
		receipt, decodeErr := outputbinding.DecodeCanonicalReceipt(line)
		if decodeErr != nil {
			return ledgerSnapshot{}, fmt.Errorf("output binding store: ledger line %d: %w", index+1, decodeErr)
		}
		receipts[index] = receipt
	}
	if err := outputbinding.ValidateReceiptChain(receipts); err != nil {
		return ledgerSnapshot{}, fmt.Errorf("output binding store: invalid ledger chain: %w", err)
	}
	return ledgerSnapshot{receipts: receipts, data: bytes.Clone(data), bytes: int64(len(data))}, nil
}

func prepareAppend(draft outputbinding.AgentOutputReceipt,
	snapshot ledgerSnapshot) (outputbinding.AgentOutputReceipt, []byte, error) {
	draft.LedgerSequence = int64(len(snapshot.receipts) + 1)
	if len(snapshot.receipts) > 0 {
		prior := snapshot.receipts[len(snapshot.receipts)-1].ReceiptSHA256
		draft.PriorReceiptSHA256 = &prior
	}
	sealed, err := outputbinding.SealReceipt(draft)
	if err != nil {
		return outputbinding.AgentOutputReceipt{}, nil,
			fmt.Errorf("output binding store: seal receipt: %w", err)
	}
	candidate := make([]outputbinding.AgentOutputReceipt, len(snapshot.receipts)+1)
	copy(candidate, snapshot.receipts)
	candidate[len(snapshot.receipts)] = sealed
	if err := outputbinding.ValidateReceiptChain(candidate); err != nil {
		return outputbinding.AgentOutputReceipt{}, nil,
			fmt.Errorf("output binding store: candidate ledger chain: %w", err)
	}
	encoded, err := outputbinding.CanonicalReceiptJSON(sealed)
	if err != nil {
		return outputbinding.AgentOutputReceipt{}, nil, err
	}
	return sealed, append(encoded, '\n'), nil
}

func validateAppendCapacity(snapshot ledgerSnapshot, lineBytes int64, limits ledgerLimits) error {
	if len(snapshot.receipts) >= limits.entries {
		return fmt.Errorf("output binding store: ledger receipt limit %d reached", limits.entries)
	}
	if lineBytes < 1 || lineBytes > limits.bytes || snapshot.bytes > limits.bytes-lineBytes {
		return fmt.Errorf("output binding store: append would exceed %d ledger bytes", limits.bytes)
	}
	return nil
}

func commitLedger(path string, current, line []byte) error {
	next := make([]byte, 0, len(current)+len(line))
	next = append(next, current...)
	next = append(next, line...)
	if err := statefs.AtomicWrite(path, next, 0o600); err != nil {
		return fmt.Errorf("output binding store: atomically commit ledger: %w", err)
	}
	return nil
}
