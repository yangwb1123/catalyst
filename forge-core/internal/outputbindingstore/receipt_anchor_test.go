package outputbindingstore

import (
	"bytes"
	"os"
	"testing"
)

func TestReceiptJournalValidPrefixRollbackFailsClosed(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	firstImage, err := os.ReadFile(Path(root))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := store.Append(receiptDraft(t, "second")); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(Path(root), firstImage, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root).Load(); err == nil {
		t.Fatal("valid-prefix receipt rollback was accepted")
	}
}

func TestReceiptJournalEmptyRollbackCannotRestartAtGenesis(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	anchorBefore, err := os.ReadFile(ReceiptAnchorPath(root))
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(Path(root), nil, 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root).Append(receiptDraft(t, "second")); err == nil {
		t.Fatal("empty receipt rollback restarted the chain at genesis")
	}
	anchorAfter, err := os.ReadFile(ReceiptAnchorPath(root))
	if err != nil || !bytes.Equal(anchorBefore, anchorAfter) {
		t.Fatalf("rejected rollback changed receipt anchor: %v", err)
	}
}

func TestReceiptJournalAndAnchorMustBothBePresent(t *testing.T) {
	root := t.TempDir()
	store := New(root)
	if _, err := store.Append(receiptDraft(t, "first")); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(ReceiptAnchorPath(root)); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root).Load(); err == nil {
		t.Fatal("nonempty receipt journal without anchor was accepted")
	}
}
