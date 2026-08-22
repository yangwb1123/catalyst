package outputbindingstore

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"forgeos/forge-core/internal/outputbinding"
)

func TestDefaultLedgerLimitsAreFrozen(t *testing.T) {
	store := New(t.TempDir())
	limits := store.limits()
	if limits.bytes != 128<<20 || limits.entries != 65_536 {
		t.Fatalf("default ledger limits = %d bytes, %d entries", limits.bytes, limits.entries)
	}
}

func TestProductionByteBoundaryArithmeticIsInclusive(t *testing.T) {
	limits := ledgerLimits{bytes: defaultMaxLedgerBytes, entries: defaultMaxReceipts}
	snapshot := ledgerSnapshot{receipts: []outputbinding.AgentOutputReceipt{},
		bytes: defaultMaxLedgerBytes - 4096}
	if err := validateAppendCapacity(snapshot, 4096, limits); err != nil {
		t.Fatalf("exact 128 MiB boundary was rejected: %v", err)
	}
	if err := validateAppendCapacity(snapshot, 4097, limits); err == nil {
		t.Fatal("one byte past 128 MiB boundary was accepted")
	}
	if err := validateAppendCapacity(ledgerSnapshot{}, defaultMaxLedgerBytes+1, limits); err == nil {
		t.Fatal("single oversized receipt line was accepted")
	}
}

func TestAppendByteLimitAllowsExactLineAndRejectsOneByteOverBeforeWrite(t *testing.T) {
	root := t.TempDir()
	draft := receiptDraft(t, "exact")
	line := canonicalNextLine(t, draft, 1, nil)
	store := New(root)
	store.maxBytes = int64(len(line) - 1)
	if _, err := store.Append(draft); err == nil || !strings.Contains(err.Error(), "would exceed") {
		t.Fatalf("over-limit append error = %v", err)
	}
	if _, err := os.Lstat(filepath.Join(root, ".forge")); !os.IsNotExist(err) {
		t.Fatalf("rejected append created control state: %v", err)
	}
	store.maxBytes = int64(len(line))
	if _, err := store.Append(draft); err != nil {
		t.Fatalf("exact-limit append: %v", err)
	}
	assertLedgerBytes(t, Path(root), line)
}

func TestAppendChecksExistingBytesPlusNewLineBeforeOpeningLedger(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	first, err := store.Append(receiptDraft(t, "first"))
	if err != nil {
		t.Fatal(err)
	}
	path := Path(root)
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(path, 0o640); err != nil {
		t.Fatal(err)
	}
	draft := receiptDraft(t, "second")
	line := canonicalNextLine(t, draft, 2, &first.ReceiptSHA256)
	store.maxBytes = int64(len(before) + len(line) - 1)
	if _, err := store.Append(draft); err == nil || !strings.Contains(err.Error(), "would exceed") {
		t.Fatalf("combined over-limit append error = %v", err)
	}
	assertLedgerImage(t, path, before, 0o640)
	store.maxBytes++
	if _, err := store.Append(draft); err != nil {
		t.Fatalf("combined exact-limit append: %v", err)
	}
	assertLedgerImage(t, path, append(bytes.Clone(before), line...), 0o600)
}

func TestAppendReceiptCountLimitPreservesExactLedger(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	store.maxEntries = 1
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(Path(root))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := store.Append(receiptDraft(t, "second")); err == nil ||
		!strings.Contains(err.Error(), "receipt limit 1 reached") {
		t.Fatalf("count-limit append error = %v", err)
	}
	assertLedgerBytes(t, Path(root), before)
	receipts, err := store.Load()
	if err != nil || len(receipts) != 1 {
		t.Fatalf("exact count-limit load = %d, %v", len(receipts), err)
	}
}

func TestAppendRejectsLedgerAlreadyOverReceiptLimitWithoutMutation(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	store.maxEntries = 2
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	if _, err := store.Append(receiptDraft(t, "second")); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(Path(root))
	if err != nil {
		t.Fatal(err)
	}
	store.maxEntries = 1
	if _, err := store.Append(receiptDraft(t, "blocked")); err == nil ||
		!strings.Contains(err.Error(), "exceeds 1 receipts") {
		t.Fatalf("existing count overflow error = %v", err)
	}
	assertLedgerBytes(t, Path(root), before)
}

func TestAppendRejectsAlreadyOversizedLedgerWithoutMutation(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	path := Path(root)
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	store.maxBytes = int64(len(before) - 1)
	if _, err := store.Append(receiptDraft(t, "blocked")); err == nil ||
		!strings.Contains(err.Error(), "exceeds") {
		t.Fatalf("existing over-limit error = %v", err)
	}
	assertLedgerBytes(t, path, before)
}

func canonicalNextLine(t *testing.T, draft outputbinding.AgentOutputReceipt,
	sequence int64, prior *string) []byte {
	t.Helper()
	draft.LedgerSequence = sequence
	draft.PriorReceiptSHA256 = prior
	sealed, err := outputbinding.SealReceipt(draft)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := outputbinding.CanonicalReceiptJSON(sealed)
	if err != nil {
		t.Fatal(err)
	}
	return append(encoded, '\n')
}

func assertLedgerBytes(t *testing.T, path string, want []byte) {
	t.Helper()
	got, err := os.ReadFile(path)
	if err != nil || !bytes.Equal(got, want) {
		t.Fatalf("ledger image = %d bytes, %v; want %d bytes", len(got), err, len(want))
	}
}

func assertLedgerImage(t *testing.T, path string, want []byte, mode os.FileMode) {
	t.Helper()
	assertLedgerBytes(t, path, want)
	info, err := os.Stat(path)
	if err != nil || info.Mode().Perm() != mode {
		t.Fatalf("ledger mode = %v, %v; want %#o", info, err, mode)
	}
}
